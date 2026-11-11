use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use iroh::{endpoint::Connection, protocol::{AcceptError, ProtocolHandler}};
use tracing::info;

use crate::identity::short_id;
use crate::protocols::{
    codec::StreamCodec,
    list_volumes::list_volumes_header::{ListVolumesRequest, ListVolumesResponse, VolumeEntry},
};

#[derive(Debug, Clone)]
pub struct ListVolumesServerProtocol {
    pub volumes: Arc<HashMap<String, PathBuf>>,
}

impl ProtocolHandler for ListVolumesServerProtocol {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        self.handle(connection).await
            .map_err(|e| AcceptError::from_boxed(e.into()))
    }
}

impl ListVolumesServerProtocol {
    async fn handle(&self, connection: Connection) -> anyhow::Result<()> {
        info!("Accepted list_volumes connection from {}", short_id(connection.remote_id()));
        let (mut send, mut recv) = connection.accept_bi().await?;

        ListVolumesRequest::decode(&mut recv).await?;

        let mut volumes: Vec<VolumeEntry> = self
            .volumes
            .iter()
            .map(|(name, path)| VolumeEntry { name: name.clone(), path: path.display().to_string() })
            .collect();
        volumes.sort_by(|a, b| a.name.cmp(&b.name));

        ListVolumesResponse { volumes }.encode(&mut send).await?;
        send.finish()?;
        connection.closed().await;

        info!("Sent volume list to {}", short_id(connection.remote_id()));
        Ok(())
    }
}
