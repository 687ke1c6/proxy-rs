use anyhow::{Context, Result};
use iroh::{Endpoint, address_lookup::{self, PkarrPublisher}, endpoint::presets, protocol::{AccessLimit, ProtocolHandler, Router, RouterBuilder}};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use crate::authorized_clients::{self, ClientAllowlist};
use crate::cli::ServerArgs;
use crate::identity::{SERVER_KEY_FILE, load_or_create_secret_key};
use crate::config_dir::config_dir;
use crate::protocols::{file_send::{alpn::FILE_ALPN_V1, file_send_protocol_handler::FileServerProtocolV1}, list_volumes::{alpn::LIST_VOLUMES_ALPN_V1, list_volumes_protocol_handler::ListVolumesServerProtocolV1}, ping::{alpn::PING_ALPN_V1, ping_protocol_handler::PingServerProtocolV1}, proxy::{alpn::TCP_PROXY_ALPN_V2, proxy_protocol_handler::ProxyServerProtocolV2, target_policy::TargetPolicy}, rsync::{alpn::RSYNC_ALPN_V1, rsync_protocol_handler::RsyncServerProtocolV1}};

/// Registers `handler` on `alpn`, behind the client allowlist unless it's `None` (`--allow-any`).
/// A refused client's connection is closed with reason `not allowed` before `handler` sees it.
fn accept<P: ProtocolHandler + Clone>(
    router: RouterBuilder,
    alpn: &[u8],
    handler: P,
    allowlist: &Option<Arc<ClientAllowlist>>,
) -> RouterBuilder {
    match allowlist {
        Some(list) => {
            let list = list.clone();
            router.accept(alpn, AccessLimit::new(handler, move |id| list.allows(id)))
        }
        None => router.accept(alpn, handler),
    }
}

/// Parses `-v`/`--volume` specs of the form `name:path` into a name -> canonicalized
/// root directory map. Each path must exist and be a directory; failing fast here (at
/// startup) beats discovering a typo the first time a client tries to send a file.
fn parse_volumes(raw: &[String]) -> Result<HashMap<String, PathBuf>> {
    let mut volumes = HashMap::new();
    for spec in raw {
        let (name, path) = spec
            .split_once(':')
            .with_context(|| format!("invalid --volume {spec:?}: expected name:path"))?;
        anyhow::ensure!(
            !name.is_empty() && !name.contains('/'),
            "invalid volume name {name:?}: must be non-empty and must not contain '/'"
        );
        let canonical = std::fs::canonicalize(path)
            .with_context(|| format!("--volume {name}: failed to resolve path {path:?}"))?;
        anyhow::ensure!(canonical.is_dir(), "--volume {name}: {path:?} is not a directory");
        anyhow::ensure!(
            volumes.insert(name.to_string(), canonical).is_none(),
            "duplicate --volume name {name:?}"
        );
    }
    Ok(volumes)
}

fn rsync_on_path() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join("rsync").is_file()))
}

pub async fn run_server(args: ServerArgs) -> Result<()> {
    println!("Mode: server");
    let policy = Arc::new(TargetPolicy::parse(&args.target)?);
    let volumes = Arc::new(parse_volumes(&args.volumes)?);

    anyhow::ensure!(
        volumes.is_empty() || args.file || args.rsync,
        "--volume given but nothing uses it: add -f/--file and/or -r/--rsync"
    );
    anyhow::ensure!(
        !policy.is_empty() || args.file || args.rsync,
        "no features enabled: pass at least one of -t/--target, -f/--file, -r/--rsync"
    );
    anyhow::ensure!(
        !volumes.is_empty() || !(args.file || args.rsync),
        "-f/--file and -r/--rsync need at least one -v/--volume"
    );

    // Printed rather than logged: the operator should see exactly what is exposed even at
    // the default (error-only) log level.
    println!("Features:");
    if !policy.is_empty() {
        let patterns: Vec<String> = policy.patterns().iter().map(ToString::to_string).collect();
        println!("  tcp proxy -> {}", patterns.join(", "));
    }
    if args.file {
        println!("  file send");
    }
    if args.rsync {
        println!("  rsync push");
    }
    if !volumes.is_empty() {
        println!("Volumes:");
        for (name, path) in volumes.iter() {
            println!("  {name} -> {}", path.display());
        }
    }
    if policy.is_open() {
        eprintln!(
            "WARNING: open proxy enabled (--target '*'): any client with this node id can reach \
             anything this host can, including localhost and the LAN"
        );
    }

    let allowlist = if args.allow_any {
        None
    } else {
        let list = ClientAllowlist::new(&args.allow, config_dir()?.join(authorized_clients::FILENAME))?;
        list.create_file_if_missing()?;
        let file_count = list.read_file()?.len();
        anyhow::ensure!(
            list.fixed_count() + file_count > 0,
            "no clients allowed: pass --allow <node-id> (a client prints its id with `proxy-rs client whoami`), \
             add ids to {}, or pass --allow-any",
            list.file().display()
        );
        println!("Clients: {} via --allow, {file_count} in {} (re-read on every connection)", list.fixed_count(), list.file().display());
        Some(Arc::new(list))
    };
    if allowlist.is_none() {
        eprintln!("WARNING: --allow-any: any client that knows this server's node id can connect");
    }
    if args.rsync && !rsync_on_path() {
        eprintln!("WARNING: -r/--rsync enabled but rsync was not found on PATH; pushes will fail");
    }

    let secret_key = load_or_create_secret_key(SERVER_KEY_FILE)?;

    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .address_lookup(PkarrPublisher::n0_dns())
        .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
        .bind()
        .await?;

    // Only enabled features are registered; any other ALPN fails the QUIC handshake.
    // Every ALPN, ping included, sits behind the client allowlist.
    let mut router = accept(Router::builder(endpoint), PING_ALPN_V1, PingServerProtocolV1, &allowlist);
    if !policy.is_empty() {
        router = accept(router, TCP_PROXY_ALPN_V2, ProxyServerProtocolV2 { policy }, &allowlist);
    }
    if args.file {
        router = accept(router, FILE_ALPN_V1, FileServerProtocolV1 { volumes: volumes.clone() }, &allowlist);
    }
    if args.rsync {
        router = accept(router, RSYNC_ALPN_V1, RsyncServerProtocolV1 { volumes: volumes.clone() }, &allowlist);
    }
    if args.file || args.rsync {
        router = accept(router, LIST_VOLUMES_ALPN_V1, ListVolumesServerProtocolV1 { volumes }, &allowlist);
    }
    let router = router.spawn();

    // Essential output, not a log line: the operator needs this id to give to clients,
    // and it must stay visible at the default (error-only) log level.
    println!("Iroh node listening [{}]", router.endpoint().id());

    tokio::signal::ctrl_c().await?;
    info!("Shutting down server");
    router.shutdown().await?;

    Ok(())
}
