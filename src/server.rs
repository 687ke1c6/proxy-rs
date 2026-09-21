use anyhow::{Context, Result};
use iroh::{Endpoint, SecretKey, address_lookup::{self, PkarrPublisher}, endpoint::presets, protocol::Router};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

use crate::config_dir::config_dir;
use crate::protocols::{file_send::{alpn::FILE_ALPN_V1, file_send_protocol_handler::FileServerProtocolV1}, list_volumes::{alpn::LIST_VOLUMES_ALPN_V1, list_volumes_protocol_handler::ListVolumesServerProtocolV1}, ping::{alpn::PING_ALPN_V1, ping_protocol_handler::PingServerProtocolV1}, proxy::{alpn::TCP_PROXY_ALPN_V1, proxy_protocol_handler::ProxyServerProtocolV1}};

fn load_or_create_secret_key() -> Result<SecretKey> {
    let path = config_dir()?.join("server-key");
    let key = if path.exists() {
        let hex = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read key file: {}", path.display()))?;
        SecretKey::from_str(&hex).with_context(|| "")?
    } else {
        let key = SecretKey::generate();
        let ss: String = key.to_bytes().iter().map(|b| format!("{b:02x}")).collect();
        std::fs::write(&path, ss)
            .with_context(|| format!("failed to write key file: {}", path.display()))?;
        info!("Generated new secret key, saved to {} {}", path.display(), key.public().to_string());
        key
    };

    let pub_path = config_dir()?.join("server-key.pub");
    std::fs::write(&pub_path, key.public().to_string())
        .with_context(|| format!("failed to write public key file: {}", pub_path.display()))?;

    Ok(key)
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

pub async fn run_server(volume_specs: Vec<String>) -> Result<()> {
    println!("Mode: server");
    let volumes = Arc::new(parse_volumes(&volume_specs)?);
    if volumes.is_empty() {
        println!("Volumes: (none exposed — file transfers will be rejected)");
    } else {
        println!("Volumes:");
        for (name, path) in volumes.iter() {
            println!("  {name} -> {}", path.display());
        }
    }
    let secret_key = load_or_create_secret_key()?;

    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .address_lookup(PkarrPublisher::n0_dns())
        .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
        .bind()
        .await?;

    let router = Router::builder(endpoint)
        .accept(PING_ALPN_V1, PingServerProtocolV1)
        .accept(FILE_ALPN_V1, FileServerProtocolV1 { volumes: volumes.clone() })
        .accept(LIST_VOLUMES_ALPN_V1, ListVolumesServerProtocolV1 { volumes: volumes.clone() })
        .accept(TCP_PROXY_ALPN_V1, ProxyServerProtocolV1)
        .spawn();

    // Essential output, not a log line: the operator needs this id to give to clients,
    // and it must stay visible at the default (error-only) log level.
    println!("Iroh node listening [{}]", router.endpoint().id());

    tokio::signal::ctrl_c().await?;
    info!("Shutting down server");
    router.shutdown().await?;

    Ok(())
}
