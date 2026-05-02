use anyhow::{Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::info;

pub struct PingHeader {
    pub version: u8,
    pub msg: String,
}

impl PingHeader {
    pub async fn write_to_stream<W: AsyncWrite + Unpin>(&self, stream: &mut W) -> Result<()> {
        stream.write_u8(self.version).await?;
        let msg_bytes = self.msg.as_bytes();
        stream.write_u16(msg_bytes.len() as u16).await?;
        stream.write_all(msg_bytes).await?;
        Ok(())
    }

    pub async fn from_stream<R: AsyncRead + Unpin>(stream: &mut R) -> Result<Self> {
        info!("reading ping header from stream");
        let version = stream.read_u8().await.with_context(||"something isn't working")?;
        info!("Reading ping header: version {}", version);
        let msg_len = stream.read_u16().await? as usize;
        let mut msg_bytes = vec![0u8; msg_len];
        stream.read_exact(&mut msg_bytes).await?;
        info!("Received ping header: version {}, msg_len {}", version, msg_len);
        Ok(Self { version, msg: String::from_utf8(msg_bytes)? })
    }
}
