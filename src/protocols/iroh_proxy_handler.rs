use anyhow::Context;
use iroh::{endpoint::Connection, protocol::{AcceptError, ProtocolHandler}};
use tokio::net::TcpStream;
use tracing::{info, warn};

use crate::proxy::{proxy_streams, read_target};

#[derive(Debug, Clone)]
pub struct ProxyServerProtocol;

impl ProtocolHandler for ProxyServerProtocol {
    async fn accept(&self, connection: Connection) -> anyhow::Result<(), AcceptError> {
        let alpn = String::from_utf8(connection.alpn().to_vec()).unwrap();
        let (iroh_send, mut iroh_recv) = connection.accept_bi().await?;

        let proxy_header = read_target(&mut iroh_recv).await.with_context(||"").unwrap();
        let tcp_target = format!("{}:{}", proxy_header.host, proxy_header.port);
        info!("Connecting to TCP target {tcp_target}");

        let tcp = TcpStream::connect(&tcp_target).await?;
        let (tcp_read, tcp_write) = tcp.into_split();

        proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await.unwrap_or_default();

        warn!("Connection from {alpn} to {tcp_target} closed");

        Ok(())
    }
}
