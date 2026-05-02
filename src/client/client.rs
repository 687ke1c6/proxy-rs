use std::str::FromStr;
use std::sync::Arc;
use anyhow::{Context, Result};
use iroh::{Endpoint, EndpointId, address_lookup::{self, PkarrPublisher}, endpoint::{presets}};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::protocols::{ack::Ack, file_send::{alpn::FILE_ALPN_V1, file_send_header::FileSendHeader}, ping::{alpn::PING_ALPN_V1, ping_header::PingHeader}, proxy::{alpn::TCP_PROXY_ALPN_V1, proxy_header::ProxyHeader}};
use crate::protocols::proxy::proxy_helpers::{proxy_streams};
use crate::socks5;
use crate::http;
use crate::client::client_helpers::{load_node_id_from_file, write_node_id_to_file};

#[derive(Debug, Clone, Copy)]
enum ProxyType {
    Socks5,
    Http,
}

fn get_proxy_addr_and_type(url: &str) -> (ProxyType, String) {
    let (prefix, addr) = url
        .split_once("://")
        .expect("Invalid format: must be protocol://host:port");

    let typ = match prefix.to_lowercase().as_str() {
        "socks5" => ProxyType::Socks5,
        "http" => ProxyType::Http,
        _ => panic!("Unsupported proxy type: {}", prefix),
    };

    (typ, addr.to_string())
}

async fn ping_server(endpoint: &Endpoint, server_node_id: EndpointId) -> Result<()> {
    const MSG: &str = "ping";
    info!("Pinging server {server_node_id}");
    let conn = endpoint.connect(server_node_id, PING_ALPN_V1).await?;
    info!("Connected to server, opening stream");
    let (mut send, mut recv) = conn.open_bi().await.with_context(||"Could not get bi_directional channel")?;
    info!("Sending ping to server {server_node_id}");
    PingHeader { version: 1, msg: MSG.to_string() }.write_to_stream(&mut send).await?;
    info!("Ping sent");
    send.finish()?;
    info!("Ping finished, waiting for pong response");
    let pong = PingHeader::from_stream(&mut recv).await.with_context(||"Couldn't receive ping header")?;
    info!("Received pong from server: {:?}", pong.msg);
    anyhow::ensure!(pong.msg == MSG, "ping/pong message mismatch: got {:?}", pong.msg);
    info!("Server is reachable (pong: {:?})", pong.msg);
    Ok(())
}

pub async fn run_send_file(file_path: String, server_node_id_str: Option<String>, can_overwrite: bool) -> Result<()> {
    info!("Client send file");

    let full_path = std::fs::canonicalize(&file_path).with_context(|| format!("Failed to canonicalize path: {file_path}"))?;
    let metadata = tokio::fs::metadata(&full_path).await?;
    let file_size = metadata.len();

    info!("{}, {file_size} bytes", full_path.display());

    let path = std::path::Path::new(&full_path);

    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap().to_string();

    let mut reader = tokio::fs::File::open(&full_path).await?;

    let raw: String = server_node_id_str.unwrap_or_else(|| load_node_id_from_file().expect("Could not get node-id"));
    let server_node_id = EndpointId::from_str(&raw).with_context(|| "Could not parse server node id")?;

    let endpoint = Arc::new(
        Endpoint::builder(presets::N0)
            .address_lookup(PkarrPublisher::n0_dns())
            .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
            .bind()
            .await?
    );

    info!("creating endpoint");
    endpoint.online().await;

    let result = async {
        ping_server(&endpoint, server_node_id).await?;
        info!("connecting");
        let conn = endpoint.connect(server_node_id, FILE_ALPN_V1).await?;
        info!("connected to server {server_node_id}");
        let (mut iroh_send, mut iroh_recv) = conn.open_bi().await?;
        info!("opened bidirectional stream");
        info!("sending file header");
        let file_send_header = FileSendHeader { file_name, file_size, version: 1, can_overwrite };
        file_send_header.write_to_stream(&mut iroh_send).await?;
        info!("sent file header, waiting for ack");
        let file_send_header_ack = Ack::read_ack(&mut iroh_recv).await?;
        if file_send_header_ack.ack != 0 {
            anyhow::bail!("Server responded with error ack: {:?}", file_send_header_ack.msg);
        }
        let bytes = tokio::io::copy(&mut reader, &mut iroh_send).await?;
        info!("Finished sending file: {bytes} bytes");
        iroh_send.finish()?;
        let file_send_ack = Ack::read_ack(&mut iroh_recv).await?;
        if file_send_ack.ack != 0 {
            anyhow::bail!("Server responded with error ack: {:?}", file_send_ack.msg);
        }
        Ok(())
    }.await;

    info!("shutting down p2p endpoint");
    endpoint.close().await;

    result
}

pub async fn run_tcp_client(listen_addr: String, server_node_id_str: Option<String>) -> Result<()> {
    info!("Client mode");

    let (typ, addr) = get_proxy_addr_and_type(&listen_addr);
    info!("Proxy type: {:?}, Proxy address: {addr}", typ);

    if let Some(id) = &server_node_id_str {
        write_node_id_to_file(id)?;
    }

    let raw: String = server_node_id_str.unwrap_or_else(|| load_node_id_from_file().unwrap());
    let server_node_id = EndpointId::from_str(&raw).with_context(|| "Could not parse server node id")?;

    let endpoint = Arc::new(
        Endpoint::builder(presets::N0)
            .address_lookup(PkarrPublisher::n0_dns())
            .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
            .bind()
            .await?,
    );
    endpoint.online().await;

    let result = async {
        ping_server(&endpoint, server_node_id).await?;

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
        Ok(())
    }.await;

    endpoint.close().await;
    result
}

async fn handle_http(
    mut tcp: TcpStream,
    endpoint: Arc<Endpoint>,
    server_node_id: EndpointId,
) -> Result<()> {
    let (host, port, preamble) = http::handshake(&mut tcp).await?;
    info!("HTTP proxy -> {}:{}", host, port);

    let conn = endpoint.connect(server_node_id, TCP_PROXY_ALPN_V1).await?;
    let (mut iroh_send, iroh_recv) = conn.open_bi().await?;

    let proxy_header = ProxyHeader { version: 1, host: host.clone(), port, can_read: true, can_write: false, can_execute: false };
    proxy_header.write_to_stream(&mut iroh_send).await?;

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
    let conn = endpoint.connect(server_node_id, TCP_PROXY_ALPN_V1).await?;
    info!("Connected.");

    let (mut iroh_send, iroh_recv) = conn.open_bi().await?;

    let proxy_header = ProxyHeader { version: 1, host, port, can_read: true, can_write: false, can_execute: false };
    proxy_header.write_to_stream(&mut iroh_send).await?;

    let (tcp_read, tcp_write) = tcp.into_split();
    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("Connection to {}:{} via iroh server closed", proxy_header.host, proxy_header.port);
    Ok(())
}
