use anyhow::Result;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const ALPN: &[u8] = b"proxy-rs/0";

const FLAG_CAN_READ: u8    = 0b0000_0001;
const FLAG_CAN_WRITE: u8   = 0b0000_0010;
const FLAG_CAN_EXECUTE: u8 = 0b0000_0100;

pub struct ProxyHeader {
    pub version: u8,
    pub host: String,
    pub port: u16,
    pub can_read: bool,
    pub can_write: bool,
    pub can_execute: bool,
}

fn pack_bits(bits: [bool; 8]) -> u8 {
    let mut byte = 0;
    for i in 0..8 {
        if bits[i] {
            // LSB first: index 0 maps to the 1s place (2^0)
            byte |= 1 << i;
        }
    }
    byte
}

/// Write a target host and port as a length-prefixed header.
/// Format: u16 host_len | host bytes | u16 port (all big-endian)
pub async fn write_target<W: AsyncWrite + Unpin>(stream: &mut W, header: &ProxyHeader) -> Result<()> {
    stream.write_u8(header.version).await?;
    let bytes = header.host.as_bytes();
    stream.write_u16(bytes.len() as u16).await?;
    stream.write_all(bytes).await?;
    stream.write_u16(header.port).await?;
    stream.write_u8(pack_bits([
        header.can_read,
        header.can_write,
        header.can_execute,
        false, false, false, false, false])).await?; // reserved for future flags
    Ok(())
}

/// Read a target host and port written by `write_target`.
pub async fn read_target<R: AsyncRead + Unpin>(stream: &mut R) -> Result<ProxyHeader> {
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
