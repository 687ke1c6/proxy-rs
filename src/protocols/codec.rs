//! Wire format for every protocol message: a `u32` big-endian length prefix followed by
//! that many bytes of [postcard]-encoded body.
//!
//! The length prefix means a reader always consumes exactly one whole message, and lets it
//! reject an oversized one before allocating. There's no version on the wire: postcard isn't
//! self-describing, so *any* change to a message struct is a breaking change, and is made by
//! bumping that protocol's ALPN string (`alpn.rs`), so mismatched peers fail at the QUIC
//! handshake instead of mis-decoding each other's bytes.

use anyhow::{Result, ensure};
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Largest message body accepted or sent. Messages are small one-shot headers; this only
/// bounds what a peer can make us allocate.
pub const MAX_FRAME_LEN: u32 = 64 * 1024;

/// Implemented for every serde message type: `msg.encode(&mut send)`, `T::decode(&mut recv)`.
pub trait StreamCodec: Sized {
    async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()>;
    async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self>;
}

impl<T: Serialize + DeserializeOwned> StreamCodec for T {
    async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
        let body = postcard::to_stdvec(self)?;
        let len = u32::try_from(body.len()).ok().filter(|&len| len <= MAX_FRAME_LEN);
        let Some(len) = len else {
            anyhow::bail!("message too large: {} bytes (max {MAX_FRAME_LEN})", body.len());
        };
        w.write_u32(len).await?;
        w.write_all(&body).await?;
        Ok(())
    }

    async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
        let len = r.read_u32().await?;
        ensure!(len <= MAX_FRAME_LEN, "message too large: {len} bytes (max {MAX_FRAME_LEN})");
        let mut body = vec![0u8; len as usize];
        r.read_exact(&mut body).await?;
        Ok(postcard::from_bytes(&body)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Msg {
        name: String,
        items: Vec<String>,
        size: u64,
        flag: bool,
    }

    #[tokio::test]
    async fn round_trips() {
        let msg = Msg { name: "héllo".into(), items: vec!["a".into(), String::new()], size: u64::MAX, flag: true };
        let mut buf = Vec::new();
        msg.encode(&mut buf).await.unwrap();
        // Two messages back to back: each decode must consume exactly its own frame.
        msg.encode(&mut buf).await.unwrap();
        let mut r = &buf[..];
        assert_eq!(Msg::decode(&mut r).await.unwrap(), msg);
        assert_eq!(Msg::decode(&mut r).await.unwrap(), msg);
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn rejects_oversized_on_encode() {
        let msg = Msg { name: "x".repeat(MAX_FRAME_LEN as usize), items: vec![], size: 0, flag: false };
        let mut buf = Vec::new();
        assert!(msg.encode(&mut buf).await.is_err());
        assert!(buf.is_empty(), "nothing written for a rejected message");
    }

    #[tokio::test]
    async fn rejects_oversized_length_prefix_without_reading_body() {
        let buf = (MAX_FRAME_LEN + 1).to_be_bytes();
        assert!(Msg::decode(&mut &buf[..]).await.is_err());
    }

    #[tokio::test]
    async fn malformed_body_is_an_error() {
        let mut buf = 1u32.to_be_bytes().to_vec();
        buf.push(0xff); // string length varint with continuation bit, then EOF
        assert!(Msg::decode(&mut &buf[..]).await.is_err());
    }
}
