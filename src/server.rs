use anyhow::{Context, Result};
use iroh::{Endpoint, SecretKey, address_lookup::{self, PkarrPublisher}, endpoint::presets, protocol::Router};
use tokio::io::{AsyncReadExt};
use std::{path::Path, str::FromStr};
use tracing::info;

use crate::protocols::{file_send::{alpn::FILE_ALPN_V1, file_send_protocol_handler::FileServerProtocolV1}, ping::{alpn::PING_ALPN_V1, ping_protocol_handler::PingServerProtocolV1}, proxy::{alpn::TCP_PROXY_ALPN_V1, proxy_protocol_handler::ProxyServerProtocolV1}};

fn load_or_create_secret_key() -> Result<SecretKey> {
    let path = ".server-key";
    if Path::new(path).exists() {
        let hex = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read key file: {path}"))?;
        let key = SecretKey::from_str(&hex).with_context(|| "")?;
        info!("Loaded secret key from {path}");
        Ok(key)
    } else {
        let key = SecretKey::generate();
        let ss: String = key.to_bytes().iter().map(|b| format!("{b:02x}")).collect();
        std::fs::write(path, ss)
            .with_context(|| format!("failed to write key file: {path}"))?;
        info!("Generated new secret key, saved to {path}");
        Ok(key)
    }
}

pub async fn run_server() -> Result<()> {
    info!("Server mode");
    let secret_key = load_or_create_secret_key()?;

    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .address_lookup(PkarrPublisher::n0_dns())
        .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
        .bind()
        .await?;

    let router = Router::builder(endpoint)
        .accept(PING_ALPN_V1, PingServerProtocolV1)
        .accept(FILE_ALPN_V1, FileServerProtocolV1)
        .accept(TCP_PROXY_ALPN_V1, ProxyServerProtocolV1)
        .spawn();

    info!("Server NodeId: {}", router.endpoint().id());
    info!("Listening for iroh connections");

    tokio::signal::ctrl_c().await?;
    info!("Shutting down server");
    router.shutdown().await?;

    Ok(())
}

pub async fn copy_bytes<A, B>(a: &mut A, b: &mut B, size: usize) -> anyhow::Result<()>
    where 
        A: tokio::io::AsyncRead + Unpin,
        B: tokio::io::AsyncWrite + Unpin 
    {
        let mut limited = a.take(size as u64);
        tokio::io::copy(&mut limited, b).await?;
        Ok(())
    }
