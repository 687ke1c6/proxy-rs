use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use iroh::{endpoint::{Connection, RecvStream, SendStream}, protocol::{AcceptError, ProtocolHandler}};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::info;

use crate::errors::display_chain;
use crate::identity::short_id;
use crate::protocols::stream::rsync::{self, Volumes};
use crate::protocols::stream::stream_header::{ACK_CONNECT_FAILED, ACK_REFUSED, Open};
use crate::protocols::stream::target_policy::TargetPolicy;
use crate::protocols::{ack::Ack, codec::StreamCodec};
use crate::stream_helpers::proxy_streams;
use crate::{http, socks5};

/// How long a client gets to send its `Open` frame, and then its SOCKS5/HTTP request.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// How long to wait for the client to hang up after a final refusal, so the refusal isn't
/// lost to the connection being torn down when the handler returns.
const LINGER: Duration = Duration::from_secs(10);

/// What the server does with every client stream. The server runs exactly one mode; the
/// client doesn't choose (or know) it.
#[derive(Debug)]
pub enum Mode {
    /// Speak SOCKS5 to the client, connect to the target it asks for if `-t` allows it.
    Socks5(TargetPolicy),
    /// Speak HTTP proxy (CONNECT, or plain absolute-URL requests) likewise.
    Http(TargetPolicy),
    /// Connect every stream to one fixed target.
    Tunnel { host: String, port: u16 },
    /// Run `rsync --server` into a volume, for a client used as rsync's `-e` command.
    Rsync(Volumes),
}

impl Mode {
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Socks5(_) => "socks5",
            Mode::Http(_) => "http",
            Mode::Tunnel { .. } => "tunnel",
            Mode::Rsync(_) => "rsync",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamServerProtocol {
    pub mode: Arc<Mode>,
}

impl ProtocolHandler for StreamServerProtocol {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        self.handle(connection).await
            .map_err(|e| AcceptError::from_boxed(e.into()))
    }
}

/// Finishes the stream and waits (bounded) for the client to hang up.
async fn linger(mut send: SendStream, connection: &Connection) {
    let _ = send.finish();
    let _ = timeout(LINGER, connection.closed()).await;
}

/// Sends a failure `Ack` as the stream's last message.
pub async fn reject(mut send: SendStream, connection: &Connection, code: u8, msg: String) -> anyhow::Result<()> {
    Ack::no_ack(code, Some(msg)).encode(&mut send).await?;
    linger(send, connection).await;
    Ok(())
}

enum Dial {
    Connected(TcpStream),
    NotAllowed,
    Failed(std::io::Error),
}

/// Connects to `host:port` if `policy` (when there is one) allows it, printing one line per
/// attempt, like `Client connected:`, so every proxied connection is visible at the default
/// (error-only) log level. The target is shown as given: no DNS lookup.
async fn dial(client: &str, policy: Option<&TargetPolicy>, host: &str, port: u16) -> Dial {
    let target = format!("{host}:{port}");
    if policy.is_some_and(|policy| !policy.allows(host, port)) {
        println!("tcp [{client}] -> {target} refused: not allowed by -t");
        return Dial::NotAllowed;
    }
    // (host, port) rather than the formatted string, so bare IPv6 hosts resolve correctly.
    match TcpStream::connect((host, port)).await {
        Ok(tcp) => {
            println!("tcp [{client}] -> {target}");
            Dial::Connected(tcp)
        }
        Err(e) => {
            println!("tcp [{client}] -> {target} failed: {e}");
            Dial::Failed(e)
        }
    }
}

/// Splices the client's stream onto the connected target until both directions are done.
async fn pipe(recv: RecvStream, send: SendStream, tcp: TcpStream) {
    let (tcp_read, tcp_write) = tcp.into_split();
    if let Err(e) = proxy_streams(recv, send, tcp_read, tcp_write).await {
        info!("Proxy stream ended with error: {}", display_chain(&e));
    }
}

impl StreamServerProtocol {
    async fn handle(&self, connection: Connection) -> anyhow::Result<()> {
        let client = short_id(connection.remote_id());
        let (send, mut recv) = connection.accept_bi().await?;
        let open = timeout(HANDSHAKE_TIMEOUT, Open::decode(&mut recv))
            .await
            .context("timed out waiting for the client's open frame")??;

        match &*self.mode {
            Mode::Rsync(volumes) => rsync::handle(volumes, &connection, send, recv, &open.args).await,
            mode if !open.args.is_empty() => {
                let msg = format!("this server runs in {} mode, which takes no --pipe arguments", mode.label());
                reject(send, &connection, ACK_REFUSED, msg).await
            }
            Mode::Tunnel { host, port } => self.tunnel(&connection, send, recv, &client, host, *port).await,
            Mode::Socks5(policy) => self.socks5(&connection, send, recv, &client, policy).await,
            Mode::Http(policy) => self.http(&connection, send, recv, &client, policy).await,
        }
    }

    /// Acks once the fixed target is connected, so the client can report a dead target.
    async fn tunnel(&self, connection: &Connection, mut send: SendStream, recv: RecvStream, client: &str, host: &str, port: u16) -> anyhow::Result<()> {
        let tcp = match dial(client, None, host, port).await {
            Dial::Connected(tcp) => tcp,
            Dial::NotAllowed => unreachable!("tunnel mode has no target policy"),
            Dial::Failed(e) => {
                return reject(send, connection, ACK_CONNECT_FAILED, format!("connect to {host}:{port} failed: {e}")).await;
            }
        };
        Ack::ack().encode(&mut send).await?;
        pipe(recv, send, tcp).await;
        Ok(())
    }

    /// Acks at once: from here the stream is the SOCKS5 conversation, and refusals are
    /// SOCKS5 replies (`0x02` not allowed, `0x05` connect failed).
    async fn socks5(&self, connection: &Connection, mut send: SendStream, recv: RecvStream, client: &str, policy: &TargetPolicy) -> anyhow::Result<()> {
        Ack::ack().encode(&mut send).await?;
        let mut stream = tokio::io::join(recv, send);
        let request = timeout(HANDSHAKE_TIMEOUT, socks5::handshake(&mut stream))
            .await
            .map_err(|_| anyhow::anyhow!("SOCKS5 handshake timed out"))
            .and_then(|request| request);
        let (host, port) = match request {
            Ok(request) => request,
            Err(e) => {
                // The handshake may have written its own refusal; let it arrive.
                linger(stream.into_inner().1, connection).await;
                return Err(e.context(format!("SOCKS5 handshake with [{client}] failed")));
            }
        };

        let refusal = match dial(client, Some(policy), &host, port).await {
            Dial::Connected(tcp) => {
                socks5::reply(&mut stream, socks5::REP_SUCCEEDED).await?;
                let (recv, send) = stream.into_inner();
                pipe(recv, send, tcp).await;
                return Ok(());
            }
            Dial::NotAllowed => socks5::REP_NOT_ALLOWED,
            Dial::Failed(_) => socks5::REP_CONNECTION_REFUSED,
        };
        socks5::reply(&mut stream, refusal).await?;
        linger(stream.into_inner().1, connection).await;
        Ok(())
    }

    /// Acks at once: from here the stream is the HTTP proxy conversation, and refusals are
    /// HTTP responses (`403` not allowed, `502` connect failed).
    async fn http(&self, connection: &Connection, mut send: SendStream, recv: RecvStream, client: &str, policy: &TargetPolicy) -> anyhow::Result<()> {
        Ack::ack().encode(&mut send).await?;
        let mut stream = tokio::io::join(recv, send);
        let request = timeout(HANDSHAKE_TIMEOUT, http::handshake(&mut stream))
            .await
            .map_err(|_| anyhow::anyhow!("HTTP proxy request timed out"))
            .and_then(|request| request);
        let http::ProxyRequest { host, port, is_connect, preamble } = match request {
            Ok(request) => request,
            Err(e) => {
                http::respond_error(&mut stream, "400 Bad Request", &format!("proxy-rs: {e}\n")).await.ok();
                linger(stream.into_inner().1, connection).await;
                return Err(e.context(format!("HTTP proxy request from [{client}] failed")));
            }
        };

        let (status, body) = match dial(client, Some(policy), &host, port).await {
            Dial::Connected(mut tcp) => {
                if is_connect {
                    http::respond_connected(&mut stream).await?;
                } else {
                    tcp.write_all(&preamble).await?;
                }
                let (recv, send) = stream.into_inner();
                pipe(recv, send, tcp).await;
                return Ok(());
            }
            Dial::NotAllowed => ("403 Forbidden", format!("proxy-rs: target {host}:{port} is not allowed by this server\n")),
            Dial::Failed(e) => ("502 Bad Gateway", format!("proxy-rs: connect to {host}:{port} failed: {e}\n")),
        };
        http::respond_error(&mut stream, status, &body).await?;
        linger(stream.into_inner().1, connection).await;
        Ok(())
    }
}
