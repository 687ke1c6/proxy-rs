use anyhow::Result;
use iroh::Watcher;
use iroh::endpoint::{ConnectionError, PathInfoList, PathWatcher, RecvStream, SendStream};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

/// Why a connection ended, as a ` (reason)` suffix for a console line, or empty for a
/// deliberate close by either side (a process exited or shut down cleanly).
pub fn close_reason_suffix(reason: &ConnectionError) -> String {
    match reason {
        ConnectionError::ApplicationClosed(_) | ConnectionError::LocallyClosed => String::new(),
        reason => format!(" ({reason})"),
    }
}

/// `"direct"` or `"relay"`: the kind of network path a connection currently sends on, or
/// `None` if none is selected yet.
fn selected_path_kind(paths: &PathInfoList) -> Option<&'static str> {
    paths.iter().find(|p| p.is_selected()).map(|p| if p.is_relay() { "relay" } else { "direct" })
}

/// The connection's current path kind as a ` (direct)`/` (relay)` suffix for its
/// "connected" console line, or empty if no path is selected yet.
pub fn path_kind_suffix(paths: &mut PathWatcher) -> String {
    selected_path_kind(&paths.get()).map(|kind| format!(" ({kind})")).unwrap_or_default()
}

/// Calls `on_change` with `"direct"`/`"relay"` whenever the connection's selected path
/// switches kind, e.g. when holepunching upgrades a relayed connection to direct.
/// `paths` should be the watcher [`path_kind_suffix`] was called with, so the kind already
/// printed isn't repeated. Never returns, even once the connection is gone: race it
/// against the connection's `closed()`.
pub async fn watch_path_kind(mut paths: PathWatcher, on_change: impl Fn(&'static str)) {
    let mut current = selected_path_kind(&paths.get());
    while let Ok(list) = paths.updated().await {
        let kind = selected_path_kind(&list);
        if kind.is_some() && kind != current {
            on_change(kind.unwrap());
            current = kind;
        }
    }
    std::future::pending().await
}

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

