use anyhow::Result;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub use proxy_rs_derive::StreamCodec;

pub trait StreamCodec: Sized {
    async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()>;
    async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self>;
}

impl StreamCodec for u8 {
    async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
        Ok(w.write_u8(*self).await?)
    }
    async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        Ok(r.read_u8().await?)
    }
}

impl StreamCodec for u16 {
    async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
        Ok(w.write_u16(*self).await?)
    }
    async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        Ok(r.read_u16().await?)
    }
}

impl StreamCodec for u64 {
    async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
        Ok(w.write_u64(*self).await?)
    }
    async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        Ok(r.read_u64().await?)
    }
}

impl StreamCodec for bool {
    async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
        Ok(w.write_u8(if *self { 1 } else { 0 }).await?)
    }
    async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        Ok(r.read_u8().await? != 0)
    }
}

// Strings are always encoded as a u16 length prefix followed by UTF-8 bytes.
impl StreamCodec for String {
    async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
        let bytes = self.as_bytes();
        w.write_u16(bytes.len() as u16).await?;
        Ok(w.write_all(bytes).await?)
    }
    async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        let len = r.read_u16().await? as usize;
        let mut bytes = vec![0u8; len];
        r.read_exact(&mut bytes).await?;
        Ok(String::from_utf8(bytes)?)
    }
}

// Vecs are always encoded as a u16 length prefix followed by each item in order.
impl<T: StreamCodec> StreamCodec for Vec<T> {
    async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
        w.write_u16(self.len() as u16).await?;
        for item in self {
            item.encode(w).await?;
        }
        Ok(())
    }
    async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        let len = r.read_u16().await? as usize;
        let mut items = Vec::with_capacity(len);
        for _ in 0..len {
            items.push(T::decode(r).await?);
        }
        Ok(items)
    }
}
