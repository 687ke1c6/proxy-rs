use iroh::{endpoint::Connection, protocol::{AcceptError, ProtocolHandler}};
use tokio::net::TcpStream;
use tracing::{info, warn};

use crate::protocols::{codec::StreamCodec, proxy::{proxy_header::ProxyHeaderV1, proxy_helpers::proxy_streams}};

#[derive(Debug, Clone)]
pub struct ProxyServerProtocolV1;

impl ProtocolHandler for ProxyServerProtocolV1 {
    async fn accept(&self, connection: Connection) -> anyhow::Result<(), AcceptError> {
        let alpn = String::from_utf8(connection.alpn().to_vec()).unwrap();
        let (iroh_send, mut iroh_recv) = connection.accept_bi().await?;

        let proxy_header = ProxyHeaderV1::decode(&mut iroh_recv).await.expect("Decode ProxyHeader failed");
        let tcp_target = format!("{}:{}", proxy_header.host, proxy_header.port);
        info!("Connecting to TCP target {tcp_target}");

        let tcp = TcpStream::connect(&tcp_target).await?;
        let (tcp_read, tcp_write) = tcp.into_split();

        proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await.unwrap_or_default();

        warn!("Connection from {alpn} to {tcp_target} closed");

        Ok(())
    }
}
