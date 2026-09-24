use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::Context;
use iroh::{endpoint::{Connection, SendStream}, protocol::{AcceptError, ProtocolHandler}};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{error, info, warn};

use crate::protocols::{ack::Ack, codec::StreamCodec, rsync::rsync_header::RsyncHeader, volume_paths::{parse_volume_path, safe_join_dir}};
use crate::stream_helpers::proxy_streams;

#[derive(Debug, Clone)]
pub struct RsyncServerProtocolV1 {
    /// name -> canonicalized root directory, configured via `-v`/`--volume`. Same map
    /// `FileServerProtocolV1` uses — one allowlist shared by every protocol that
    /// writes into the filesystem on the client's behalf.
    pub volumes: Arc<HashMap<String, PathBuf>>,
}

impl ProtocolHandler for RsyncServerProtocolV1 {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        self.handle(connection).await
            .map_err(|e| AcceptError::from_boxed(e.into()))
    }
}

/// Finishes the send half of the stream and waits for the connection to fully close.
/// Same pattern (and same race it avoids) as file-send's `finish_and_close`.
async fn finish_and_close(send: &mut SendStream, connection: &Connection) -> anyhow::Result<()> {
    send.finish()?;
    connection.closed().await;
    Ok(())
}

impl RsyncServerProtocolV1 {
    /// Validates `argv` (as `sync-rsh` forwarded it) and resolves the client's
    /// destination-path token into a safe, volume-confined directory.
    ///
    /// ⚠️ Flags are passed through unmodified in this first pass — no `--delete`
    /// filtering yet. See the "Known gap" section in the rsync-sync plan.
    fn resolve(&self, argv: &[String]) -> Result<(Vec<String>, PathBuf), String> {
        println!("rsync args: {}", argv.join(", "));
        if argv.len() < 2 {
            return Err("invalid rsync invocation: expected --server and a destination path".to_string());
        }
        if argv[0] != "--server" {
            return Err("invalid rsync invocation: expected --server as the first argument".to_string());
        }

        let path_token = &argv[argv.len() - 1];
        let passthrough_flags = argv[1..argv.len() - 1].to_vec();

        let (volume, target_dir) = parse_volume_path(path_token)?;
        let root = self.volumes.get(&volume).ok_or_else(|| {
            let names: Vec<&str> = self.volumes.keys().map(String::as_str).collect();
            format!("unknown volume {volume:?}; available volumes: {}", names.join(", "))
        })?;
        let resolved_dir = safe_join_dir(root, &target_dir)?;

        Ok((passthrough_flags, resolved_dir))
    }

    async fn handle(&self, connection: Connection) -> anyhow::Result<()> {
        println!("Accepted rsync connection from {}", connection.remote_id());
        let (mut iroh_send, mut iroh_recv) = connection.accept_bi().await?;

        let header = RsyncHeader::decode(&mut iroh_recv).await?;
        if header.version != 1 {
            error!("Bad rsync header version");
            Ack::no_ack(1, Some("unsupported rsync protocol version".to_string())).encode(&mut iroh_send).await?;
            finish_and_close(&mut iroh_send, &connection).await?;
            anyhow::bail!("unsupported rsync protocol version: {}", header.version);
        }

        let (passthrough_flags, resolved_dir) = match self.resolve(&header.argv) {
            Ok(v) => v,
            Err(msg) => {
                error!(msg);
                Ack::no_ack(1, Some(msg.clone())).encode(&mut iroh_send).await?;
                finish_and_close(&mut iroh_send, &connection).await?;
                anyhow::bail!(msg);
            }
        };

        Ack::ack().encode(&mut iroh_send).await?;

        // NOTE: `passthrough_flags` is `argv[1..len-1]` verbatim, which per the
        // documented rsync `--server` invocation shape already ends with the "."
        // source-args marker rsync itself sends — we don't reconstruct it here,
        // only substitute our own resolved path for the client's destination token.
        // Unverified against a real rsync binary in this session (see the plan's
        // "Known gap" section) — if this assumption is wrong, rsync will fail loudly
        // (protocol error), not silently, which the smoke test will catch.
        let mut args = vec!["--server".to_string()];
        args.extend(passthrough_flags);
        args.push(resolved_dir.display().to_string());

        info!("Spawning: rsync {}", args.join(" "));
        let mut child = Command::new("rsync")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn rsync --server (is rsync installed on this server host?)")?;

        let child_stdin = child.stdin.take().expect("stdin was piped");
        let child_stdout = child.stdout.take().expect("stdout was piped");
        let child_stderr = child.stderr.take().expect("stderr was piped");

        // Drain stderr concurrently with the main bridge below — otherwise, once the
        // pipe's kernel buffer fills, the child would block writing to stderr and the
        // whole transfer would stall.
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(child_stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                warn!("rsync --server: {line}");
            }
        });

        let bridge_result = proxy_streams(iroh_recv, iroh_send, child_stdout, child_stdin).await;
        let _ = stderr_task.await;

        let status = child.wait().await?;
        info!("rsync --server exited with {status}");

        bridge_result?;
        Ok(())
    }
}
