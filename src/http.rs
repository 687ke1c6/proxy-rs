use anyhow::{bail, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Perform an HTTP proxy handshake.
///
/// Returns `(host, port, preamble)` where `preamble` is any data that must be
/// forwarded to the upstream target before streaming the rest of the connection.
/// For `CONNECT` (HTTPS) this is empty; for plain HTTP it is the original
/// request headers so the upstream server receives a well-formed request.
pub async fn handshake(stream: &mut TcpStream) -> Result<(String, u16, Vec<u8>)> {
    let headers = read_headers(stream).await?;
    let headers_str = std::str::from_utf8(&headers)?;

    let first_line = headers_str
        .split_once("\r\n")
        .map(|(l, _)| l)
        .ok_or_else(|| anyhow::anyhow!("empty HTTP request"))?;

    let mut parts = first_line.splitn(3, ' ');
    let method = parts.next().ok_or_else(|| anyhow::anyhow!("missing HTTP method"))?;
    let target = parts.next().ok_or_else(|| anyhow::anyhow!("missing HTTP target"))?;

    if method == "CONNECT" {
        // HTTPS tunnel: target is "host:port"
        let (host, port_str) = target
            .rsplit_once(':')
            .ok_or_else(|| anyhow::anyhow!("invalid CONNECT target: {target}"))?;
        let port: u16 = port_str.parse()?;
        stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
        Ok((host.to_string(), port, vec![]))
    } else {
        // Plain HTTP: target is an absolute URL, e.g. "http://example.com/path"
        let without_scheme = target
            .strip_prefix("http://")
            .ok_or_else(|| anyhow::anyhow!("expected http:// URL, got: {target}"))?;
        let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
        let (host, port) = if let Some((h, p)) = authority.rsplit_once(':') {
            (h.to_string(), p.parse::<u16>()?)
        } else {
            (authority.to_string(), 80u16)
        };
        // RFC 7230 §5.3.2: servers MUST accept the absolute-form, so forward as-is.
        Ok((host, port, headers))
    }
}

/// Read bytes from `stream` until the end of the HTTP headers (`\r\n\r\n`).
async fn read_headers(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(1024);
    let mut tmp = [0u8; 1];
    loop {
        stream.read_exact(&mut tmp).await?;
        buf.push(tmp[0]);
        if buf.ends_with(b"\r\n\r\n") {
            return Ok(buf);
        }
        if buf.len() > 64 * 1024 {
            bail!("HTTP headers exceed 64 KiB");
        }
    }
}
