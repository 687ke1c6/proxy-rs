use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iroh::{endpoint::{Connection, SendStream}, protocol::{AcceptError, ProtocolHandler}};
use tokio::io::AsyncWriteExt;
use tracing::{error, info};
use crate::{protocols::codec::StreamCodec, stream_helpers::copy_bytes};

use crate::protocols::{ack::Ack, file_send::file_send_header::FileSendHeader};

#[derive(Debug, Clone)]
pub struct FileServerProtocolV1 {
    /// name -> canonicalized root directory, configured via `-v`/`--volume`.
    pub volumes: Arc<HashMap<String, PathBuf>>,
}

impl ProtocolHandler for FileServerProtocolV1 {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        self.handle(connection).await
            .map_err(|e| AcceptError::from_boxed(e.into()))
    }
}

/// Joins `target_dir`/`file_name` onto `root` (already canonical), rejecting any
/// traversal outside of it (including via symlinks, since the joined directory is
/// canonicalized before the check).
fn safe_join(root: &Path, target_dir: &str, file_name: &str) -> Result<PathBuf, String> {
    if file_name.is_empty() || file_name.contains('/') || file_name == "." || file_name == ".." {
        return Err(format!("invalid file name: {file_name:?}"));
    }

    let mut dir = root.to_path_buf();
    for component in target_dir.split('/').filter(|s| !s.is_empty()) {
        if component == "." || component == ".." {
            return Err(format!("invalid target path segment: {component:?}"));
        }
        dir.push(component);
    }

    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create target directory: {e}"))?;
    let canonical_dir = dir.canonicalize().map_err(|e| format!("failed to resolve target directory: {e}"))?;
    if !canonical_dir.starts_with(root) {
        return Err("target path escapes the volume root".to_string());
    }

    Ok(canonical_dir.join(file_name))
}

/// Finishes the send half of the stream and waits for the connection to fully close.
///
/// Without this, the `Router` tears the connection down as soon as `handle()` returns,
/// racing whatever was just written — the client can see "closed by peer" while decoding
/// an ack even though every byte the server wrote was already correct and flushed to
/// disk. Mirrors what `PingServerProtocolV1` already does after sending its pong.
async fn finish_and_close(send: &mut SendStream, connection: &Connection) -> anyhow::Result<()> {
    send.finish()?;
    connection.closed().await;
    Ok(())
}

/// Resolves the on-disk destination for an incoming file, given the server's configured
/// volumes and the client-requested volume/target_dir from the header.
fn resolve_destination(volumes: &HashMap<String, PathBuf>, header: &FileSendHeader) -> Result<PathBuf, String> {
    if header.volume.is_empty() {
        return match volumes.len() {
            0 => Err("server exposes no volumes; an operator must configure at least one with -v/--volume".to_string()),
            1 => safe_join(volumes.values().next().unwrap(), &header.target_dir, &header.file_name),
            _ => {
                let names: Vec<&str> = volumes.keys().map(String::as_str).collect();
                Err(format!(
                    "server exposes multiple volumes ({}); pass --target <volume>[/dir]",
                    names.join(", ")
                ))
            }
        };
    }

    match volumes.get(&header.volume) {
        Some(root) => safe_join(root, &header.target_dir, &header.file_name),
        None => {
            let names: Vec<&str> = volumes.keys().map(String::as_str).collect();
            Err(format!("unknown volume {:?}; available volumes: {}", header.volume, names.join(", ")))
        }
    }
}

impl FileServerProtocolV1 {
    async fn handle(&self, connection: Connection) -> anyhow::Result<()> {
        info!("Accepted file_send connection from {}", connection.remote_id());
        let alpn_string = String::from_utf8(connection.alpn().to_vec())?;
        let (mut iroh_send, mut iroh_recv) = connection.accept_bi().await?;

        let file_send_header = FileSendHeader::decode(&mut iroh_recv).await?;

        if file_send_header.version != 2 {
            error!("Bad file send header version");
            Ack::no_ack(1, Some("unsupported file_send protocol version".to_string())).encode(&mut iroh_send).await?;
            finish_and_close(&mut iroh_send, &connection).await?;
            anyhow::bail!("unsupported file_send protocol version: {}", file_send_header.version);
        }

        let file_path = match resolve_destination(&self.volumes, &file_send_header) {
            Ok(path) => path,
            Err(msg) => {
                error!(msg);
                Ack::no_ack(1, Some(msg.clone())).encode(&mut iroh_send).await?;
                finish_and_close(&mut iroh_send, &connection).await?;
                anyhow::bail!(msg);
            }
        };
        let file_exists = tokio::fs::metadata(&file_path).await.is_ok();

        if file_exists && !file_send_header.can_overwrite {
            let msg = format!("Error: File {} already exists", file_path.display());
            error!(msg);
            Ack::no_ack(1, Some(msg.clone())).encode(&mut iroh_send).await?;
            finish_and_close(&mut iroh_send, &connection).await?;
            anyhow::bail!(msg);
        }

        Ack::ack().encode(&mut iroh_send).await?;

        info!("Creating file: {}", file_path.display());
        let mut file = tokio::fs::File::create(&file_path).await?;

        info!("Copying {} bytes", file_send_header.file_size);
        copy_bytes(&mut iroh_recv, &mut file, file_send_header.file_size as usize).await?;

        info!("flushing");
        file.flush().await?;

        info!("sending ack");
        Ack::ack().encode(&mut iroh_send).await?;
        finish_and_close(&mut iroh_send, &connection).await?;

        info!("Finished writing file: {alpn_string}");
        Ok(())
    }
}
