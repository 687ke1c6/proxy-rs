use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Iroh proxy (SOCKS5 + HTTP + tunnel) — server and client modes")]
pub struct Cli {
    /// defaults to ~/.proxy-rs
    #[arg(short = 'd', long, value_name = "DIR", env = "PROXY_RS_CONFIG_DIR", global = true)]
    pub config_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run as a server, exposing configured volumes to clients
    Server(ServerArgs),
    /// Run as a client: TCP proxy (socks5/http/tunnel), file sender, volume lister, or rsync transport
    Client(ClientArgs),
}

#[derive(Args)]
pub struct ServerArgs {
    /// directory to expose to clients over --file, as name:path (repeatable)
    #[arg(short = 'v', long = "volume")]
    pub volumes: Vec<String>,
}

#[derive(Args)]
pub struct ClientArgs {
    /// iroh ticket to connect to
    #[arg(short, long, env = "PROXY_RS_NODE_ID", global = true)]
    pub node_id: Option<String>,
    /// saved name for the server node id
    #[arg(long, env = "PROXY_RS_NAME", global = true)]
    pub name: Option<String>,

    #[command(subcommand)]
    pub mode: ClientMode,
}

#[derive(Subcommand)]
pub enum ClientMode {
    /// Run a local SOCKS5 proxy
    Socks5(Socks5Args),
    /// Run a local HTTP proxy
    Http(HttpArgs),
    /// Forward a local port to a fixed remote host:port through the server (ssh -L style)
    Tunnel(TunnelArgs),
    /// Send a file to a server volume
    File(FileArgs),
    /// List the volumes the server has exposed via -v/--volume
    Volumes(VolumesArgs),
    /// rsync `-e`/`--rsh` transport over iroh — pass it to rsync's `-e`, don't run it directly
    #[command(after_help = SYNC_RSH_EXAMPLE)]
    SyncRsh(SyncRshArgs),
}

#[derive(Args)]
pub struct Socks5Args {
    /// local address to listen on, e.g. 127.0.0.1:1080
    #[arg(short, long, env = "PROXY_RS_LISTEN")]
    pub listen: String,
}

#[derive(Args)]
pub struct HttpArgs {
    /// local address to listen on, e.g. 127.0.0.1:8080
    #[arg(short, long, env = "PROXY_RS_LISTEN")]
    pub listen: String,
}

#[derive(Args)]
pub struct TunnelArgs {
    /// local address to listen on, e.g. 127.0.0.1:9000
    #[arg(short, long, env = "PROXY_RS_LISTEN")]
    pub listen: String,
    /// fixed remote host to forward every connection to
    #[arg(long, env = "PROXY_RS_REMOTE_HOST")]
    pub remote_host: String,
    /// fixed remote port to forward every connection to
    #[arg(long, env = "PROXY_RS_REMOTE_PORT")]
    pub remote_port: u16,
}

#[derive(Args)]
pub struct FileArgs {
    /// local file to send
    #[arg(env = "PROXY_RS_FILE")]
    pub file: String,
    /// allow overwriting existing files
    #[arg(short, long, env = "PROXY_RS_OVERWRITE")]
    pub overwrite: bool,
    /// destination directory, as volume[/dir]; the file keeps its local name.
    /// Required unless the server has exactly one volume configured, in which
    /// case it's the default
    #[arg(short = 't', long, env = "PROXY_RS_TARGET")]
    pub target: Option<String>,
}

#[derive(Args)]
pub struct VolumesArgs {}

const SYNC_RSH_EXAMPLE: &str = "\
Example (push ./localdir into the server's `home` volume, under backup/):
  rsync -av -e \"'/path/to/proxy-rs' client sync-rsh -n <node-id>\" ./localdir/ \"x:home/backup/\"

rsync spawns this command in place of ssh. The host before the `:` is a placeholder
(ignored; the server is picked by -n/--name), and the path after it must start with a
volume name the server exposes (see `client volumes`). -n/--name is required, since
stdin is rsync's pipe and the interactive node menu can't be shown. Push only; rsync
must be installed on both ends.";

#[derive(Args)]
pub struct SyncRshArgs {
    /// everything rsync appends to the `-e` command: the placeholder host, then
    /// `rsync --server <flags> . <volume[/dir]>`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub argv: Vec<String>,
}
