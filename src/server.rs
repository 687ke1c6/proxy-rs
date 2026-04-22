use anyhow::{Context, Result};
use iroh::{Endpoint, SecretKey, address_lookup::{self, PkarrPublisher}, endpoint::presets, protocol::Router};
use std::{path::Path, str::FromStr};
use tracing::info;

use crate::proxy::ALPN;
use crate::protocols::proxy::ProxyServerProtocol;

fn load_or_create_secret_key() -> Result<SecretKey> {
    let path = ".server.key";
    if Path::new(path).exists() {
        let hex = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read key file: {path}"))?;
        let key = SecretKey::from_str(&hex).with_context(|| "")?;
        info!("Loaded secret key from {path}");
        Ok(key)
    } else {
        let key = SecretKey::generate();
        let ss = String::from_utf8(key.to_bytes().to_vec()).with_context(|| format!("failed to convert key to string: {path}"))?;
        std::fs::write(path, ss)
            .with_context(|| format!("failed to write key file: {path}"))?;
        info!("Generated new secret key, saved to {path}");
        Ok(key)
    }
}

pub async fn run() -> Result<()> {
    info!("Server mode");
    let secret_key = load_or_create_secret_key()?;

    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .address_lookup(PkarrPublisher::n0_dns())
        .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
        .bind()
        .await?;

    let router = Router::builder(endpoint)
        .accept(ALPN, ProxyServerProtocol)
        .spawn();

    info!("Server NodeId: {}", router.endpoint().id());
    info!("Listening for iroh connections");

    tokio::signal::ctrl_c().await?;
    info!("Shutting down server");
    router.shutdown().await?;

    Ok(())
}
