use anyhow::{bail, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// A parsed HTTP proxy request.
pub struct ProxyRequest {
    pub host: String,
    pub port: u16,
    /// `CONNECT` tunnel: the caller must send [`respond_connected`] once the upstream
    /// is reachable. Plain HTTP requests get no proxy-level success response; the
    /// upstream's own response is streamed back instead.
    pub is_connect: bool,
    /// Data to forward to the upstream before streaming the rest of the connection.
    /// For `CONNECT` (HTTPS) this is empty; for plain HTTP it is the original request
    /// headers so the upstream server receives a well-formed request.
    pub preamble: Vec<u8>,
}

/// Perform an HTTP proxy handshake: read and parse the request, without responding.
pub async fn handshake(stream: &mut TcpStream) -> Result<ProxyRequest> {
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
        Ok(ProxyRequest { host: host.to_string(), port, is_connect: true, preamble: vec![] })
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
        Ok(ProxyRequest { host, port, is_connect: false, preamble: headers })
    }
}

/// Tell a `CONNECT` client its tunnel is up.
pub async fn respond_connected(stream: &mut TcpStream) -> Result<()> {
    stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
    Ok(())
}

/// Send a proxy-generated error response (e.g. `403 Forbidden`, `502 Bad Gateway`)
/// with `body` as plain text.
pub async fn respond_error(stream: &mut TcpStream, status: &str, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
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
