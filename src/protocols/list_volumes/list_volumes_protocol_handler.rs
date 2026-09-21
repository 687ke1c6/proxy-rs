use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use iroh::{endpoint::Connection, protocol::{AcceptError, ProtocolHandler}};
use tracing::info;

use crate::protocols::{
    codec::StreamCodec,
    list_volumes::list_volumes_header::{ListVolumesRequest, ListVolumesResponse, VolumeEntry},
};

#[derive(Debug, Clone)]
pub struct ListVolumesServerProtocolV1 {
    pub volumes: Arc<HashMap<String, PathBuf>>,
}

impl ProtocolHandler for ListVolumesServerProtocolV1 {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        self.handle(connection).await
            .map_err(|e| AcceptError::from_boxed(e.into()))
    }
}

impl ListVolumesServerProtocolV1 {
    async fn handle(&self, connection: Connection) -> anyhow::Result<()> {
        info!("Accepted list_volumes connection from {}", connection.remote_id());
        let (mut send, mut recv) = connection.accept_bi().await?;

        let request = ListVolumesRequest::decode(&mut recv).await?;
        anyhow::ensure!(request.version == 1, "unsupported list_volumes protocol version: {}", request.version);

        let mut volumes: Vec<VolumeEntry> = self
            .volumes
            .iter()
            .map(|(name, path)| VolumeEntry { name: name.clone(), path: path.display().to_string() })
            .collect();
        volumes.sort_by(|a, b| a.name.cmp(&b.name));

        ListVolumesResponse { version: 1, volumes }.encode(&mut send).await?;
        send.finish()?;
        connection.closed().await;

        info!("Sent volume list to {}", connection.remote_id());
        Ok(())
    }
}
