use anyhow::Result;
use clap::Parser;

mod cli;
mod client;
mod config_dir;
mod http;
mod protocols;
mod server;
mod socks5;
mod stream_helpers;

use cli::{Cli, ClientArgs, ClientMode, Command, FileArgs, HttpArgs, ServerArgs, Socks5Args, SyncRshArgs, TunnelArgs, VolumesArgs};
use client::client::{run_list_volumes, run_send_file, run_sync_rsh, run_tcp_client, ProxyType};

fn print_banner() {
    println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // In sync-rsh mode stdout is rsync's protocol pipe, so the banner is skipped and
    // logs go to stderr (rsync passes that through to the user) instead of stdout.
    let is_sync_rsh = matches!(&cli.command, Command::Client(ClientArgs { mode: ClientMode::SyncRsh(_), .. }));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error")),
        );
    if is_sync_rsh {
        subscriber.with_writer(std::io::stderr).init();
    } else {
        subscriber.init();
        print_banner();
    }

    if let Some(dir) = cli.config_dir {
        config_dir::set_config_dir_override(dir);
    }

    match cli.command {
        Command::Server(ServerArgs { volumes }) => server::run_server(volumes).await,
        Command::Client(ClientArgs { node_id, name, mode }) => match mode {
            ClientMode::Socks5(Socks5Args { listen }) => {
                run_tcp_client(ProxyType::Socks5, listen, None, node_id, name).await.or_else(|e: anyhow::Error| anyhow::bail!("Failed to run SOCKS5 client: {e:#}"))
            }
            ClientMode::Http(HttpArgs { listen }) => {
                run_tcp_client(ProxyType::Http, listen, None, node_id, name).await.or_else(|e: anyhow::Error| anyhow::bail!("Failed to run HTTP client: {e:#}"))
            }
            ClientMode::Tunnel(TunnelArgs { listen, remote_host, remote_port }) => {
                run_tcp_client(ProxyType::Tunnel, listen, Some((remote_host, remote_port)), node_id, name).await.or_else(|e: anyhow::Error| anyhow::bail!("Failed to run tunnel client: {e:#}"))
            }
            ClientMode::File(FileArgs { file, overwrite, target }) => {
                run_send_file(file, node_id, name, target, overwrite).await.or_else(|e: anyhow::Error| anyhow::bail!("Failed to run file send: {e:#}"))
            }
            ClientMode::Volumes(VolumesArgs {}) => {
                run_list_volumes(node_id, name).await.or_else(|e: anyhow::Error| anyhow::bail!("Failed to list volumes: {e:#}"))
            }
            ClientMode::SyncRsh(SyncRshArgs { argv }) => {
                let result = run_sync_rsh(argv, node_id, name).await;
                // Exit explicitly instead of returning: tokio's stdin reader sits on a
                // blocking thread that runtime shutdown would otherwise wait on until
                // rsync closes our stdin, which it may only do after we've exited.
                let code = match result {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("sync-rsh failed: {e:#}");
                        1
                    }
                };
                std::process::exit(code);
            }
        },
    }
}
