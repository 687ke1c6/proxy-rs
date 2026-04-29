use iroh::{endpoint::Connection, protocol::{AcceptError, ProtocolHandler}};
use tokio::io::AsyncWriteExt;
use tracing::info;
use crate::server::copy_bytes;

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

        let file_send_header = FileSendHeader::from_stream(&mut iroh_recv).await?;
        dbg!(&file_send_header);

        if file_send_header.version != 1 {
            Ack::new(1, Some("unsupported file_send protocol version".to_string())).write_ack(&mut iroh_send).await?;
            anyhow::bail!("unsupported file_send protocol version: {}", file_send_header.version);
        }

        let file_path = format!("{}", &file_send_header.file_name);
        let file_exists = tokio::fs::metadata(&file_path).await.is_ok();

        if file_exists && !file_send_header.can_overwrite {
            Ack::new(1, Some(format!("file {} already exists", file_send_header.file_name))).write_ack(&mut iroh_send).await?;
            anyhow::bail!("file already exists and overwrite not allowed");
        }

        Ack::new(0, None).write_ack(&mut iroh_send).await?;

        info!("Creating file: {}", file_send_header.file_name);
        let mut file = tokio::fs::File::create(&file_path).await?;

        info!("Copying {} bytes", file_send_header.file_size);
        copy_bytes(&mut iroh_recv, &mut file, file_send_header.file_size as usize).await?;

        info!("flushing");
        file.flush().await?;

        info!("sending ack");
        Ack::new(0, None).write_ack(&mut iroh_send).await?;

        info!("Finished writing file: {_alpn}");
        Ok(())
    }
}
