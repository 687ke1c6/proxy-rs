use iroh::{endpoint::Connection, protocol::{AcceptError, ProtocolHandler}};
use tracing::info;

use crate::protocols::{codec::StreamCodec, ping::ping_header::PingHeader};
use crate::identity::short_id;

#[derive(Debug, Clone)]
pub struct PingServerProtocolV1;

impl ProtocolHandler for PingServerProtocolV1 {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        self.handle(connection).await
            .map_err(|e| AcceptError::from_boxed(e.into()))
    }
}

impl PingServerProtocolV1 {
    async fn handle(&self, connection: Connection) -> anyhow::Result<()> {
        info!("Accepted ping connection from {}", short_id(connection.remote_id()));
        let (mut send, mut recv) = connection.accept_bi().await?;
        let ping = PingHeader::decode(&mut recv).await?;
        info!("Ping received: \"{}\"", ping.msg);
        ping.encode(&mut send).await?;
        info!("Pong sent: \"{}\"", ping.msg);
        send.finish()?;
        connection.closed().await;
        info!("Ping connection with {} completed", short_id(connection.remote_id()));
        Ok(())
    }
}
