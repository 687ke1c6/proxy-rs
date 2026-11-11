use std::io::IsTerminal;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use iroh::{Endpoint, EndpointId, address_lookup::{self, PkarrPublisher}, endpoint::{ConnectWithOptsError, Connection, ConnectionError, RecvStream, SendStream, TransportErrorCode, presets}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

use crate::{protocols::{ack::Ack, codec::StreamCodec, file_send::{alpn::FILE_ALPN, file_send_header::FileSendHeader}, list_volumes::{alpn::LIST_VOLUMES_ALPN, list_volumes_header::{ListVolumesRequest, ListVolumesResponse}}, ping::{alpn::PING_ALPN, ping_header::PingHeader}, proxy::{alpn::TCP_PROXY_ALPN, proxy_header::{ACK_CONNECT_FAILED, ACK_NOT_ALLOWED, ProxyHeader}}, rsync::{alpn::RSYNC_ALPN, rsync_header::RsyncHeader}}, stream_helpers::{PathKindWatch, close_reason_suffix, path_kind_suffix, proxy_process_streams, proxy_streams}};
use crate::BANNER;
use crate::config_dir::banner_suffix;
use crate::errors::display_chain;
use crate::signals::until_signal;
use crate::socks5;
use crate::http;
use crate::client::client_helpers::{resolve_node_id, server_label};
use crate::client::spinner::Spinner;
use crate::identity::{CLIENT_KEY_FILE, load_or_create_secret_key, short_id};

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

/// The client's resolved intent for where a sent file should land on the server. The
/// file always keeps its local basename — `--target` only ever picks a destination
/// directory, never a new name, so it's unambiguous what it does.
struct FileTarget {
    /// Volume name, e.g. "home"; empty means "let the server pick a default".
    volume: String,
    /// `/`-separated relative directory within the volume; empty means the volume root.
    target_dir: String,
}

/// Parses a `--target` spec of the form `volume[/dir...]`. Examples: `home`,
/// `home/subfolder`, `home/subfolder/deeper`.
fn parse_target(spec: &str) -> Result<FileTarget> {
    let (volume, rest) = spec.split_once('/').unwrap_or((spec, ""));
    anyhow::ensure!(!volume.is_empty(), "invalid --target {spec:?}: must start with a volume name");

    let target_dir = rest.trim_end_matches('/').to_string();
    Ok(FileTarget { volume: volume.to_string(), target_dir })
}

/// The server's client allowlist refused this client. Unlike the server being unreachable,
/// retrying won't help, so it ends the client.
#[derive(Debug)]
struct NotAllowed(EndpointId);

impl std::fmt::Display for NotAllowed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "server refused this client: node id {} is not in its allowlist; ask the server operator \
             to add it with --allow or to their authorized-clients file (`proxy-rs client whoami` prints the full id)",
            short_id(self.0)
        )
    }
}

impl std::error::Error for NotAllowed {}

/// `Endpoint::connect`, but explains two common failures in one line:
/// - no address found for the server (its node id isn't published in DNS): it's not online;
///   iroh's own error lists every failed lookup, over several lines.
/// - the server refusing `alpn` (TLS alert 120, "no application protocol"): protocol versions
///   live only in ALPN strings, so this is what a client/server version mismatch looks like,
///   as well as a server feature that's off.
async fn connect(endpoint: &Endpoint, server_node_id: EndpointId, alpn: &[u8]) -> Result<Connection> {
    endpoint.connect(server_node_id, alpn).await.map_err(|e| {
        let mut chain = std::iter::successors(Some(&e as &(dyn std::error::Error + 'static)), |e| e.source());
        if chain.clone().any(|e| matches!(e.downcast_ref::<ConnectWithOptsError>(), Some(ConnectWithOptsError::NoAddress { .. }))) {
            debug!("Address lookup for {server_node_id} failed: {e:#}");
            return anyhow::anyhow!("server [{}] is not online (no address found for its node id)", short_id(server_node_id));
        }
        let refused_alpn = chain
            .any(|e| matches!(
                e.downcast_ref::<ConnectionError>(),
                Some(ConnectionError::ConnectionClosed(close)) if close.error_code == TransportErrorCode::crypto(120)
            ));
        if !refused_alpn {
            return e.into();
        }
        let alpn = String::from_utf8_lossy(alpn);
        let hint = if alpn.starts_with("proxy-rs/ping/") {
            // Ping is always enabled, so this can only be a version mismatch.
            "the server runs an incompatible proxy-rs version; upgrade the older side".to_string()
        } else {
            "the feature is not enabled on the server, or it runs an incompatible proxy-rs version".to_string()
        };
        anyhow::anyhow!("server does not accept protocol {alpn}: {hint}")
    })
}

/// Pings the server and returns the ping connection, open. Callers hold it for as long as
/// they run: the server prints `Client connected`/`Client disconnected` as it opens and closes.
async fn ping_server(endpoint: &Endpoint, server_node_id: EndpointId) -> Result<Connection> {
    const MSG: &str = "ping";
    let conn = connect(endpoint, server_node_id, PING_ALPN).await?;
    let result: Result<()> = async {
        let (mut send, mut recv) = conn.open_bi().await.with_context(||"Could not get bi_directional channel")?;
        PingHeader { msg: MSG.to_string() }.encode(&mut send).await?;
        send.finish()?;
        let pong = PingHeader::decode(&mut recv).await.with_context(||"Couldn't receive ping header")?;
        anyhow::ensure!(pong.msg == MSG, "ping/pong message mismatch: got {:?}", pong.msg);
        Ok(())
    }.await;

    // The server's client allowlist (iroh `AccessLimit`) closes refused connections with
    // reason "not allowed"; turn that into something the user can act on.
    if result.is_err()
        && let Some(ConnectionError::ApplicationClosed(close)) = conn.close_reason()
        && &close.reason[..] == b"not allowed"
    {
        return Err(NotAllowed(endpoint.id()).into());
    }
    result.map(|()| conn)
}

/// Keeps a ping connection to the server open for as long as a long-running client runs,
/// printing when it connects and when the server is lost, and reconnecting until it's back.
/// Only returns on a fatal error: the server doesn't allow this client.
///
/// `session` is an already-open ping connection to start with (else it pings first). Without
/// `retry`, any failure ends it with an error, ncat-style: a failed ping, or the session closing
/// (`Disconnected from ...`). With `retry` it only returns on `NotAllowed`.
///
/// `spinner` is the caller's `Connecting to ...` spinner, kept running until connected; while
/// disconnected (with `retry`) a `Reconnecting to ...` one runs, with `Waiting for` printed above it.
async fn maintain_session(endpoint: &Endpoint, server_node_id: EndpointId, server: &str, retry: bool, mut session: Option<Connection>, spinner: Spinner) -> Result<()> {
    let mut spinner = Some(spinner);
    const RETRY: Duration = Duration::from_secs(2);
    // Print the first failed attempt only, not every retry.
    let mut waiting = false;
    loop {
        let attempt = match session.take() {
            Some(conn) => Ok(conn),
            None => ping_server(endpoint, server_node_id).await,
        };
        // A failed attempt (only retried with `retry`) shows the next frame set, so each
        // retry is visible; a fresh `Reconnecting` spinner already starts on a new one.
        if attempt.is_err()
            && let Some(spinner) = &spinner
        {
            spinner.next_frames();
        }
        match attempt {
            Ok(conn) => {
                if let Some(spinner) = spinner.take() {
                    spinner.finish();
                }
                let paths = PathKindWatch::new(&conn);
                println!("Connected to {server}{}", paths.suffix());
                let reason = tokio::select! {
                    reason = conn.closed() => reason,
                    () = paths.run(|kind| println!("Connection to {server} now {kind}")) => unreachable!(),
                };
                let disconnected = format!("Disconnected from {server}{}", close_reason_suffix(&reason));
                if !retry {
                    anyhow::bail!(disconnected);
                }
                println!("{disconnected}");
                // Failed attempts after this print `Waiting for` once, then just spin.
                spinner = Some(Spinner::start(format!("Reconnecting to {server}")));
                waiting = false;
            }
            Err(e) if e.is::<NotAllowed>() || !retry => return Err(e),
            Err(e) if !waiting => {
                let line = format!("Waiting for {server}: {}", display_chain(&e));
                // The spinner keeps saying `Connecting to`/`Reconnecting to`: that's still what
                // it's doing; this line above it says why it's taking a while.
                match &spinner {
                    Some(spinner) => spinner.println(line),
                    None => println!("{line}"),
                }
                waiting = true;
            }
            Err(e) => info!("Still waiting for {server}: {}", display_chain(&e)),
        }
        tokio::time::sleep(RETRY).await;
    }
}

pub async fn run_list_volumes(server_node_id_str: Option<String>, name: Option<String>) -> Result<()> {
    println!("{BANNER} client volumes{}", banner_suffix());

    let (node_name, server_node_id) = resolve_node_id(server_node_id_str, name)?;
    let spinner = Spinner::start(format!("Connecting to {}", server_label(node_name.as_deref(), server_node_id)));

    let endpoint = Arc::new(
        Endpoint::builder(presets::N0)
            .secret_key(load_or_create_secret_key(CLIENT_KEY_FILE)?)
            .address_lookup(PkarrPublisher::n0_dns())
            .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
            .bind()
            .await?
    );

    // Getting online is inside `until_signal` too, so Ctrl-C while it waits is reported.
    let result = until_signal(async {
        endpoint.online().await;
        let _session = ping_server(&endpoint, server_node_id).await?;
        let conn = connect(&endpoint, server_node_id, LIST_VOLUMES_ALPN).await?;
        spinner.finish();
        println!("Connected to {}{}", server_label(node_name.as_deref(), server_node_id), path_kind_suffix(&conn));

        let (mut iroh_send, mut iroh_recv) = conn.open_bi().await?;
        ListVolumesRequest {}.encode(&mut iroh_send).await?;
        iroh_send.finish()?;

        let response = ListVolumesResponse::decode(&mut iroh_recv).await?;
        if response.volumes.is_empty() {
            println!("(server exposes no volumes)");
        }
        for volume in response.volumes {
            println!("{}:{}", volume.name, volume.path);
        }
        Ok(())
    }).await;

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
    println!("{BANNER} client file {file_path} -> {}{}", target.as_deref().unwrap_or("(default volume)"), banner_suffix());

    let full_path = std::fs::canonicalize(&file_path).with_context(|| format!("Failed to canonicalize path: {file_path}"))?;
    let metadata = tokio::fs::metadata(&full_path).await?;
    let file_size = metadata.len();

    info!("{}, {file_size} bytes", full_path.display());

    let path = std::path::Path::new(&full_path);

    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap().to_string();

    let target = target.as_deref().map(parse_target).transpose()?;
    let (volume, target_dir) = match target {
        Some(t) => (t.volume, t.target_dir),
        None => (String::new(), String::new()),
    };

    let mut reader = tokio::fs::File::open(&full_path).await?;

    let (node_name, server_node_id) = resolve_node_id(server_node_id_str, name)?;
    let spinner = Spinner::start(format!("Connecting to {}", server_label(node_name.as_deref(), server_node_id)));

    let endpoint = Arc::new(
        Endpoint::builder(presets::N0)
            .secret_key(load_or_create_secret_key(CLIENT_KEY_FILE)?)
            .address_lookup(PkarrPublisher::n0_dns())
            .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
            .bind()
            .await?
    );

    // Getting online is inside `until_signal` too, so Ctrl-C while it waits is reported.
    let result = until_signal(async {
        endpoint.online().await;
        let _session = ping_server(&endpoint, server_node_id).await?;
        let conn = connect(&endpoint, server_node_id, FILE_ALPN).await?;
        spinner.finish();
        println!("Connected to {}{}", server_label(node_name.as_deref(), server_node_id), path_kind_suffix(&conn));

        let (mut iroh_send, mut iroh_recv) = conn.open_bi().await?;
        info!("sending file header");

        let file_send_header = FileSendHeader { file_name, file_size, volume, target_dir, can_overwrite };
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
    }).await;

    info!("shutting down p2p endpoint");
    endpoint.close().await;

    result
}

/// rsync `-e`/`--rsh` transport: rsync spawns this as `sync-rsh <host> rsync --server
/// <flags> . <volume[/dir]>`, exactly as it would spawn `ssh`, and expects our
/// stdin/stdout to become a duplex pipe to that remote command. The placeholder host
/// and literal `rsync` token are dropped (the server is picked by `-n`/`--name`), the
/// rest is forwarded as `RsyncHeader.argv`, and once acked stdin/stdout are spliced
/// onto the iroh stream.
///
/// stdout *is* the rsync protocol stream here, so nothing in this mode may print to
/// it — all diagnostics go to stderr, which rsync passes through to the user.
pub async fn run_sync_rsh(argv: Vec<String>, server_node_id_str: Option<String>, name: Option<String>) -> Result<()> {
    // stdin belongs to rsync, so the interactive node-id menu can't work here.
    anyhow::ensure!(
        server_node_id_str.is_some() || name.is_some(),
        "sync-rsh needs -n/--node-id or --name (stdin is rsync's pipe, so it can't prompt)"
    );

    let mut rest = argv.into_iter().skip(1); // placeholder host
    anyhow::ensure!(
        rest.next().as_deref() == Some("rsync"),
        "sync-rsh expected rsync's remote command (\"rsync --server ...\") after the host"
    );
    let argv: Vec<String> = rest.collect();
    info!("sync-rsh argv: {argv:?}");

    let (node_name, server_node_id) = resolve_node_id(server_node_id_str, name)?;
    info!("Connecting to {}...", server_label(node_name.as_deref(), server_node_id));

    let endpoint = Arc::new(
        Endpoint::builder(presets::N0)
            .secret_key(load_or_create_secret_key(CLIENT_KEY_FILE)?)
            .address_lookup(PkarrPublisher::n0_dns())
            .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
            .bind()
            .await?
    );

    info!("creating endpoint");
    endpoint.online().await;

    let result = async {
        let _session = ping_server(&endpoint, server_node_id).await?;
        let conn = connect(&endpoint, server_node_id, RSYNC_ALPN).await?;
        info!("Connected to {}", server_label(node_name.as_deref(), server_node_id));

        let (mut iroh_send, mut iroh_recv) = conn.open_bi().await?;
        RsyncHeader { argv }.encode(&mut iroh_send).await?;

        let ack = Ack::decode(&mut iroh_recv).await?;
        if ack.ack != 0 {
            anyhow::bail!("Server responded with error ack: {:?}", ack.msg);
        }

        proxy_process_streams(iroh_recv, iroh_send, tokio::io::stdin(), tokio::io::stdout()).await
    }.await;

    info!("shutting down p2p endpoint");
    endpoint.close().await;

    result
}

pub async fn run_tcp_client(typ: ProxyType, bind_addr: String, tunnel_target: Option<(String, u16)>, server_node_id_str: Option<String>, name: Option<String>, retry: bool) -> Result<()> {
    let retry_flag = if retry { " retry" } else { "" };
    match &tunnel_target {
        Some((remote_host, remote_port)) => println!("{BANNER} client {} {bind_addr} -> {remote_host}:{remote_port}{retry_flag}{}", typ.label(), banner_suffix()),
        None => println!("{BANNER} client {} {bind_addr}{retry_flag}{}", typ.label(), banner_suffix()),
    }

    let (node_name, server_node_id) = resolve_node_id(server_node_id_str, name)?;
    let server = server_label(node_name.as_deref(), server_node_id);
    // Started before the endpoint comes up: getting online is part of the wait.
    let spinner = Spinner::start(format!("Connecting to {server}"));

    let endpoint = Arc::new(
        Endpoint::builder(presets::N0)
            .secret_key(load_or_create_secret_key(CLIENT_KEY_FILE)?)
            .address_lookup(PkarrPublisher::n0_dns())
            .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
            .bind()
            .await?,
    );
    // Getting online is inside `until_signal` too, so Ctrl-C while it waits is reported.
    // A signal ends the loop below (and drops the session, clearing its spinner).
    let result = until_signal(async {
        endpoint.online().await;
        info!("Client NodeId: {}", short_id(endpoint.id()));

        // Without --retry, the server must be there before we listen (and we exit when it goes).
        // With it, listen right away: proxied sockets open their own connections, so they don't
        // wait on the session, which reconnects in the background whenever the server is lost.
        let first_session = if retry { None } else { Some(ping_server(&endpoint, server_node_id).await?) };
        let listener = TcpListener::bind(&bind_addr).await?;
        info!("Listening for {:?} connections on {bind_addr}", typ);
        let session = maintain_session(&endpoint, server_node_id, &server, retry, first_session, spinner);
        tokio::pin!(session);

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
                            error!("Proxy error from {peer_addr}: {}", display_chain(&e));
                        }
                    });
                }
                result = &mut session => return result,
            }
        }
    }).await;

    endpoint.close().await;
    result
}

/// Result of asking the server to open a TCP connection on our behalf.
enum ProxyOpen {
    /// The server connected to the target; keep `Connection` alive while streaming.
    Connected(Connection, SendStream, RecvStream),
    /// The server refused (`ACK_NOT_ALLOWED`, `ACK_CONNECT_FAILED`, ...); the connection is closed.
    Refused(Ack),
}

/// Opens a `proxy-rs/tcp/3` stream to `host:port` via the server and waits for its verdict.
/// `Err` means the server itself couldn't be reached or broke protocol.
async fn open_proxy_stream(endpoint: &Endpoint, server_node_id: EndpointId, host: &str, port: u16) -> Result<ProxyOpen> {
    info!("Connecting to iroh server {}", short_id(server_node_id));
    let conn = connect(endpoint, server_node_id, TCP_PROXY_ALPN).await?;
    let (mut iroh_send, mut iroh_recv) = conn.open_bi().await?;

    let proxy_header = ProxyHeader { host: host.to_string(), port };
    proxy_header.encode(&mut iroh_send).await?;

    let ack = Ack::decode(&mut iroh_recv).await?;
    if ack.ack != 0 {
        conn.close(0u32.into(), b"refused");
        return Ok(ProxyOpen::Refused(ack));
    }
    Ok(ProxyOpen::Connected(conn, iroh_send, iroh_recv))
}

async fn handle_http(
    mut tcp: TcpStream,
    endpoint: Arc<Endpoint>,
    server_node_id: EndpointId,
) -> Result<()> {
    let http::ProxyRequest { host, port, is_connect, preamble } = http::handshake(&mut tcp).await?;
    info!("HTTP proxy -> {}:{}", host, port);

    let opened = match open_proxy_stream(&endpoint, server_node_id, &host, port).await {
        Ok(opened) => opened,
        Err(e) => {
            http::respond_error(&mut tcp, "502 Bad Gateway", "proxy-rs: could not reach the iroh server\n").await.ok();
            return Err(e);
        }
    };
    let (_conn, mut iroh_send, iroh_recv) = match opened {
        ProxyOpen::Connected(conn, send, recv) => (conn, send, recv),
        ProxyOpen::Refused(ack) => {
            let status = if ack.ack == ACK_NOT_ALLOWED { "403 Forbidden" } else { "502 Bad Gateway" };
            http::respond_error(&mut tcp, status, &format!("proxy-rs: {}\n", ack.msg)).await?;
            anyhow::bail!("server refused {host}:{port}: {}", ack.msg);
        }
    };

    if is_connect {
        http::respond_connected(&mut tcp).await?;
    } else {
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

    // No local protocol to report errors through: a refusal just closes the local socket
    // (by dropping `tcp`) and surfaces as a logged error.
    let (_conn, iroh_send, iroh_recv) = match open_proxy_stream(&endpoint, server_node_id, &remote_host, remote_port).await? {
        ProxyOpen::Connected(conn, send, recv) => (conn, send, recv),
        ProxyOpen::Refused(ack) => anyhow::bail!("server refused {remote_host}:{remote_port}: {}", ack.msg),
    };

    let (tcp_read, tcp_write) = tcp.into_split();
    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("Tunnel connection to {}:{} via iroh server closed", remote_host, remote_port);
    Ok(())
}

async fn handle_socks5(
    mut tcp: TcpStream,
    endpoint: Arc<Endpoint>,
    server_node_id: EndpointId,
) -> Result<()> {
    let (host, port) = socks5::handshake(&mut tcp).await?;
    info!("SOCKS5 CONNECT -> {}:{}", host, port);

    let opened = match open_proxy_stream(&endpoint, server_node_id, &host, port).await {
        Ok(opened) => opened,
        Err(e) => {
            socks5::reply(&mut tcp, socks5::REP_GENERAL_FAILURE).await.ok();
            return Err(e);
        }
    };
    let (_conn, iroh_send, iroh_recv) = match opened {
        ProxyOpen::Connected(conn, send, recv) => (conn, send, recv),
        ProxyOpen::Refused(ack) => {
            let rep = match ack.ack {
                ACK_NOT_ALLOWED => socks5::REP_NOT_ALLOWED,
                ACK_CONNECT_FAILED => socks5::REP_CONNECTION_REFUSED,
                _ => socks5::REP_GENERAL_FAILURE,
            };
            socks5::reply(&mut tcp, rep).await?;
            anyhow::bail!("server refused {host}:{port}: {}", ack.msg);
        }
    };
    socks5::reply(&mut tcp, socks5::REP_SUCCEEDED).await?;

    let (tcp_read, tcp_write) = tcp.into_split();
    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;

    warn!("Connection to {}:{} via iroh server closed", host, port);
    Ok(())
}
