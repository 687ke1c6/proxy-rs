use anyhow::{bail, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Perform a SOCKS5 handshake on the given stream.
/// Returns the requested target host and port, with the stream
/// left positioned at the start of proxied data.
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

    // Reply: success, bound address 0.0.0.0:0
    stream
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    Ok((host, port))
}
