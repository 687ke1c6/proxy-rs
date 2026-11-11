//! Shutdown signals: Ctrl-C (SIGINT), SIGTERM (`kill`, `docker stop`, systemd) and SIGHUP
//! (terminal closed). SIGKILL can't be caught, so it can't be reported: the process just ends.

use std::fmt;
use std::io::IsTerminal;
use std::future::Future;

#[derive(Debug, Clone, Copy)]
pub enum Signal {
    Interrupt,
    Terminate,
    Hangup,
}

impl Signal {
    /// Shell convention for "ended by signal N": 128 + N.
    pub fn exit_code(self) -> i32 {
        128 + match self {
            Signal::Hangup => 1,
            Signal::Interrupt => 2,
            Signal::Terminate => 15,
        }
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Signal::Interrupt => "Ctrl-C (SIGINT)",
            Signal::Terminate => "SIGTERM",
            Signal::Hangup => "SIGHUP",
        })
    }
}

/// Resolves on the first shutdown signal. Until this is first polled, the signals keep their
/// default action (end the process silently).
pub async fn shutdown_signal() -> Signal {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("failed to listen for SIGTERM");
        let mut hup = signal(SignalKind::hangup()).expect("failed to listen for SIGHUP");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => Signal::Interrupt,
            _ = term.recv() => Signal::Terminate,
            _ = hup.recv() => Signal::Hangup,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        Signal::Interrupt
    }
}

/// The error `until_signal` returns when a signal stopped the work; `main` turns it into a
/// quiet exit with `Signal::exit_code` rather than an `Error:` line.
#[derive(Debug)]
pub struct Interrupted(pub Signal);

impl fmt::Display for Interrupted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "interrupted by {}", self.0)
    }
}

impl std::error::Error for Interrupted {}

/// Runs `work` until it finishes or a shutdown signal arrives. On a signal, `work` is dropped
/// first (which clears any spinner it owns), then `Received <signal>, shutting down` is
/// printed and `Interrupted` returned, so the caller can still close its endpoint cleanly.
pub async fn until_signal<T>(work: impl Future<Output = anyhow::Result<T>>) -> anyhow::Result<T> {
    let outcome = tokio::select! {
        result = work => Ok(result),
        signal = shutdown_signal() => Err(signal),
    };
    match outcome {
        Ok(result) => result,
        Err(signal) => {
            announce(signal);
            Err(Interrupted(signal).into())
        }
    }
}

/// Prints `Received <signal>, shutting down`. On a terminal it first clears the current line,
/// which holds the `^C` the terminal echoed (with no newline after it) for a typed Ctrl-C.
pub fn announce(signal: Signal) {
    let clear = if std::io::stdout().is_terminal() { "\r\x1b[2K" } else { "" };
    println!("{clear}Received {signal}, shutting down");
}
