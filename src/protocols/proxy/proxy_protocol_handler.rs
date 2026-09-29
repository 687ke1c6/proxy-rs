use std::sync::Arc;

use iroh::{endpoint::{Connection, SendStream}, protocol::{AcceptError, ProtocolHandler}};
use tokio::net::TcpStream;
use tracing::{info, warn};

use crate::protocols::{ack::Ack, codec::StreamCodec, proxy::{proxy_header::{ACK_BAD_VERSION, ACK_CONNECT_FAILED, ACK_NOT_ALLOWED, PROXY_HEADER_VERSION, ProxyHeader}, target_policy::TargetPolicy}};
use crate::stream_helpers::proxy_streams;

#[derive(Debug, Clone)]
pub struct ProxyServerProtocolV2 {
    pub policy: Arc<TargetPolicy>,
}

impl ProtocolHandler for ProxyServerProtocolV2 {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        self.handle(connection).await
            .map_err(|e| AcceptError::from_boxed(e.into()))
    }
}

/// Sends a failure `Ack` and waits for the client to hang up, so the Ack isn't lost
/// to the connection being torn down when the handler returns.
async fn reject(send: &mut SendStream, connection: &Connection, code: u8, msg: String) -> anyhow::Result<()> {
    Ack::no_ack(code, Some(msg)).encode(send).await?;
    send.finish()?;
    connection.closed().await;
    Ok(())
}

impl ProxyServerProtocolV2 {
    async fn handle(&self, connection: Connection) -> anyhow::Result<()> {
        let client = connection.remote_id();
        let (mut iroh_send, mut iroh_recv) = connection.accept_bi().await?;

        let header = ProxyHeader::decode(&mut iroh_recv).await?;
        if header.version != PROXY_HEADER_VERSION {
            let msg = format!("unsupported proxy header version {} (expected {PROXY_HEADER_VERSION})", header.version);
            warn!("{msg} from {client}");
            return reject(&mut iroh_send, &connection, ACK_BAD_VERSION, msg).await;
        }

        let tcp_target = format!("{}:{}", header.host, header.port);
        if !self.policy.allows(&header.host, header.port) {
            warn!("Rejected TCP target {tcp_target} from {client}: not allowed by --target");
            let msg = format!("target {tcp_target} is not allowed by this server");
            return reject(&mut iroh_send, &connection, ACK_NOT_ALLOWED, msg).await;
        }

        info!("Connecting to TCP target {tcp_target} for {client}");
        // (host, port) rather than the formatted string, so bare IPv6 hosts resolve correctly.
        let tcp = match TcpStream::connect((header.host.as_str(), header.port)).await {
            Ok(tcp) => tcp,
            Err(e) => {
                warn!("Failed to connect to {tcp_target} for {client}: {e}");
                return reject(&mut iroh_send, &connection, ACK_CONNECT_FAILED, format!("connect to {tcp_target} failed: {e}")).await;
            }
        };
        Ack::ack().encode(&mut iroh_send).await?;

        let (tcp_read, tcp_write) = tcp.into_split();
        if let Err(e) = proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await {
            info!("Proxy stream to {tcp_target} for {client} ended with error: {e:#}");
        }
        info!("Connection from {client} to {tcp_target} closed");
        Ok(())
    }
}
