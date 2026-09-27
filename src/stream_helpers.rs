use anyhow::Result;
use iroh::endpoint::{RecvStream, SendStream};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

/// Proxies an iroh bi-stream <-> a local read/write pair, TCP-style: EOF on one side
/// shuts down the other side's writer (half-close) and it returns once both are done.
pub async fn proxy_streams<C, D>(iroh_recv: RecvStream, iroh_send: SendStream, local_read: C, local_write: D) -> Result<()>
where
    C: AsyncRead + Unpin,
    D: AsyncWrite + Unpin,
{
    let mut iroh = tokio::io::join(iroh_recv, iroh_send);
    tokio::io::copy_bidirectional(&mut iroh, &mut tokio::io::join(local_read, local_write)).await?;
    wait_delivered(iroh.into_inner().1).await;
    Ok(())
}

/// Like [`proxy_streams`], but done as soon as either direction ends (ssh-style, for
/// rsync: it only closes our stdin after we exit, so waiting for both would deadlock).
pub async fn proxy_process_streams<C, D>(mut iroh_recv: RecvStream, mut iroh_send: SendStream, mut local_read: C, mut local_write: D) -> Result<()>
where
    C: AsyncRead + Unpin,
    D: AsyncWrite + Unpin,
{
    tokio::select! {
        result = tokio::io::copy(&mut iroh_recv, &mut local_write) => { result?; }
        result = tokio::io::copy(&mut local_read, &mut iroh_send) => { result?; }
    }
    wait_delivered(iroh_send).await;
    Ok(())
}

/// Finishes the stream and waits for the peer to ack it: the caller drops the
/// `Connection` next, which would otherwise discard data still in flight.
async fn wait_delivered(mut send: SendStream) {
    let _ = send.finish();
    let _ = send.stopped().await;
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

