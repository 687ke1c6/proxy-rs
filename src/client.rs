use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use iroh::{Endpoint, EndpointId, address_lookup::{self, PkarrPublisher}, endpoint::presets};
use std::path::Path;
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::http;
use crate::proxy::{ALPN, ProxyHeader, proxy_streams, write_target};
use crate::socks5;

#[derive(Debug, Clone, Copy)]
enum ProxyType {
    Socks5,
    Http,
}

const PATH: &str = ".node-id";

fn load_node_id_from_file() -> Result<String> {
    if Path::new(PATH).exists() {
        let id = std::fs::read_to_string(PATH)
            .with_context(|| format!("failed to read node id file: {PATH}"))?;
        info!("Loaded server node id from {PATH}");
        Ok(id.trim().to_string())
    } else {
        panic!("Server node id not found. Pass it as an argument on first run.");
    }
}

fn write_node_id_to_file(id: &str) -> Result<()> {
    std::fs::write(PATH, id)
        .with_context(|| format!("failed to write node id file: {PATH}"))?;
    info!("Saved server node id to {PATH}");
    Ok(())
}

fn get_proxy_addr_and_type(url: &str) -> (ProxyType, String) {
    let (prefix, addr) = url
        .split_once("://")
        .expect("Invalid format: must be protocol://host:port");

    let typ = match prefix {
        "socks5" => ProxyType::Socks5,
        "http" => ProxyType::Http,
        _ => panic!("Unsupported proxy type: {}", prefix),
    };

    (typ, addr.to_string())
}

pub async fn run(listen_addr: String, server_node_id_str: Option<String>) -> Result<()> {
    info!("Client mode");

    let (typ, addr) = get_proxy_addr_and_type(&listen_addr);
    info!("Proxy type: {:?}, Proxy address: {addr}", typ);

    if let Some(id) = &server_node_id_str {
        write_node_id_to_file(id)?;
    }

    let raw = server_node_id_str.unwrap_or_else(|| load_node_id_from_file().unwrap());
    let server_node_id = EndpointId::from_str(&raw).with_context(|| "Could not parse server node id")?;

    let endpoint = Arc::new(
        Endpoint::builder(presets::N0)
            .address_lookup(PkarrPublisher::n0_dns())
            .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
            .bind()
            .await?,
    );
    endpoint.online().await;

    info!("Client NodeId: {}", endpoint.id());
    info!("Connecting to server NodeId: {server_node_id}");

    let listener = TcpListener::bind(&addr).await?;
    info!("Listening for {:?} connections on {addr}", typ);

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (tcp_stream, peer_addr) = result?;
                info!("Accepted {:?} connection from {peer_addr}", typ);

                let ep = endpoint.clone();
                tokio::spawn(async move {
                    let result = match typ {
                        ProxyType::Socks5 => handle_socks5(tcp_stream, ep, server_node_id).await,
                        ProxyType::Http   => handle_http(tcp_stream, ep, server_node_id).await,
                    };
                    if let Err(e) = result {
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

async fn handle_http(
    mut tcp: TcpStream,
    endpoint: Arc<Endpoint>,
    server_node_id: EndpointId,
) -> Result<()> {
    let (host, port, preamble) = http::handshake(&mut tcp).await?;
    info!("HTTP proxy -> {}:{}", host, port);

    let conn = endpoint.connect(server_node_id, ALPN).await?;
    let (mut iroh_send, iroh_recv) = conn.open_bi().await?;

    let proxy_header = ProxyHeader { version: 1, host: host.clone(), port, can_read: true, can_write: false, can_execute: false };
    write_target(&mut iroh_send, &proxy_header).await?;

    if !preamble.is_empty() {
        iroh_send.write_all(&preamble).await?;
    }

    let (tcp_read, tcp_write) = tcp.into_split();
    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("HTTP proxy connection to {}:{} via iroh server closed", host, port);
    Ok(())
}

async fn handle_socks5(
    mut tcp: TcpStream,
    endpoint: Arc<Endpoint>,
    server_node_id: EndpointId,
) -> Result<()> {
    let (host, port) = socks5::handshake(&mut tcp).await?;
    info!("SOCKS5 CONNECT -> {}:{}", host, port);

    info!("Connecting to iroh server {server_node_id}");
    let conn = endpoint.connect(server_node_id, ALPN).await?;
    info!("Connected.");

    let (mut iroh_send, iroh_recv) = conn.open_bi().await?;

    let proxy_header = ProxyHeader { version: 1, host, port, can_read: true, can_write: false, can_execute: false };
    write_target(&mut iroh_send, &proxy_header).await?;

    let (tcp_read, tcp_write) = tcp.into_split();
    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("Connection to {}:{} via iroh server closed", proxy_header.host, proxy_header.port);
    Ok(())
}
