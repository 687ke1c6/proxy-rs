use anyhow::Result;
use clap::error::ErrorKind;
use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches};

mod allowlist;
mod cli;
mod client;
mod config;
mod errors;
mod http;
mod identity;
mod protocols;
mod server;
mod signals;
mod socks5;
mod stream_helpers;

use cli::{CLIENT_ARG_IDS, Cli, ClientArgs, Command, ProxyArgs, RsyncArgs, TunnelArgs};
use client::client::{run_listen, run_pipe};
use protocols::stream::{stream_protocol_handler::Mode, target_policy::TargetPolicy};

/// Start of each mode's one-line startup summary, e.g. `proxy-rs v0.1.2 socks5 targets=*:* clients=any`.
pub const BANNER: &str = concat!(env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"));

/// What a client invocation (no subcommand) does.
enum ClientRun {
    Listen { addr: String, retry: bool },
    Pipe { args: Vec<String> },
}

/// Works out the client mode from the flags. Only flags given on the command line count
/// for conflicts, so an exported `PROXY_RS_LISTEN` doesn't get in the way of `--pipe`.
fn client_run(client: &ClientArgs, on_cli: impl Fn(&str) -> bool) -> Result<ClientRun, String> {
    if client.pipe {
        if on_cli("listen") {
            return Err("--listen and --pipe can't be used together".into());
        }
        if on_cli("retry") {
            return Err("--retry only applies to --listen".into());
        }
        return Ok(ClientRun::Pipe { args: client.args.clone() });
    }
    if let Some(first) = client.args.first() {
        return Err(format!(
            "unrecognized command '{first}': server modes are socks5, http, tunnel and rsync; \
             extra arguments are only accepted with --pipe"
        ));
    }
    match &client.listen {
        Some(addr) => Ok(ClientRun::Listen { addr: addr.clone(), retry: client.retry }),
        None => Err("nothing to do: pass a server mode (socks5, http, tunnel, rsync), or --listen <ADDR> / --pipe to run a client".into()),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let usage_error = |msg: String| -> ! { Cli::command().error(ErrorKind::ArgumentConflict, msg).exit() };
    let on_cli = |id: &str| matches.value_source(id) == Some(ValueSource::CommandLine);

    // A subcommand is a server mode (or whoami); without one, the client flags say what to do.
    let client_run = match &cli.command {
        Some(_) => {
            if CLIENT_ARG_IDS.iter().any(|id| on_cli(id)) {
                usage_error("--listen, --pipe, -n/--node-id, --name and --retry are client flags; they can't be combined with a server mode or whoami".into());
            }
            None
        }
        None => Some(client_run(&cli.client, on_cli).unwrap_or_else(|msg| usage_error(msg))),
    };

    // First, so every banner (and anything else) sees the right config file.
    if let Some(path) = cli.config.clone() {
        config::set_config_override(path);
    }

    // In pipe mode stdout is the data stream, and whoami's stdout is meant to be captured
    // (`--allow $(proxy-rs whoami)`), so the banner and logs go to stderr instead. Every
    // other mode prints its own banner line, with its settings appended.
    let quiet_stdout = match (&cli.command, &client_run) {
        (Some(Command::Whoami), _) => Some("whoami"),
        (_, Some(ClientRun::Pipe { .. })) => Some("pipe"),
        _ => None,
    };

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error")),
        );
    if let Some(mode) = quiet_stdout {
        subscriber.with_writer(std::io::stderr).init();
        eprintln!("{BANNER} {mode}{}", config::banner_suffix());
        // whoami prints the id on stdout itself; pipe can't (stdout is the data stream).
        if mode == "pipe" {
            eprintln!("{}", config::load_secret_key()?.public());
        }
    } else {
        subscriber.init();
    }

    let ClientArgs { node_id, name, .. } = cli.client;
    match (cli.command, client_run) {
        (Some(Command::Socks5(ProxyArgs { target, allow })), _) => {
            server::run_server(Mode::Socks5(TargetPolicy::parse(&target)?), allow.allow).await
        }
        (Some(Command::Http(ProxyArgs { target, allow })), _) => {
            server::run_server(Mode::Http(TargetPolicy::parse(&target)?), allow.allow).await
        }
        (Some(Command::Tunnel(TunnelArgs { remote_host, remote_port, allow })), _) => {
            server::run_server(Mode::Tunnel { host: remote_host, port: remote_port }, allow.allow).await
        }
        (Some(Command::Rsync(RsyncArgs { volumes, allow })), _) => {
            server::run_server(Mode::Rsync(server::parse_volumes(&volumes)?), allow.allow).await
        }
        (Some(Command::Whoami), _) => {
            println!("{}", config::load_secret_key()?.public());
            Ok(())
        }
        (None, Some(ClientRun::Listen { addr, retry })) => {
            run_listen(addr, node_id, name, retry).await.map_err(|e| failed("Client failed", e))
        }
        (None, Some(ClientRun::Pipe { args })) => {
            let result = run_pipe(args, node_id, name).await;
            // Exit explicitly instead of returning: tokio's stdin reader sits on a
            // blocking thread that runtime shutdown would otherwise wait on until the
            // parent (rsync, ssh) closes our stdin, which it may only do after we've exited.
            let code = match result {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("proxy-rs pipe failed: {}", errors::display_chain(&e));
                    1
                }
            };
            std::process::exit(code);
        }
        (None, None) => unreachable!("no subcommand always resolves to a client mode"),
    }
}

/// The error for a failed client: `<what>: <cause chain>`. A client stopped by a signal
/// has already said so (`signals::until_signal`) and closed its connection, so it exits
/// quietly with the signal's code instead of printing an `Error:` line.
fn failed(what: &str, e: anyhow::Error) -> anyhow::Error {
    if let Some(signals::Interrupted(signal)) = e.downcast_ref() {
        std::process::exit(signal.exit_code());
    }
    anyhow::anyhow!("{what}: {}", errors::display_chain(&e))
}
