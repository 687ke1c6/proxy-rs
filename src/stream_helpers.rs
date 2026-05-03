use anyhow::Result;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

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

pub async fn copy_bytes<A, B>(a: &mut A, b: &mut B, size: usize) -> anyhow::Result<()>
    where 
        A: tokio::io::AsyncRead + Unpin,
        B: tokio::io::AsyncWrite + Unpin 
    {
        let mut limited = a.take(size as u64);
        tokio::io::copy(&mut limited, b).await?;
        Ok(())
    }

