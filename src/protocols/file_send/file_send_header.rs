use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use anyhow::Result;

const FLAG_CAN_OVERWRITE: u8 = 0b0000_0001;

#[derive(Debug)]
pub struct FileSendHeader {
    pub version: u8,
    pub file_name: String,
    pub file_size: u64,
    pub can_overwrite: bool,
}

impl FileSendHeader {
    pub async fn from_stream<T>(stream: &mut T) -> Result<Self>
    where
        T: AsyncRead + Unpin,
    {
        let version = stream.read_u8().await?;
        let len = stream.read_u16().await? as usize;
        let mut file_name_bytes = vec![0u8; len];
        stream.read_exact(&mut file_name_bytes).await?;
        let file_name = String::from_utf8(file_name_bytes)?;
        let file_size = stream.read_u64().await?;
        let flags = stream.read_u8().await?;

        Ok(FileSendHeader {
            version,
            file_name,
            file_size,
            can_overwrite: (flags & FLAG_CAN_OVERWRITE) != 0,
        })
    }

    pub async fn write_to_stream<T>(&self, stream: &mut T) -> Result<()>
    where
        T: AsyncWrite + Unpin,
    {
        stream.write_u8(self.version).await?;
        let file_name_bytes = self.file_name.as_bytes();
        stream.write_u16(file_name_bytes.len() as u16).await?;
        stream.write_all(file_name_bytes).await?;
        stream.write_u64(self.file_size).await?;
        stream.write_u8(pack_bits([
            self.can_overwrite,
            false, false, false, false, false, false, false
        ])).await?;
        Ok(())
    }
}

fn pack_bits(bits: [bool; 8]) -> u8 {
    let mut byte = 0;
    for i in 0..8 {
        if bits[i] {
            byte |= 1 << i;
        }
    }
    byte
}