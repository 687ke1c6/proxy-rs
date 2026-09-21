use std::io::IsTerminal;
use std::str::FromStr;
use std::sync::Arc;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use iroh::{Endpoint, EndpointId, address_lookup::{self, PkarrPublisher}, endpoint::{presets}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::{protocols::{ack::Ack, codec::StreamCodec, file_send::{alpn::FILE_ALPN_V1, file_send_header::FileSendHeader}, list_volumes::{alpn::LIST_VOLUMES_ALPN_V1, list_volumes_header::{ListVolumesRequest, ListVolumesResponse}}, ping::{alpn::PING_ALPN_V1, ping_header::PingHeader}, proxy::{alpn::TCP_PROXY_ALPN_V1, proxy_header::ProxyHeaderV1}}, stream_helpers::proxy_streams};
use crate::socks5;
use crate::http;
use crate::client::client_helpers::resolve_node_id;

#[derive(Debug, Clone, Copy)]
pub enum ProxyType {
    Socks5,
    Http,
    Tunnel,
}

impl ProxyType {
    fn label(&self) -> &'static str {
        match self {
            ProxyType::Socks5 => "socks5",
            ProxyType::Http => "http",
            ProxyType::Tunnel => "tunnel",
        }
    }
}

/// The client's resolved intent for where a sent file should land on the server.
struct FileTarget {
    /// Volume name, e.g. "home"; empty means "let the server pick a default".
    volume: String,
    /// `/`-separated relative directory within the volume; empty means the volume root.
    target_dir: String,
    /// Overrides the file's local basename on the server, if the client renamed it.
    file_name: Option<String>,
}

/// Parses a `--target` spec of the form `volume[/dir...][/new_name]`.
///
/// A trailing `/` (or no `/` at all) means "directory only" and the file keeps its local
/// basename; anything else after the volume name is treated as `dir/.../new_name`, i.e. a
/// rename. Examples: `home`, `home/`, `home/subfolder/`, `home/subfolder/renamed.log`.
fn parse_target(spec: &str) -> Result<FileTarget> {
    let (volume, rest) = spec.split_once('/').unwrap_or((spec, ""));
    anyhow::ensure!(!volume.is_empty(), "invalid --target {spec:?}: must start with a volume name");

    if rest.is_empty() {
        return Ok(FileTarget { volume: volume.to_string(), target_dir: String::new(), file_name: None });
    }
    if let Some(dir) = rest.strip_suffix('/') {
        return Ok(FileTarget { volume: volume.to_string(), target_dir: dir.to_string(), file_name: None });
    }
    let (dir, name) = rest.rsplit_once('/').unwrap_or(("", rest));
    Ok(FileTarget { volume: volume.to_string(), target_dir: dir.to_string(), file_name: Some(name.to_string()) })
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

pub async fn run_list_volumes(server_node_id_str: Option<String>, name: Option<String>) -> Result<()> {
    println!("Mode: client volumes");

    let (node_name, raw): (String, String) = resolve_node_id(server_node_id_str, name)?;
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
        let conn = endpoint.connect(server_node_id, LIST_VOLUMES_ALPN_V1).await?;
        println!("Connected to \"{node_name}\" [{server_node_id}]");

        let (mut iroh_send, mut iroh_recv) = conn.open_bi().await?;
        ListVolumesRequest { version: 1 }.encode(&mut iroh_send).await?;
        iroh_send.finish()?;

        let response = ListVolumesResponse::decode(&mut iroh_recv).await?;
        if response.volumes.is_empty() {
            println!("(server exposes no volumes)");
        }
        for volume in response.volumes {
            println!("{}:{}", volume.name, volume.path);
        }
        Ok(())
    }.await;

    info!("shutting down p2p endpoint");
    endpoint.close().await;

    result
}

/// Copies `total_size` bytes from `reader` to `writer` in chunks, driving an indicatif
/// progress bar (bytes sent, transfer rate, ETA) as it goes. The bar renders to stderr
/// and only when stderr is a real terminal, so piping/redirecting output doesn't fill
/// the destination with raw progress-bar escape codes.
async fn copy_with_progress<R, W>(reader: &mut R, writer: &mut W, total_size: u64) -> Result<u64>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    const CHUNK_SIZE: usize = 256 * 1024;

    let draw_target = if std::io::stderr().is_terminal() {
        ProgressDrawTarget::stderr()
    } else {
        ProgressDrawTarget::hidden()
    };
    let pb = ProgressBar::with_draw_target(Some(total_size), draw_target);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA {eta})",
        )
        .expect("static progress bar template is valid")
        .progress_chars("#>-"),
    );

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut sent: u64 = 0;
    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n]).await?;
        sent += n as u64;
        pb.set_position(sent);
    }
    pb.finish_and_clear();

    Ok(sent)
}

pub async fn run_send_file(file_path: String, server_node_id_str: Option<String>, name: Option<String>, target: Option<String>, can_overwrite: bool) -> Result<()> {
    println!("Mode: client file");
    println!("Sending: {file_path}");
    println!("Target: {}", target.as_deref().unwrap_or("(server default volume)"));

    let full_path = std::fs::canonicalize(&file_path).with_context(|| format!("Failed to canonicalize path: {file_path}"))?;
    let metadata = tokio::fs::metadata(&full_path).await?;
    let file_size = metadata.len();

    info!("{}, {file_size} bytes", full_path.display());

    let path = std::path::Path::new(&full_path);

    let local_file_name = path.file_name().and_then(|s| s.to_str()).unwrap().to_string();

    let target = target.as_deref().map(parse_target).transpose()?;
    let (volume, target_dir, file_name) = match target {
        Some(t) => (t.volume, t.target_dir, t.file_name.unwrap_or(local_file_name)),
        None => (String::new(), String::new(), local_file_name),
    };

    let mut reader = tokio::fs::File::open(&full_path).await?;

    let (node_name, raw): (String, String) = resolve_node_id(server_node_id_str, name)?;
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
        println!("Connected to \"{node_name}\" [{server_node_id}]");

        let (mut iroh_send, mut iroh_recv) = conn.open_bi().await?;
        info!("sending file header");

        let file_send_header = FileSendHeader { file_name, file_size, version: 2, volume, target_dir, can_overwrite };
        file_send_header.encode(&mut iroh_send).await?;
        info!("sent file header, waiting for ack");

        let file_send_header_ack = Ack::decode(&mut iroh_recv).await?;
        info!("ack received, {}", file_send_header_ack.msg);
        if file_send_header_ack.ack != 0 {
            info!("Bad ack from server, {}", file_send_header_ack.msg);
            anyhow::bail!("Server responded with error ack: {:?}", file_send_header_ack.msg);
        }
        let bytes = copy_with_progress(&mut reader, &mut iroh_send, file_size).await?;
        println!("Finished sending file: {}", indicatif::HumanBytes(bytes));
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

pub async fn run_tcp_client(typ: ProxyType, bind_addr: String, tunnel_target: Option<(String, u16)>, server_node_id_str: Option<String>, name: Option<String>) -> Result<()> {
    println!("Mode: client {}", typ.label());
    println!("Listening on: {bind_addr}");
    if let Some((remote_host, remote_port)) = &tunnel_target {
        println!("Forwarding to: {remote_host}:{remote_port}");
    }

    let (node_name, node_id_raw): (String, String) = resolve_node_id(server_node_id_str, name)?;
    let server_node_id = EndpointId::from_str(&node_id_raw).with_context(|| "Could not parse server node id")?;

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
        println!("Connected to \"{node_name}\" [{server_node_id}]");

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
                                let (remote_host, remote_port) = tunnel_target.expect("tunnel target set for ProxyType::Tunnel");
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
    println!("Connected to iroh server {server_node_id}");
 
    let (mut iroh_send, iroh_recv) = conn.open_bi().await?;

    let proxy_header = ProxyHeaderV1 { version: 1, host, port };
    proxy_header.encode(&mut iroh_send).await?;

    let (tcp_read, tcp_write) = tcp.into_split();
    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("Connection to {}:{} via iroh server closed", proxy_header.host, proxy_header.port);
    Ok(())
}
