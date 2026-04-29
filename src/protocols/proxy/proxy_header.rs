use tokio::io::{AsyncWrite, AsyncWriteExt};

pub struct ProxyHeader {
    pub version: u8,
    pub host: String,
    pub port: u16,
    pub can_read: bool,
    pub can_write: bool,
    pub can_execute: bool,
}

impl ProxyHeader {
    pub async fn write_to_stream<T>(&self, stream: &mut T) -> anyhow::Result<()>
    where
        T: AsyncWrite + Unpin,
    {
        stream.write_u8(self.version).await?;
        let host_bytes = self.host.as_bytes();
        stream.write_u16(host_bytes.len() as u16).await?;
        stream.write_all(host_bytes).await?;
        stream.write_u16(self.port).await?;
        stream.write_u8(pack_bits([
            self.can_read,
            self.can_write,
            self.can_execute,
            false, false, false, false, false])).await?; // reserved for future flags
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