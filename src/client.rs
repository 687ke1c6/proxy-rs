use std::sync::Arc;

use anyhow::{Context, Result};
use std::path::Path;
use iroh::{Endpoint, NodeAddr, NodeId};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::proxy::{ALPN, ProxyHeader, proxy_streams, write_target};
use crate::socks5;

#[derive(Debug)]
enum ProxyType {
    Socks5,
    Http,
}

const PATH: &str = ".node-id";

fn load_key_from_file() -> Result<String> {
    if Path::new(PATH).exists() {
        let hex = std::fs::read_to_string(PATH)
            .with_context(|| format!("failed to read key file: {PATH}"))?;
        let key = hex
            .trim()
            .parse()
            .with_context(|| format!("failed to parse key file: {PATH}"))?;
        info!("Loaded secret key from {PATH}");
        Ok(key)
    } else {
        panic!("Target node id not found.");
    }
}

fn write_key_to_file(key: &str) -> Result<()> {
    std::fs::write(PATH, key.to_string())
        .with_context(|| format!("failed to write key file: {PATH}"))?;
    info!("Generated new secret key, saved to {PATH}");
    Ok(())
}

fn get_proxy_addr_and_type(url: &str) -> (ProxyType, String) {
    let (prefix, addr) = url
        .split_once("://")
        .expect("Invalid format: must be protocol://host:port");

    // Use strip_suffix to get rid of the ":" if it exists
    let typ = match prefix {
        "socks5" => ProxyType::Socks5,
        "http" => ProxyType::Http,
        _ => panic!("Unsupported proxy type: {}", prefix),
    };

    (typ, addr.to_string())
}

pub async fn run(listen_addr: String, server_node_id: Option<String>) -> Result<()> {
    info!("Client mode");

    let (typ, addr) = get_proxy_addr_and_type(&listen_addr);

    info!("Proxy type: {:?}, Proxy address: {addr}", typ);

    if let Some(id) = &server_node_id {
        write_key_to_file(id).unwrap();
    }

    let key = server_node_id.unwrap_or_else(|| load_key_from_file().unwrap());

    let node_id: NodeId = key.parse()?;

    let endpoint = Arc::new(
        Endpoint::builder()
            .discovery_n0()
            .bind()
            .await?,
    );

    info!("Client NodeId: {}", endpoint.node_id());

    let listener = TcpListener::bind(&addr).await?;
    info!("Listening for SOCKS5 connections on {addr}");

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (tcp_stream, peer_addr) = result?;
                info!("Accepted SOCKS5 connection from {peer_addr}");

                let ep = endpoint.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_socks5(tcp_stream, ep, node_id).await {
                        error!("Proxy error from {peer_addr}: {e:#}");
                    }
                });
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting down client");
                break;
            }
        }
    }

    endpoint.close().await;
    Ok(())
}

async fn handle_socks5(
    mut tcp: TcpStream,
    endpoint: Arc<Endpoint>,
    server_node_id: NodeId,
) -> Result<()> {
    let (host, port) = socks5::handshake(&mut tcp).await?;
    info!("SOCKS5 CONNECT -> {}:{}", host, port);

    let addr = NodeAddr::from(server_node_id);
    info!("Connecting to iroh server {server_node_id}");
    let conn = endpoint.connect(addr, ALPN).await?;
    info!("Connected.");

    let (mut iroh_send, iroh_recv) = conn.open_bi().await?;

    let proxy_header = ProxyHeader { host, port, can_read: true, can_write: false, can_execute: false };
    write_target(&mut iroh_send, &proxy_header).await?;

    let (tcp_read, tcp_write) = tcp.into_split();
    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("Connection to {}:{} via {server_node_id} closed", proxy_header.host, proxy_header.port);
    Ok(())
}
