use std::io::IsTerminal;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

/// Animated "still trying" line for while the client connects to the server:
/// `⠋ Connecting to "Name" [3f1eb...58d] 3s`. On a terminal it ticks in place on stdout;
/// otherwise (piped, docker logs) it prints `<msg>...` once as a plain line and nothing more.
/// Cleared when finished or dropped, so an error printed afterwards starts on a clean line.
pub struct Spinner(Option<ProgressBar>);

const TICK: Duration = Duration::from_millis(80);

/// Frame sets, used in turn: each new spinner, and each failed retry (`next_frames`), takes
/// the next one. Each ends with a space: indicatif's "finished" frame, never shown.
const FRAME_SETS: &[&str] = &[
    "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ", // 6-dot braille (sits high in the cell)
    "⣾⣽⣻⢿⡿⣟⣯⣷ ",   // 8-dot braille (full cell height)
    "◐◓◑◒ ",       // rotating half circle
    "|/-\\ ",      // classic ASCII
];

/// Index into `FRAME_SETS` for the next spinner (or `next_frames` call).
static NEXT_FRAME_SET: AtomicUsize = AtomicUsize::new(0);

/// The spinner style, with the next frame set in turn.
fn next_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} {msg} {elapsed:.dim}")
        .expect("valid spinner template")
        .tick_chars(FRAME_SETS[NEXT_FRAME_SET.fetch_add(1, Ordering::Relaxed) % FRAME_SETS.len()])
}

impl Spinner {
    pub fn start(msg: impl Into<String>) -> Self {
        let msg = msg.into();
        if !std::io::stdout().is_terminal() {
            println!("{msg}...");
            return Spinner(None);
        }
        let pb = ProgressBar::with_draw_target(None, ProgressDrawTarget::stdout());
        pb.set_style(next_style());
        pb.set_message(msg);
        pb.enable_steady_tick(TICK);
        Spinner(Some(pb))
    }

    /// Switches to the next frame set, e.g. after each failed connection attempt. Message
    /// and elapsed time carry on, since it's the same spinner.
    pub fn next_frames(&self) {
        if let Some(pb) = &self.0 {
            pb.set_style(next_style());
        }
    }

    /// Prints a line above the spinner (or plainly, when there's no spinner).
    pub fn println(&self, line: impl AsRef<str>) {
        match &self.0 {
            Some(pb) => pb.println(line),
            None => println!("{}", line.as_ref()),
        }
    }

    /// Removes the spinner line; the caller prints the outcome (`Connected to ...`).
    pub fn finish(self) {}
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if let Some(pb) = self.0.take() {
            pb.finish_and_clear();
        }
    }
}
