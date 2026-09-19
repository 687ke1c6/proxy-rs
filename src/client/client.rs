use std::str::FromStr;
use std::sync::Arc;
use anyhow::{Context, Result};
use iroh::{Endpoint, EndpointId, address_lookup::{self, PkarrPublisher}, endpoint::{presets}};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::{protocols::{ack::Ack, codec::StreamCodec, file_send::{alpn::FILE_ALPN_V1, file_send_header::FileSendHeader}, ping::{alpn::PING_ALPN_V1, ping_header::PingHeader}, proxy::{alpn::TCP_PROXY_ALPN_V1, proxy_header::ProxyHeaderV1}}, stream_helpers::proxy_streams};
use crate::socks5;
use crate::http;
use crate::client::client_helpers::{load_node_id_from_file, resolve_node_id};

#[derive(Debug, Clone, Copy)]
enum ProxyType {
    Socks5,
    Http,
    Tunnel,
}

fn get_proxy_addr_and_type(url: &str) -> (ProxyType, String) {
    let (prefix, addr) = url
        .split_once("://")
        .expect("Invalid format: must be protocol://host:port");

    let typ = match prefix.to_lowercase().as_str() {
        "socks5" => ProxyType::Socks5,
        "http" => ProxyType::Http,
        "tunnel" => ProxyType::Tunnel,
        _ => panic!("Unsupported proxy type: {}", prefix),
    };

    (typ, addr.to_string())
}

/// Parses a `tunnel://` target of the form `local_host:local_port:remote_host:remote_port`.
fn parse_tunnel_addr(addr: &str) -> Result<(String, String, u16)> {
    let parts: Vec<&str> = addr.split(':').collect();
    let [local_host, local_port, remote_host, remote_port] = parts.as_slice() else {
        anyhow::bail!(
            "Invalid tunnel address {addr:?}: expected local_host:local_port:remote_host:remote_port"
        );
    };
    let remote_port: u16 = remote_port
        .parse()
        .with_context(|| format!("Invalid remote port in tunnel address {addr:?}"))?;
    // validate the local port too, even though TcpListener::bind takes it as a string
    local_port
        .parse::<u16>()
        .with_context(|| format!("Invalid local port in tunnel address {addr:?}"))?;

    Ok((format!("{local_host}:{local_port}"), remote_host.to_string(), remote_port))
}

async fn ping_server(endpoint: &Endpoint, server_node_id: EndpointId) -> Result<()> {
    const MSG: &str = "ping";
    let conn = endpoint.connect(server_node_id, PING_ALPN_V1).await?;
    let (mut send, mut recv) = conn.open_bi().await.with_context(||"Could not get bi_directional channel")?;
    PingHeader { version: 1, msg: MSG.to_string() }.encode(&mut send).await?;
    send.finish()?;
    let pong = PingHeader::decode(&mut recv).await.with_context(||"Couldn't receive ping header")?;
    anyhow::ensure!(pong.msg == MSG, "ping/pong message mismatch: got {:?}", pong.msg);
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

    let raw: String = match server_node_id_str {
        Some(id) => id,
        None => load_node_id_from_file()?,
    };
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
        let conn = endpoint.connect(server_node_id, FILE_ALPN_V1).await?;
        info!("connected to server {server_node_id}");

        let (mut iroh_send, mut iroh_recv) = conn.open_bi().await?;
        info!("sending file header");

        let file_send_header = FileSendHeader { file_name, file_size, version: 1, can_overwrite };
        file_send_header.encode(&mut iroh_send).await?;
        info!("sent file header, waiting for ack");

        let file_send_header_ack = Ack::decode(&mut iroh_recv).await?;
        info!("ack received, {}", file_send_header_ack.msg);
        if file_send_header_ack.ack != 0 {
            info!("Bad ack from server, {}", file_send_header_ack.msg);
            anyhow::bail!("Server responded with error ack: {:?}", file_send_header_ack.msg);
        }
        let bytes = tokio::io::copy(&mut reader, &mut iroh_send).await?;
        info!("Finished sending file: {bytes} bytes");
        iroh_send.finish()?;
        let file_send_ack = Ack::decode(&mut iroh_recv).await?;
        if file_send_ack.ack != 0 {
            anyhow::bail!("Server responded with error ack: {:?}", file_send_ack.msg);
        }
        Ok(())
    }.await;

    info!("shutting down p2p endpoint");
    endpoint.close().await;

    result
}

pub async fn run_tcp_client(listen_addr: String, server_node_id_str: Option<String>, name: Option<String>) -> Result<()> {
    info!("Client mode");

    let (typ, addr) = get_proxy_addr_and_type(&listen_addr);
    info!("Proxy type: {:?}, Proxy address: {addr}", typ);

    let tunnel_target = match typ {
        ProxyType::Tunnel => Some(parse_tunnel_addr(&addr)?),
        _ => None,
    };
    let bind_addr = tunnel_target
        .as_ref()
        .map(|(local_addr, _, _)| local_addr.clone())
        .unwrap_or(addr);

    let raw: String = resolve_node_id(server_node_id_str, name)?;
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

        let listener = TcpListener::bind(&bind_addr).await?;
        info!("Listening for {:?} connections on {bind_addr}", typ);

        loop {
            tokio::select! {
                result = listener.accept() => {
                    let (tcp_stream, peer_addr) = result?;
                    info!("Accepted {:?} connection from {peer_addr}", typ);

                    let ep = endpoint.clone();
                    let tunnel_target = tunnel_target.clone();
                    tokio::spawn(async move {
                        let result = match typ {
                            ProxyType::Socks5 => handle_socks5(tcp_stream, ep, server_node_id).await,
                            ProxyType::Http   => handle_http(tcp_stream, ep, server_node_id).await,
                            ProxyType::Tunnel => {
                                let (_, remote_host, remote_port) = tunnel_target.expect("tunnel target set for ProxyType::Tunnel");
                                handle_tunnel(tcp_stream, ep, server_node_id, remote_host, remote_port).await
                            }
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

    let proxy_header = ProxyHeaderV1 { version: 1, host: host.clone(), port };
    proxy_header.encode(&mut iroh_send).await?;

    if !preamble.is_empty() {
        iroh_send.write_all(&preamble).await?;
    }

    let (tcp_read, tcp_write) = tcp.into_split();
    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("HTTP proxy connection to {}:{} via iroh server closed", host, port);
    Ok(())
}

async fn handle_tunnel(
    tcp: TcpStream,
    endpoint: Arc<Endpoint>,
    server_node_id: EndpointId,
    remote_host: String,
    remote_port: u16,
) -> Result<()> {
    info!("Tunnel -> {}:{}", remote_host, remote_port);

    let conn = endpoint.connect(server_node_id, TCP_PROXY_ALPN_V1).await?;
    let (mut iroh_send, iroh_recv) = conn.open_bi().await?;

    let proxy_header = ProxyHeaderV1 { version: 1, host: remote_host, port: remote_port };
    proxy_header.encode(&mut iroh_send).await?;

    let (tcp_read, tcp_write) = tcp.into_split();
    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("Tunnel connection to {}:{} via iroh server closed", proxy_header.host, proxy_header.port);
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

    let proxy_header = ProxyHeaderV1 { version: 1, host, port };
    proxy_header.encode(&mut iroh_send).await?;

    let (tcp_read, tcp_write) = tcp.into_split();
    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("Connection to {}:{} via iroh server closed", proxy_header.host, proxy_header.port);
    Ok(())
}
