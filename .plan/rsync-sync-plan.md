# `sync-rsh` + rsync ALPN (Phase 1), `client sync` convenience wrapper (Phase 2)

Background: rsync doesn't know or care what transport carries it. `ssh` (or anything
set via `-e`/`--rsh`/`RSYNC_RSH`) has exactly one job: given a command line, run it on
the peer and hand back a duplex byte pipe to that command's stdio. Local rsync then
speaks its own wire protocol directly over that pipe — checksums, delta blocks,
compression, permissions, exclude filters, all of it, with zero awareness of what's
underneath. This plan replaces `ssh` with a thin proxy-rs subcommand that does the same
job over an iroh connection instead of a TCP+SSH session, and has the *server* spawn a
real local `rsync --server` process instead of `sshd` doing it. No reimplementation of
rsync's algorithm — we only own the transport and the destination-path trust boundary.

Split into two phases: **Phase 1** builds the actual plumbing — the new ALPN, the
server-side process spawning, and the hidden `sync-rsh` client subcommand — and is
tested by invoking real `rsync` directly with `-e` pointed at `sync-rsh` by hand.
**Phase 2** is purely the `proxy-rs client sync <path>` convenience wrapper that
constructs that same `rsync -e ...` invocation for you. Phase 1 is a fully working,
independently useful feature on its own (anyone comfortable with rsync's `-e` flag can
use it as-is); Phase 2 is ergonomics on top, not new capability.

## Mechanism, end to end

1. *(Phase 2)* User runs `proxy-rs client sync ./localdir -n <node-id> -t home/backup`.
2. *(Phase 2, or done by hand today with Phase 1 alone)* Something shells out to the
   **local system's `rsync` binary**, roughly:
   ```
   rsync -a -e "'<path-to-this-proxy-rs-binary>' client sync-rsh --node-id <node-id> --" \
       ./localdir/ "placeholder:home/backup/"
   ```
   `sync-rsh` is a hidden subcommand (own binary, own `client` mode — not user-facing,
   hidden from `--help`) that rsync's `-e` mechanism spawns as a child process.
3. *(Phase 1)* Local rsync computes its own `--server ...` argv (flags depend on
   `-a`/`-z`/etc.) and invokes `sync-rsh <placeholder-host> rsync --server <flags> .
   home/backup/` — i.e. `sync-rsh` receives that whole command as trailing argv,
   exactly as `ssh` would.
4. *(Phase 1)* `sync-rsh` ignores the placeholder host and the literal
   `rsync`/`--server` tokens (it already knows which node to reach from its own
   `--node-id`/`--name` flag), opens an iroh connection on a new ALPN, sends the
   remaining argv (flags + the destination path token) in one small header, waits for
   an ack, then — once acked — simply splices its own stdin/stdout onto the iroh
   stream via the **existing** `stream_helpers::proxy_streams` helper. No subprocess
   spawning on the client side at all; rsync already did that when it spawned
   `sync-rsh`.
5. *(Phase 1)* Server accepts the connection, decodes the header, and **does not
   trust the argv's destination-path token as a filesystem path**. It parses that
   token exactly like `--target` is parsed today (`volume[/dir]`), resolves it
   through the *existing* volume-allowlist + traversal guard, and only then spawns a
   real local `rsync --server <the client's flags, passed through verbatim> . <our
   own resolved, validated absolute path>` — substituting our path for the client's,
   never the client's flags for anything filesystem-related.
   `tokio::process::Command` wires that child's stdin/stdout to the iroh stream,
   again via `proxy_streams` — full reuse on both ends.
6. *(Phase 1)* Real rsync protocol negotiation (versions, checksums, deltas) happens
   entirely between the two actual rsync processes, exactly as it would over ssh.
   Neither `sync-rsh` nor the server's protocol handler parses any of it — they're a
   pipe.

The only new filesystem-adjacent trust decision is step 5's argv split (flags:
pass-through, path: server-resolved) — everything else is either "reuse
`proxy_streams`" or "reuse the volume/traversal logic file-send already has."

## Decisions (recommended defaults — confirm or override)

| Question | Recommendation |
|---|---|
| **Argv trust boundary**: pass through the client's rsync flags (`-a`, `-z`, exclude filters, etc.) verbatim, or hardcode a fixed flag set server-side? | **Pass through flags, never the path.** Flags like compression/archive-mode/filters aren't filesystem-referencing by themselves — only the trailing destination-path argument is dangerous, and that's the one thing we always discard and replace with our own `safe_join`-validated path. This preserves real rsync functionality (users can `-z`, exclude-filter, etc.) without trusting anything that could escape the volume. *(Phase 1.)* |
| **Direction**: push only (client → server, like `client file`/directory-upload), or also support pull (server → client)? | **Push only for v1.** Matches this tool's existing directionality everywhere else. Pull is symmetric future work (server becomes `--sender`) if actually wanted later. *(Phase 1.)* |
| **`--delete`** (removing server-side files that no longer exist locally — the one genuinely destructive rsync flag): allow it, or block it? | **⚠️ REVISED, not yet enforced.** Originally planned as a string-match blocklist on `--delete*` tokens, but rsync folds many negotiated options into a single short-flag bundle (e.g. `-logDtprxze.iLsfxC`) rather than always using separate long tokens, and this session has no root to install `rsync` and verify empirically whether delete-related behavior can hide in that bundle. A blocklist with an unverified blind spot is worse than no blocklist (false sense of safety), so **Phase 1 first-pass passes all flags through unmodified, `--delete` included** — the "purely additive" guarantee is **not currently enforced**. Must be revisited (allowlist known-safe flags instead of blocklisting dangerous ones is the leading candidate) once Phase 1's smoke test runs against a real `rsync` binary and we can see the actual argv shape. Tracked as a required follow-up, not a "maybe later." *(Phase 1.)* |
| **Default volume for rsync's destination-path token**: same "auto-default to the sole configured volume" behavior `--target` has for file-send, or always require an explicit volume name? | **Always require an explicit volume name.** rsync's own syntax always needs some non-empty destination-path string anyway (e.g. `home/backup`), so there's no clean way to "omit" it the way `--target` is optional for `client file` — the default-volume case wouldn't be reachable in practice. Simpler resolution logic, no ambiguous-empty-string handling needed. *(Phase 1.)* |
| **Server rsync stderr**: capture and log server-side, inherit into the server process's own stderr, or discard? | **Capture and log via `tracing`**, not piped over the wire (rsync's own protocol doesn't expect stderr multiplexed into it) — most useful for diagnosing a failed sync without polluting the actual protocol stream. *(Phase 1.)* |
| **Exit-status propagation**: ssh has a side-channel for the remote command's exit code; a dumb bidirectional pipe doesn't. Do we need one? | **Start without one, verify empirically.** rsync's own protocol usually carries enough success/failure signal for the common case. Flag explicitly as a smoke-test item — if `$?` doesn't reliably reflect server-side failures, revisit (e.g. a final one-byte status frame after the pipe closes). *(Phase 1.)* |

## Phase 1 — `sync-rsh` + rsync ALPN

- [x] **New protocol directory** `src/protocols/rsync/` (`alpn.rs`, `rsync_header.rs`,
      `rsync_protocol_handler.rs`, `mod.rs`), matching the existing per-protocol
      convention. ALPN: `proxy-rs/rsync/1`.
  - [x] `RsyncHeader { version: u8, argv: Vec<String> }` — `argv` is everything after
        the placeholder host token (flags + trailing path); `Vec<String>` needs no new
        codec work, `StreamCodec for Vec<T>` already exists from the list-volumes
        feature. Server replies with the existing `Ack` type (reuse, not a new type).
        (`rsync_protocol_handler.rs` deferred to the "Server handler" step below —
        `alpn.rs`/`rsync_header.rs`/`mod.rs` done, builds clean.)

- [x] **Refactor `safe_join`** in
      [file_send_protocol_handler.rs](../src/protocols/file_send/file_send_protocol_handler.rs)
      to extract a `safe_join_dir(root, target_dir) -> Result<PathBuf, String>` (the
      traversal-guard + `create_dir_all` + canonicalize-and-check-prefix logic, minus
      the trailing filename join). `safe_join` becomes `safe_join_dir(...).and_then(|dir|
      validate file_name and join it)`. `rsync`'s handler resolves a *directory*, not a
      file, so it needs the shared piece without the filename requirement.
      (Landed in a new [volume_paths.rs](../src/protocols/volume_paths.rs) at the
      `src/protocols/` level rather than inside `file_send/`, so `rsync`'s handler can
      reuse it without reaching into a sibling protocol's internals — keeps each
      protocol directory self-contained per CLAUDE.md's documented convention.
      Regression-checked: `client file` still sends and lands content correctly.)

- [x] **Server handler** (`RsyncServerProtocolV1`, holding the same
      `Arc<HashMap<String, PathBuf>>` volumes map `FileServerProtocolV1` already
      does): implemented in
      [rsync_protocol_handler.rs](../src/protocols/rsync/rsync_protocol_handler.rs),
      wired into [server.rs](../src/server.rs)'s router. Build is clean (zero
      warnings — `RsyncHeader`/`RSYNC_ALPN_V1` are no longer dead code), and
      `client volumes`/`client file` regression-checked as still working after the
      router change.
  - [x] Decode `RsyncHeader`. Parse `argv`: verify `argv[0] == "--server"` (reject
        anything else outright — this is the one hard gate on what we'll ever exec).
        Take the **last** element as the client's destination-path token; everything
        between as pass-through flags.
  - [x] ⚠️ No `--delete` filtering in this first pass — see the revised Decisions
        row above. Passthrough flags are forwarded unmodified. Tracked as a required
        follow-up once real `rsync` argv shape is verified.
  - [x] Parse the destination-path token as `volume[/dir]`, volume name required
        (no default-volume fallback, per the Decisions table) — `parse_volume_path`
        added alongside `safe_join_dir` in `volume_paths.rs`, analogous to but
        independent from the client-side `parse_target`. Resolved via
        `safe_join_dir` against the configured volumes.
  - [x] On success: `Ack::ack()`. On failure: `Ack::no_ack(...)` + `finish_and_close`
        (same pattern, same race avoided, as file-send).
  - [x] Spawn `tokio::process::Command::new("rsync").args(["--server",
        ...passthrough_flags, resolved_path])` — **note, changed from the plan as
        written**: no separate hardcoded `.` pushed between flags and path. Caught
        during implementation: real rsync's `--server` argv already has `.` as the
        second-to-last token (its own "source args start here" marker), which is
        already captured inside `passthrough_flags` (everything between `argv[0]`
        and the last element) — adding our own `.` on top would have duplicated it.
        **Also unverified against a real rsync binary** — if this assumption about
        argv shape is wrong, rsync will fail loudly with a protocol error rather than
        silently misbehaving, which the smoke test (still pending a machine with
        `rsync` installed) will catch. No new dependency — `tokio`'s `"full"` feature
        set (already enabled in [Cargo.toml](../Cargo.toml)) includes `process`.
  - [x] Bridge `iroh_recv` ↔ child stdin/stdout via **`stream_helpers::proxy_streams`**
        (reused as-is, no new bridging code): `proxy_streams(iroh_recv, iroh_send,
        child_stdout, child_stdin)`. Stderr drained concurrently (own `tokio::spawn`
        task, line-by-line into `tracing::warn!`) rather than after the bridge
        completes — otherwise a full stderr pipe buffer would block the child and
        stall the whole transfer.
  - [x] `child.wait().await` after the pipe ends; log exit status.

- [x] **Client: hidden `sync-rsh` subcommand** (`ClientMode::SyncRsh`, not wired into
      user-visible `--help` — clap supports `#[command(hide = true)]`). Takes
      `--node-id`/`--name` (reuse `resolve_node_id`, same as every other client mode)
      plus a trailing variadic argv (`#[arg(trailing_var_arg = true,
      allow_hyphen_values = true)]`) capturing everything rsync appends. Drops the
      placeholder host + literal `rsync` token, sends the rest as `RsyncHeader.argv`,
      waits for `Ack`, then on success splices `tokio::io::stdin()`/`stdout()` onto the
      iroh stream via `proxy_streams` — same reuse as the server side.
      (Implemented as `run_sync_rsh` in [client.rs](../src/client/client.rs),
      `SyncRshArgs` in [cli.rs](../src/cli.rs). Extra details found necessary during
      implementation: stdout *is* the rsync pipe, so `main.rs` skips the banner and
      routes `tracing` to stderr in this mode, and all status output uses `info!`
      rather than `println!`; `-n`/`--name` is required, since the interactive
      node-id menu can't read from stdin (that's rsync's pipe); rejects argv whose
      second token isn't literally `rsync`; ends via `std::process::exit` so tokio's
      blocking stdin reader can't stall runtime shutdown. Verified against real
      rsync 3.4.1 — see smoke-test results below.)

- [ ] **Docs**: new row in [CLAUDE.md](../CLAUDE.md)'s protocol table, and a note that
      `sync-rsh` is an internal implementation detail (hidden from `--help`) meant to
      be pointed at by rsync's `-e` flag — by hand for now, until Phase 2 wraps it.

- [ ] **Manual smoke test** (drives `sync-rsh` directly via real `rsync -e`, since
      `client sync` doesn't exist yet):
  - [x] Basic push:
        `rsync -a -e "'<proxy-rs>' client sync-rsh --node-id <id> --" ./localdir/ "placeholder:home/backup/"`
        — confirm content matches server-side byte-for-byte (same check used in the
        directory-upload plan).
  - [x] Re-run the same command with no changes — confirm rsync actually skips
        unchanged files (the whole point of delta-transfer; if this doesn't hold,
        something's wrong with how flags are passed through).
  - Results (rsync 3.4.1, both ends in the devcontainer): basic push of files +
    subdir + symlink landed identical (`diff -r` clean, exit 0); no-change rerun
    transferred nothing ("speedup is 1,829"); `home/../../etc/` and unknown volume
    `nope/x/` both rejected by the server ack, surfacing as rsync exit 12. Server
    argv observed: `rsync --server -vlogDtpre.iLsfxCIvu . <resolved path>` —
    **confirms the `.` marker assumption** (it's already inside
    `passthrough_flags`). With `--delete`: `rsync --server -logDtpre.iLsfxCIvu
    --delete . <path>` — `--delete` arrives as its **own long token**, not folded
    into the short bundle, and is currently **honored** (a server-only file was
    removed), confirming the known gap below.
  - [ ] Modify one file, re-run — confirm only that file's delta transfers (verify via
        rsync's own `-v` output, or a network byte-count sanity check).
  - [ ] Add `--delete` to the local rsync invocation — confirm it's stripped/rejected
        server-side, not silently honored.
  - [x] Unknown volume / traversal attempt via a hand-crafted destination-path token —
        confirm it's rejected the same way file-send already rejects these (reusing
        `safe_join_dir` should make this basically automatic, but verify).
  - [ ] Confirm `$?` after the `rsync` command reflects a deliberately-induced
        server-side failure (e.g. disk full, permission error) reasonably — this is
        the open exit-status question from the Decisions table.
  - [ ] Confirm behavior when `rsync` isn't installed on the server host — should be a
        clear error (process spawn failure), not a silent hang.

## Phase 2 — `client sync` convenience wrapper

- [ ] **Client: user-facing `client sync <path>` subcommand** (`ClientMode::Sync`,
      `SyncArgs { path: String, target: Option<String>, compress: bool }` — mirrors
      `FileArgs`'s shape). Resolves `current_exe()`, constructs the `-e` command string
      (quoting/escaping the exe path — rsync word-splits `-e`'s value itself, not a
      real shell, so use careful manual quoting or a small crate like `shlex` rather
      than hand-rolled splitting), and `tokio::process::Command::new("rsync")`s the
      local sync with that `-e`, `-a` (`-z` if `--compress`), the local path, and
      `"placeholder:{volume}/{target_dir}/"` as arguments. Inherits stdio so the
      user sees real rsync's own progress output directly (rsync already has good
      progress/verbosity flags — no need to reinvent what `copy_with_progress` does
      for file-send here).

- [ ] **Docs**: update [CLAUDE.md](../CLAUDE.md)'s "Mode selection" section with
      `client sync`, and note it (and Phase 1's `sync-rsh`) require `rsync` installed
      on both the local machine and the server host.

- [ ] **Manual smoke test**: repeat Phase 1's smoke test list via
      `proxy-rs client sync` instead of a hand-written `rsync -e ...` invocation,
      confirming identical results — this phase should change zero behavior, only
      ergonomics.

## Explicitly out of scope for v1 (both phases)

- Pull direction (server → client).
- Any UI/progress work beyond what rsync's own CLI output already provides — this is
  the one client mode that doesn't need `copy_with_progress`, since real rsync already
  has mature progress reporting built in.
- Exit-status side-channel, unless the smoke test shows it's actually needed.

## ⚠️ Known gap, not yet closed

`--delete` (and any other destructive rsync flag) is **not currently blocked** —
Phase 1's server handler passes all negotiated flags through to the spawned `rsync
--server` process unmodified. This is a deliberate, tracked deferral (see the
Decisions table) made because this session has no root access to install `rsync` and
verify the real `--server` argv shape, and a blocklist implemented against guessed
argv shape risks a false sense of safety worse than no blocklist at all. **Before this
is used against anything you'd mind losing, verify on a machine with real `rsync`
installed exactly how `--delete` (and friends) appear in server-mode argv, and either
confirm a blocklist is sufficient or implement the allowlist alternative.**

A second, related unverified assumption from implementing the "Server handler" step:
the spawned `rsync --server` argv is built as `["--server", ...passthrough_flags,
resolved_path]`, with **no separately-inserted `.` marker** — the code assumes
`passthrough_flags` (everything between `argv[0]` and the client's destination-path
token) already ends with the `.` real rsync's server-mode invocation is documented to
include. If that assumption is wrong, this fails loudly (rsync protocol error) rather
than silently, but it's still unverified against a real binary and should be checked
in the same pass as the `--delete` verification above.
