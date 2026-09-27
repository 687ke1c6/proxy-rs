use anyhow::{bail, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// SOCKS5 reply codes (RFC 1928 §6) sent via [`reply`].
pub const REP_SUCCEEDED: u8 = 0x00;
pub const REP_GENERAL_FAILURE: u8 = 0x01;
pub const REP_NOT_ALLOWED: u8 = 0x02;
pub const REP_CONNECTION_REFUSED: u8 = 0x05;

/// Perform a SOCKS5 handshake on the given stream, up to and including the CONNECT
/// request. Returns the requested target host and port. The caller must then send
/// exactly one [`reply`] once it knows whether the target is reachable.
pub async fn handshake(stream: &mut TcpStream) -> Result<(String, u16)> {
    let version = stream.read_u8().await?;
    if version != 5 {
        bail!("unsupported SOCKS version: {version}");
    }

    let nmethods = stream.read_u8().await? as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;

    if !methods.contains(&0x00) {
        stream.write_all(&[0x05, 0xFF]).await?;
        bail!("client offered no acceptable auth methods");
    }
    stream.write_all(&[0x05, 0x00]).await?;

    let version = stream.read_u8().await?;
    if version != 5 {
        bail!("expected SOCKS5 in request, got version {version}");
    }
    let cmd = stream.read_u8().await?;
    let _rsv = stream.read_u8().await?;
    let atyp = stream.read_u8().await?;

    if cmd != 0x01 {
        stream
            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await?;
        bail!("unsupported SOCKS5 command: {cmd:#04x} (only CONNECT supported)");
    }

    let host = match atyp {
        0x01 => {
            let mut addr = [0u8; 4];
            stream.read_exact(&mut addr).await?;
            format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3])
        }
        0x03 => {
            let len = stream.read_u8().await? as usize;
            let mut name = vec![0u8; len];
            stream.read_exact(&mut name).await?;
            String::from_utf8(name)?
        }
        0x04 => {
            let mut addr = [0u8; 16];
            stream.read_exact(&mut addr).await?;
            let v6 = std::net::Ipv6Addr::from(addr);
            format!("{v6}")
        }
        _ => bail!("unsupported SOCKS5 address type: {atyp:#04x}"),
    };

    let port = stream.read_u16().await?;

    Ok((host, port))
}

/// Send the SOCKS5 reply to a CONNECT request, with bound address 0.0.0.0:0.
pub async fn reply(stream: &mut TcpStream, rep: u8) -> Result<()> {
    stream
        .write_all(&[0x05, rep, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;
    Ok(())
}
