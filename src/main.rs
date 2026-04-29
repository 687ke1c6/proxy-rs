use anyhow::Result;
use clap::Parser;

mod client;
mod http;
mod protocols;
mod server;
mod socks5;

use client::client::{run_send_file, run_tcp_client};

#[derive(Parser)]
#[command(about = "Iroh proxy (SOCKS5 + HTTP) — server and client modes")]
struct Args {
    /// iroh ticket to connect to (client mode)
    #[arg(short, long)]
    node_id: Option<String>,
    #[arg(short, long)]
    listen: Option<String>,
    #[arg(short, long)]
    file: Option<String>,
    /// allow overwriting existing files
    #[arg(short, long)]
    overwrite: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args = Args::parse();
    if let Some(listen) = args.listen {
        return run_tcp_client(listen, args.node_id).await.or_else(|e: anyhow::Error| anyhow::bail!("Failed to run TCP client: {e:#}"));
    }
    if let Some(file) = args.file {
        return run_send_file(file, args.node_id, args.overwrite).await.or_else(|e: anyhow::Error| anyhow::bail!("Failed to run file send: {e:#}"));
    }
    server::run_server().await
}
