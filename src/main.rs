use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod client;
mod config_dir;
mod http;
mod protocols;
mod server;
mod socks5;
mod stream_helpers;

use client::client::{run_send_file, run_tcp_client};

#[derive(Parser)]
#[command(about = "Iroh proxy (SOCKS5 + HTTP) — server and client modes")]
struct Args {
    /// iroh ticket to connect to (client mode)
    #[arg(short, long, env = "PROXY_RS_NODE_ID")]
    node_id: Option<String>,
    #[arg(short, long, env = "PROXY_RS_LISTEN")]
    listen: Option<String>,
    #[arg(short, long, env = "PROXY_RS_FILE")]
    file: Option<String>,
    /// allow overwriting existing files
    #[arg(short, long, env = "PROXY_RS_OVERWRITE")]
    overwrite: bool,
    /// defaults to ~/.proxy-rs
    #[arg(short = 'd', long, value_name = "DIR", env = "PROXY_RS_CONFIG_DIR")]
    config_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::new("proxy_rs=warn,proxy_rs=info,error"),
        )
        .init();

    let args = Args::parse();
    if let Some(dir) = args.config_dir {
        config_dir::set_config_dir_override(dir);
    }
    if let Some(listen) = args.listen {
        return run_tcp_client(listen, args.node_id).await.or_else(|e: anyhow::Error| anyhow::bail!("Failed to run TCP client: {e:#}"));
    }
    if let Some(file) = args.file {
        return run_send_file(file, args.node_id, args.overwrite).await.or_else(|e: anyhow::Error| anyhow::bail!("Failed to run file send: {e:#}"));
    }
    server::run_server().await
}
