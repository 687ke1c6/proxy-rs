use std::sync::Arc;
use std::time::Duration;
use anyhow::{Context, Result};
use iroh::{Endpoint, EndpointId, address_lookup::{self, PkarrPublisher}, endpoint::{ConnectWithOptsError, Connection, ConnectionError, RecvStream, SendStream, TransportErrorCode, presets}};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};

use crate::BANNER;
use crate::client::client_helpers::{resolve_node_id, server_label};
use crate::client::spinner::Spinner;
use crate::config::{banner_suffix, load_secret_key};
use crate::errors::display_chain;
use crate::identity::short_id;
use crate::protocols::{ack::Ack, codec::StreamCodec, ping::{alpn::PING_ALPN, ping_header::PingHeader}, stream::{alpn::STREAM_ALPN, stream_header::Open}};
use crate::signals::until_signal;
use crate::stream_helpers::{PathKindWatch, close_reason_suffix};

/// The server's client allowlist refused this client. Unlike the server being unreachable,
/// retrying won't help, so it ends the client.
#[derive(Debug)]
struct NotAllowed(EndpointId);

impl std::fmt::Display for NotAllowed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "server refused this client: node id {} is not in its allowlist; ask the server operator \
             to add it with --allow or to [clients] in their config file (`proxy-rs whoami` prints the full id)",
            short_id(self.0)
        )
    }
}

impl std::error::Error for NotAllowed {}

/// `Endpoint::connect`, but explains two common failures in one line:
/// - no address found for the server (its node id isn't published in DNS): it's not online;
///   iroh's own error lists every failed lookup, over several lines.
/// - the server refusing `alpn` (TLS alert 120, "no application protocol"): protocol versions
///   live only in ALPN strings, so this is what a client/server version mismatch looks like.
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
        // Both protocols are always on, so a refusal can only be a version mismatch.
        let alpn = String::from_utf8_lossy(alpn);
        anyhow::anyhow!("server does not accept protocol {alpn}: it runs an incompatible proxy-rs version; upgrade the older side")
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

/// The client's iroh endpoint, with this node's key so the server's allowlist can recognize it.
async fn bind_endpoint() -> Result<Arc<Endpoint>> {
    Ok(Arc::new(
        Endpoint::builder(presets::N0)
            .secret_key(load_secret_key()?)
            .address_lookup(PkarrPublisher::n0_dns())
            .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
            .bind()
            .await?,
    ))
}

/// Opens a stream to the server and sends the `Open` frame. The server's `Ack` is still to
/// be read from the returned `RecvStream`; keep the `Connection` alive while streaming.
async fn open_stream(endpoint: &Endpoint, server_node_id: EndpointId, args: Vec<String>) -> Result<(Connection, SendStream, RecvStream)> {
    let conn = connect(endpoint, server_node_id, STREAM_ALPN).await?;
    let (mut send, recv) = conn.open_bi().await?;
    Open { args }.encode(&mut send).await?;
    Ok((conn, send, recv))
}

/// Reads the server's `Ack` to our `Open`; a refusal becomes an error with its reason.
async fn read_ack(recv: &mut RecvStream) -> Result<()> {
    let ack = Ack::decode(recv).await.context("no answer from the server")?;
    anyhow::ensure!(ack.ack == 0, "server refused the connection: {}", ack.msg);
    Ok(())
}

/// `--listen`: binds a local address and forwards every connection to it to the server, as
/// is. What the bytes mean (SOCKS5, HTTP proxy, a tunnel to some host) is up to the server.
pub async fn run_listen(bind_addr: String, server_node_id_str: Option<String>, name: Option<String>, retry: bool) -> Result<()> {
    let retry_flag = if retry { " retry" } else { "" };
    println!("{BANNER} listen {bind_addr}{retry_flag}{}", banner_suffix());

    let (node_name, server_node_id) = resolve_node_id(server_node_id_str, name)?;
    let server = server_label(node_name.as_deref(), server_node_id);
    // Started before the endpoint comes up: getting online is part of the wait.
    let spinner = Spinner::start(format!("Connecting to {server}"));

    let endpoint = bind_endpoint().await?;
    // Getting online is inside `until_signal` too, so Ctrl-C while it waits is reported.
    // A signal ends the loop below (and drops the session, clearing its spinner).
    let result = until_signal(async {
        endpoint.online().await;
        info!("Client NodeId: {}", short_id(endpoint.id()));

        // Without --retry, the server must be there before we listen (and we exit when it goes).
        // With it, listen right away: forwarded sockets open their own connections, so they don't
        // wait on the session, which reconnects in the background whenever the server is lost.
        let first_session = if retry { None } else { Some(ping_server(&endpoint, server_node_id).await?) };
        let listener = TcpListener::bind(&bind_addr).await.with_context(|| format!("failed to listen on {bind_addr}"))?;
        info!("Listening on {bind_addr}");
        let session = maintain_session(&endpoint, server_node_id, &server, retry, first_session, spinner);
        tokio::pin!(session);

        loop {
            tokio::select! {
                result = listener.accept() => {
                    let (tcp_stream, peer_addr) = result?;
                    info!("Accepted connection from {peer_addr}");

                    let ep = endpoint.clone();
                    tokio::spawn(async move {
                        if let Err(e) = forward_socket(tcp_stream, &ep, server_node_id).await {
                            error!("Connection from {peer_addr}: {}", display_chain(&e));
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

/// Forwards one accepted socket to the server. A refusal (the `Ack`) has no local protocol
/// to be reported through: it closes the socket (by dropping it) and surfaces as an error.
async fn forward_socket(tcp: TcpStream, endpoint: &Endpoint, server_node_id: EndpointId) -> Result<()> {
    let (_conn, send, recv) = open_stream(endpoint, server_node_id, Vec::new()).await?;
    let (tcp_read, tcp_write) = tcp.into_split();
    forward_after_ack(recv, send, tcp_read, tcp_write).await
}

/// Like `stream_helpers::proxy_streams` (TCP-style: half-close propagates, done when both
/// directions are), but local bytes are sent to the server right away, without waiting for
/// its `Ack` first, which would cost every connection a round trip. A refusal ends both
/// directions.
async fn forward_after_ack<R, W>(mut iroh_recv: RecvStream, mut iroh_send: SendStream, mut local_read: R, mut local_write: W) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let up = async {
        tokio::io::copy(&mut local_read, &mut iroh_send).await?;
        iroh_send.finish()?;
        anyhow::Ok(())
    };
    let down = async {
        read_ack(&mut iroh_recv).await?;
        tokio::io::copy(&mut iroh_recv, &mut local_write).await?;
        local_write.shutdown().await?;
        anyhow::Ok(())
    };
    tokio::try_join!(up, down)?;
    // The caller drops the `Connection` next, which would discard data still in flight.
    let _ = iroh_send.stopped().await;
    Ok(())
}

/// Splices stdin/stdout onto the stream, ssh-style: it's done when the server's side ends.
/// EOF on stdin only half-closes (the server sees EOF, its reply still comes back), and
/// stdin staying open doesn't keep us running: rsync only closes it after we exit.
async fn pipe_stdio(mut iroh_recv: RecvStream, mut iroh_send: SendStream) -> Result<()> {
    let (mut stdin, mut stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let up = async {
        tokio::io::copy(&mut stdin, &mut iroh_send).await?;
        iroh_send.finish()?;
        std::future::pending::<Result<()>>().await
    };
    let down = async {
        tokio::io::copy(&mut iroh_recv, &mut stdout).await?;
        stdout.flush().await?;
        anyhow::Ok(())
    };
    tokio::select! {
        result = up => result,
        result = down => result,
    }
}

/// `--pipe`: forwards stdin/stdout to the server over one stream, for use as another
/// program's transport: rsync's `-e` command (against an `rsync` server; rsync appends its
/// remote command, forwarded as `args`) or ssh's `ProxyCommand` (against a `tunnel` server).
///
/// stdout *is* the data stream here, so nothing in this mode may print to it: all
/// diagnostics go to stderr, which the parent program passes through to the user.
pub async fn run_pipe(args: Vec<String>, server_node_id_str: Option<String>, name: Option<String>) -> Result<()> {
    // stdin belongs to the parent program, so the interactive node-id menu can't work here.
    anyhow::ensure!(
        server_node_id_str.is_some() || name.is_some(),
        "--pipe needs -n/--node-id or --name (stdin is the data stream, so it can't prompt)"
    );
    info!("pipe args: {args:?}");

    let (node_name, server_node_id) = resolve_node_id(server_node_id_str, name)?;
    info!("Connecting to {}...", server_label(node_name.as_deref(), server_node_id));

    let endpoint = bind_endpoint().await?;
    endpoint.online().await;

    let result = async {
        let _session = ping_server(&endpoint, server_node_id).await?;
        let (_conn, send, mut recv) = open_stream(&endpoint, server_node_id, args).await?;
        info!("Connected to {}", server_label(node_name.as_deref(), server_node_id));
        read_ack(&mut recv).await?;
        pipe_stdio(recv, send).await
    }.await;

    info!("shutting down p2p endpoint");
    endpoint.close().await;

    result
}
