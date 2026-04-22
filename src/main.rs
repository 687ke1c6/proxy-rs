use anyhow::Result;
use clap::Parser;

mod client;
mod proxy;
mod server;
mod socks5;

#[derive(Parser)]
#[command(about = "Iroh SOCKS5 proxy — server and client modes")]
struct Args {
    /// iroh ticket to connect to (client mode)
    #[arg(short, long)]
    node_id: Option<String>,
    #[arg(short, long)]
    listen: Option<String>,
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
    match args.listen {
        Some(listen) => client::run(listen, args.node_id).await,
        None => server::run().await
    }
}
