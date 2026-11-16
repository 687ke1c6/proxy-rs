use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

const USAGE: &str = "\
proxy-rs <socks5|http|tunnel|rsync> [OPTIONS]     run a server in that mode
       proxy-rs --listen <ADDR> [-n <NODE_ID>]           run a client: forward a local port to a server
       proxy-rs --pipe -n <NODE_ID> [ARGS]...            run a client: forward stdin/stdout to a server
       proxy-rs whoami                                   print this node's id";

const AFTER_HELP: &str = "\
The server decides what a connection does (its mode); a client only forwards bytes to
it, so the same client command works against any server. One server runs one mode: for
several on a host, give each its own --config file (and so its own node id).";

#[derive(Parser)]
#[command(
    version,
    about = "Iroh proxy: a server (socks5, http, tunnel or rsync mode) reached by node id, and a client that forwards to it",
    override_usage = USAGE,
    after_help = AFTER_HELP,
    arg_required_else_help = true
)]
pub struct Cli {
    /// config file: this node's key, saved servers and allowed clients; created if
    /// missing [default: ~/.proxy-rs/config.toml]
    #[arg(short = 'c', long, value_name = "FILE", env = "PROXY_RS_CONFIG", global = true)]
    pub config: Option<PathBuf>,

    #[command(flatten)]
    pub client: ClientArgs,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Client flags: given without a subcommand. Ids match the field names, for `main`'s
/// "was this given on the command line" check.
#[derive(Args)]
#[command(next_help_heading = "Client options (no subcommand)")]
pub struct ClientArgs {
    /// local address to listen on, e.g. 127.0.0.1:1080; every connection to it is
    /// forwarded to the server
    #[arg(short, long, value_name = "ADDR", env = "PROXY_RS_LISTEN")]
    pub listen: Option<String>,
    /// forward stdin/stdout to the server instead of listening, e.g. as rsync's -e
    /// command or ssh's ProxyCommand
    #[arg(long)]
    pub pipe: bool,
    /// node id of the server to connect to
    #[arg(short, long, env = "PROXY_RS_NODE_ID")]
    pub node_id: Option<String>,
    /// saved name of the server to connect to (or the name to save --node-id under)
    #[arg(long, env = "PROXY_RS_NAME")]
    pub name: Option<String>,
    /// with --listen: keep running if the server is unreachable or lost, waiting for it
    /// at startup and reconnecting whenever it goes away (default: exit with an error)
    #[arg(long, env = "PROXY_RS_RETRY")]
    pub retry: bool,
    /// with --pipe: arguments forwarded to the server (rsync appends its remote command here)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, value_name = "ARGS")]
    pub args: Vec<String>,
}

/// Ids of `ClientArgs`' fields.
pub const CLIENT_ARG_IDS: &[&str] = &["listen", "pipe", "node_id", "name", "retry", "args"];

#[derive(Subcommand)]
pub enum Command {
    /// Run a SOCKS5 proxy server: clients' applications speak SOCKS5 to it
    Socks5(ProxyArgs),
    /// Run an HTTP proxy server (CONNECT and plain http:// requests)
    Http(ProxyArgs),
    /// Run a tunnel server: every client connection goes to one fixed host:port
    Tunnel(TunnelArgs),
    /// Run an rsync server: clients push into its volumes with rsync -e
    #[command(after_help = RSYNC_EXAMPLE)]
    Rsync(RsyncArgs),
    /// Print this node's id (for a server's --allow, or a client's -n) and nothing else
    Whoami,
}

#[derive(Args)]
pub struct AllowArgs {
    /// client node id allowed to connect, as printed by `proxy-rs whoami` there
    /// (repeatable); merged with the config file's [clients]; if both are empty, any
    /// client may connect
    #[arg(long = "allow", value_name = "NODE_ID", env = "PROXY_RS_ALLOW", value_delimiter = ',')]
    pub allow: Vec<String>,
}

#[derive(Args)]
pub struct ProxyArgs {
    /// targets clients may reach, as host:port; host may be `*` or `*.domain`, port may
    /// be `*` or `lo-hi`; bare `*` allows everything (repeatable)
    #[arg(short = 't', long = "target", value_name = "PATTERN", env = "PROXY_RS_TARGET", value_delimiter = ',', required = true)]
    pub target: Vec<String>,
    #[command(flatten)]
    pub allow: AllowArgs,
}

#[derive(Args)]
pub struct TunnelArgs {
    /// host every client connection is forwarded to
    #[arg(long, env = "PROXY_RS_REMOTE_HOST")]
    pub remote_host: String,
    /// port every client connection is forwarded to
    #[arg(long, env = "PROXY_RS_REMOTE_PORT")]
    pub remote_port: u16,
    #[command(flatten)]
    pub allow: AllowArgs,
}

#[derive(Args)]
pub struct RsyncArgs {
    /// directory clients may push into, as name:path (repeatable)
    #[arg(short = 'v', long = "volume", value_name = "NAME:PATH", required = true)]
    pub volumes: Vec<String>,
    #[command(flatten)]
    pub allow: AllowArgs,
}

const RSYNC_EXAMPLE: &str = "\
Example, on a client (push ./localdir into this server's `home` volume, under backup/):
  rsync -av -e \"'/path/to/proxy-rs' --pipe -n <node-id>\" ./localdir/ \"x:home/backup/\"

rsync runs that command in place of ssh. The host before the `:` is a placeholder
(ignored; the server is picked by -n/--name), and the path after it must start with a
volume name. Push only; rsync must be installed on both ends.";
