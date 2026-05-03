use std::time::Duration;

use iroh::{endpoint::Connection, protocol::{AcceptError, ProtocolHandler}};
use tokio::io::AsyncWriteExt;
use tracing::{error, info};
use crate::{protocols::codec::StreamCodec, server::copy_bytes};

use crate::protocols::{ack::Ack, file_send::file_send_header::FileSendHeader};

#[derive(Debug, Clone)]
pub struct FileServerProtocolV1;

impl ProtocolHandler for FileServerProtocolV1 {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        self.handle(connection).await
            .map_err(|e| AcceptError::from_boxed(e.into()))
    }
}

impl FileServerProtocolV1 {
    async fn handle(&self, connection: Connection) -> anyhow::Result<()> {
        info!("Accepted file_send connection from {}", connection.remote_id());
        let _alpn = String::from_utf8(connection.alpn().to_vec())?;
        let (mut iroh_send, mut iroh_recv) = connection.accept_bi().await?;

        let file_send_header = FileSendHeader::decode(&mut iroh_recv).await?;

        if file_send_header.version != 1 {
            error!("Bad file send header version");
            Ack::no_ack(1, Some("unsupported file_send protocol version".to_string())).encode(&mut iroh_send).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            anyhow::bail!("unsupported file_send protocol version: {}", file_send_header.version);
        }

        let file_path = format!("{}_bak", &file_send_header.file_name);
        let file_exists = tokio::fs::metadata(&file_path).await.is_ok();

        if file_exists && !file_send_header.can_overwrite {
            let msg = format!("Error: File {} already exists", file_send_header.file_name);
            error!(msg);
            Ack::no_ack(1, Some(msg.clone())).encode(&mut iroh_send).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            anyhow::bail!(msg);
        }

        Ack::ack().encode(&mut iroh_send).await?;

        info!("Creating file: {}", file_send_header.file_name);
        let mut file = tokio::fs::File::create(&file_path).await?;

        info!("Copying {} bytes", file_send_header.file_size);
        copy_bytes(&mut iroh_recv, &mut file, file_send_header.file_size as usize).await?;

        info!("flushing");
        file.flush().await?;

        info!("sending ack");
        Ack::ack().encode(&mut iroh_send).await?;

        info!("Finished writing file: {_alpn}");
        Ok(())
    }
}
