use anyhow::Result;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use crate::protocols::proxy::proxy_header::ProxyHeader;

const FLAG_CAN_READ: u8    = 0b0000_0001;
const FLAG_CAN_WRITE: u8   = 0b0000_0010;
const FLAG_CAN_EXECUTE: u8 = 0b0000_0100;

/// Read a target host and port written by `write_proxy_header`.
pub async fn read_proxy_header<R: AsyncRead + Unpin>(stream: &mut R) -> Result<ProxyHeader> {
    let version = stream.read_u8().await?;
    match version {
        1 => read_target_v1(stream).await,
        _ => anyhow::bail!("unsupported proxy protocol version: {version}"),
    }
}

pub async fn read_target_v1<R: AsyncRead + Unpin>(stream: &mut R) -> Result<ProxyHeader> {
    println!("Reading proxy header (version 1)");
    let host_len = stream.read_u16().await? as usize;
    let mut host_bytes = vec![0u8; host_len];
    stream.read_exact(&mut host_bytes).await?;
    let host = String::from_utf8(host_bytes)?;
    let port = stream.read_u16().await?;
    let flags = stream.read_u8().await?; // reserved for future flags

    Ok(ProxyHeader {
        host, port, version: 1,
        can_read: (flags & FLAG_CAN_READ) != 0, 
        can_write: (flags & FLAG_CAN_WRITE) != 0, 
        can_execute: (flags & FLAG_CAN_EXECUTE) != 0 })
}

/// Bidirectionally proxy between two async read/write halves.
/// Returns when either direction closes or errors.
pub async fn proxy_streams<A, B, C, D>(
    mut a_read: A,
    mut a_write: B,
    mut b_read: C,
    mut b_write: D,
) -> Result<()>
where
    A: AsyncRead + Unpin + Send + 'static,
    B: AsyncWrite + Unpin + Send + 'static,
    C: AsyncRead + Unpin + Send + 'static,
    D: AsyncWrite + Unpin + Send + 'static,
{
    let a_to_b = tokio::io::copy(&mut a_read, &mut b_write);
    let b_to_a = tokio::io::copy(&mut b_read, &mut a_write);

    tokio::select! {
        result = a_to_b => { result?; }
        result = b_to_a => { result?; }
    }

    Ok(())
}
