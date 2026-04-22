use anyhow::{Context, Result};
use iroh::{Endpoint, SecretKey, address_lookup::{self, PkarrPublisher}, endpoint::presets};
use std::{path::Path, str::FromStr};
use tokio::net::TcpStream;
use tracing::{error, info, warn};

use crate::proxy::{proxy_streams, read_target, ALPN};

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
        .alpns(vec![ALPN.to_vec()])
        .address_lookup(PkarrPublisher::n0_dns())
        .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
        .bind()
        .await?;

    info!("Server NodeId: {}", endpoint.id());
    info!("Listening for iroh connections");

    loop {
        tokio::select! {
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else { break };
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(incoming).await {
                        error!("Connection error: {e:#}");
                    }
                });
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting down server");
                break;
            }
        }
    }

    endpoint.close().await;
    Ok(())
}

async fn handle_connection(incoming: iroh::endpoint::Incoming) -> Result<()> {
    let conn = incoming.await?;
    let remote = conn.remote_id();
    info!("Accepted iroh connection from {remote}");

    let (iroh_send, mut iroh_recv) = conn.accept_bi().await?;

    let proxy_header = read_target(&mut iroh_recv).await?;
    let tcp_target = format!("{}:{}", proxy_header.host, proxy_header.port);
    info!("Connecting to TCP target {tcp_target}");

    let tcp = TcpStream::connect(&tcp_target).await?;
    let (tcp_read, tcp_write) = tcp.into_split();

    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("Connection from {remote} to {tcp_target} closed");
    Ok(())
}
