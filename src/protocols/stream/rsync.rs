use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::Context;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{info, warn};

use crate::identity::short_id;
use crate::protocols::stream::stream_header::ACK_REFUSED;
use crate::protocols::stream::stream_protocol_handler::reject;
use crate::protocols::stream::volume_paths::{parse_volume_path, safe_join_dir};
use crate::protocols::{ack::Ack, codec::StreamCodec};
use crate::stream_helpers::proxy_process_streams;

/// name -> canonicalized root directory, configured via `-v`/`--volume`: the only places
/// an rsync push may write into.
pub type Volumes = HashMap<String, PathBuf>;

/// Validates the client's `--pipe` arguments, which are what rsync appends to its `-e`
/// command (`<host> rsync --server <flags> . <volume[/dir]>`), and resolves the trailing
/// destination token into a safe, volume-confined directory. The host is a placeholder and
/// is ignored. Returns the flags to pass through and that directory.
///
/// The trailing path is never trusted as a filesystem path; the flags before it are passed
/// to the spawned `rsync --server` verbatim (no `--delete` filtering).
fn resolve(volumes: &Volumes, args: &[String]) -> Result<(Vec<String>, PathBuf), String> {
    let Some(("rsync", argv)) = args.get(1..).and_then(|rest| rest.split_first()).map(|(cmd, argv)| (cmd.as_str(), argv)) else {
        return Err("this server takes rsync pushes only: use it as rsync's -e command (see `proxy-rs rsync --help`)".to_string());
    };
    if argv.len() < 2 {
        return Err("invalid rsync invocation: expected --server and a destination path".to_string());
    }
    if argv[0] != "--server" {
        return Err("invalid rsync invocation: expected --server as the first argument".to_string());
    }

    let path_token = &argv[argv.len() - 1];
    let passthrough_flags = argv[1..argv.len() - 1].to_vec();

    let (volume, target_dir) = parse_volume_path(path_token)?;
    let root = volumes.get(&volume).ok_or_else(|| {
        let names: Vec<&str> = volumes.keys().map(String::as_str).collect();
        format!("unknown volume {volume:?}; available volumes: {}", names.join(", "))
    })?;
    let resolved_dir = safe_join_dir(root, &target_dir)?;

    Ok((passthrough_flags, resolved_dir))
}

/// Runs one rsync push: checks the request against the volumes, acks, then splices the
/// stream onto a spawned `rsync --server`.
pub async fn handle(volumes: &Volumes, connection: &Connection, mut iroh_send: SendStream, iroh_recv: RecvStream, args: &[String]) -> anyhow::Result<()> {
    let client = short_id(connection.remote_id());
    let (passthrough_flags, resolved_dir) = match resolve(volumes, args) {
        Ok(v) => v,
        Err(msg) => {
            println!("rsync [{client}] refused: {msg}");
            return reject(iroh_send, connection, ACK_REFUSED, msg).await;
        }
    };
    println!("rsync [{client}] -> {}", resolved_dir.display());
    Ack::ack().encode(&mut iroh_send).await?;

    // `passthrough_flags` already ends with rsync's own "." source marker; only the
    // destination token is replaced, with the resolved path.
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

    let bridge_result = proxy_process_streams(iroh_recv, iroh_send, child_stdout, child_stdin).await;
    let _ = stderr_task.await;

    let status = child.wait().await?;
    info!("rsync --server exited with {status}");

    bridge_result?;
    Ok(())
}
