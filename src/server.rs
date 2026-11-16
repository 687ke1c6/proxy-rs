use anyhow::{Context, Result};
use iroh::{Endpoint, address_lookup::{self, PkarrPublisher}, endpoint::presets, protocol::Router};
use std::sync::Arc;

use crate::BANNER;
use crate::allowlist::{ClientAllowlist, ClientGate};
use crate::config::{banner_suffix, config_path, load_secret_key};
use crate::protocols::ping::{alpn::PING_ALPN, ping_protocol_handler::PingServerProtocol};
use crate::protocols::stream::{alpn::STREAM_ALPN, rsync::Volumes, stream_protocol_handler::{Mode, StreamServerProtocol}};

/// Parses `-v`/`--volume` specs of the form `name:path` into a name -> canonicalized
/// root directory map. Each path must exist and be a directory; failing fast here (at
/// startup) beats discovering a typo the first time a client tries to push.
pub fn parse_volumes(raw: &[String]) -> Result<Volumes> {
    let mut volumes = Volumes::new();
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

/// Runs a server in `mode`, admitting `allow` plus the config file's `[clients]`.
pub async fn run_server(mode: Mode, allow: Vec<String>) -> Result<()> {
    // Before the allowlist is read: creates the config file if this is the first run.
    let secret_key = load_secret_key()?;
    let allowlist = ClientAllowlist::new(&allow, config_path()?)?;
    let clients = allowlist.count()?;

    // Printed rather than logged: the operator should see exactly what is exposed even at
    // the default (error-only) log level. One line: version, mode, what it reaches, client count.
    let mut summary = format!("{BANNER} {}", mode.label());
    match &mode {
        Mode::Socks5(policy) | Mode::Http(policy) => {
            anyhow::ensure!(!policy.is_empty(), "no targets: pass at least one -t/--target");
            let patterns: Vec<String> = policy.patterns().iter().map(ToString::to_string).collect();
            summary += &format!(" targets={}", patterns.join(","));
        }
        Mode::Tunnel { host, port } => summary += &format!(" -> {host}:{port}"),
        Mode::Rsync(volumes) => anyhow::ensure!(!volumes.is_empty(), "no volumes: pass at least one -v/--volume"),
    }
    match clients {
        0 => summary += " clients=any",
        n => summary += &format!(" clients={n}"),
    }
    summary += &banner_suffix();
    println!("{summary}");
    if let Mode::Rsync(volumes) = &mode {
        let mut volumes_sorted: Vec<_> = volumes.iter().collect();
        volumes_sorted.sort();
        for (name, path) in volumes_sorted {
            println!("  volume {name} -> {}", path.display());
        }
        if !rsync_on_path() {
            eprintln!("WARNING: rsync was not found on PATH; pushes will fail");
        }
    }
    if let Mode::Socks5(policy) | Mode::Http(policy) = &mode
        && policy.is_open()
    {
        eprintln!("WARNING: open proxy (-t '*'): clients can reach any host, including localhost and the LAN");
    }
    if clients == 0 {
        eprintln!("WARNING: no allowlist: anyone with this node id can connect (see --allow, or [clients] in {})", allowlist.file().display());
    }

    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .address_lookup(PkarrPublisher::n0_dns())
        .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
        // Every ALPN, ping included, sits behind the client allowlist.
        .hooks(ClientGate { allowlist })
        .bind()
        .await?;

    // Ping (the client's session) plus the one stream protocol, handled according to `mode`.
    let router = Router::builder(endpoint)
        .accept(PING_ALPN, PingServerProtocol)
        .accept(STREAM_ALPN, StreamServerProtocol { mode: Arc::new(mode) })
        .spawn();

    // Essential output, not a log line: the operator needs this id to give to clients,
    // and it must stay visible at the default (error-only) log level.
    println!("{}", router.endpoint().id());

    let signal = crate::signals::shutdown_signal().await;
    crate::signals::announce(signal);
    // Closes every client connection cleanly, so clients notice at once rather than after
    // QUIC's idle timeout.
    router.shutdown().await?;

    Ok(())
}
