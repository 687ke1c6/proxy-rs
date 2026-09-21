# Split `server`/`client` into subcommands — implementation plan

Scope: CLI surface only (`src/main.rs`). No change to `server.rs`, `client.rs`,
`client_helpers.rs`, or the wire protocols — every `run_*`/`run_server` function already
takes exactly the parameters it needs, so this is a dispatch/parsing restructuring, not a
behavior rewrite (one deliberate behavior change is called out below).

Target CLI:

```
proxy-rs server -v media:/media -v home:/home/user
proxy-rs client --name remote-server --list-volumes
proxy-rs client -l socks5://127.0.0.1:1080 -n <node-id>
proxy-rs client -f some_large_file.log -n <node-id> -t home/subfolder
proxy-rs -d /custom/config/dir server -v media:/media   # global flag, either position
```

## Steps, in order

- [x] **`main.rs`: restructure `Args` into a `Cli` + `Command` subcommand enum.**
  - [x] Top-level `Cli` struct keeps only `config_dir` (`-d`/`--config-dir`, env
        `PROXY_RS_CONFIG_DIR`), marked `#[arg(global = true)]` — confirmed working in
        both positions: `proxy-rs -d <dir> server ...` and `proxy-rs server -d <dir> ...`.
  - [x] `#[derive(Subcommand)] enum Command { Server(ServerArgs), Client(ClientArgs) }`
        on `Cli`, with `ServerArgs`/`ClientArgs` as separate `#[derive(Args)]` structs
        (needed so `ClientArgs` can carry its own `#[command(group(...))]`).
  - [x] `ServerArgs` gets `volumes: Vec<String>` (`-v`/`--volume`, repeatable, as
        today).
  - [x] `ClientArgs` gets everything currently client-side: `node_id` (`-n`,
        `PROXY_RS_NODE_ID`), `name` (`--name`, `PROXY_RS_NAME`), `listen` (`-l`,
        `PROXY_RS_LISTEN`), `file` (`-f`, `PROXY_RS_FILE`), `list_volumes`
        (`--list-volumes`), `overwrite` (`-o`, `PROXY_RS_OVERWRITE`), `target` (`-t`,
        `PROXY_RS_TARGET`).

- [x] **Client mode selection: make "exactly one of `-l`/`-f`/`--list-volumes`" an
      explicit, enforced rule instead of an implicit fallthrough.**
  - `client` mode with *none* of `-l`/`-f`/`--list-volumes` has no fallback to fall
    into (unlike the old flat `Args`, where omitting `-l`/`-f` silently meant "run as
    server") — confirmed as the intended behavior change, not an accident.
  - [x] Enforced with a clap `ArgGroup` (`required = true`, `listen`/`file`/
        `list_volumes` in the group) on `ClientArgs`, so both `--help`
        (`<--listen <LISTEN>|--file <FILE>|--list-volumes>` in the usage line) and the
        parse-time error message reflect the constraint — no runtime `bail!` needed.
        Verified: zero mode flags and two-at-once both produce clap's own clear error
        and exit code 2, before any connection is attempted.

- [x] **Wire dispatch in `main()`**:
  - [x] `Command::Server(ServerArgs { volumes })` → `server::run_server(volumes).await`
        (unchanged call).
  - [x] `Command::Client(ClientArgs { .. })` → same three `run_tcp_client` /
        `run_send_file` / `run_list_volumes` calls as today, moved under the match
        arm, same `.or_else(...)` error-context wrapping each already had. The
        `list_volumes` arm uses `debug_assert!` rather than re-checking, since the
        `ArgGroup` already guarantees exactly one of the three is set by the time
        dispatch runs.

- [x] **Update [tests/e2e/socks5_proxy.sh](../tests/e2e/socks5_proxy.sh)**, the only
      non-documentation place that invokes the binary programmatically:
  - [x] Server startup: `"$BIN"` → `"$BIN" server`.
  - [x] Client startup: `"$BIN" -l "socks5://127.0.0.1:$SOCKS5_PORT" -n "$NODE_ID"` →
        `"$BIN" client -l "socks5://127.0.0.1:$SOCKS5_PORT" -n "$NODE_ID"`.
  - [x] Ran the script directly end to end (docker compose was reachable in this
        session) — passed, including the curl-through-proxy assertion. Also surfaced
        and fixed a real regression: the default `error`-only log level (from earlier
        in this session) silenced the `Server NodeId: ...` line the script greps for,
        and which every real server operator needs to see. Fixed by changing that one
        line in [server.rs](../src/server.rs) from `info!` to `println!` — essential
        output, not a log line, so it stays visible regardless of `RUST_LOG`.

- [x] **Update [CLAUDE.md](../CLAUDE.md)**:
  - [x] "Commands" section: prefix every example with `server`/`client`, and added the
        previously-undocumented `--list-volumes` and volume-targeted `-f`/`-t` examples.
  - [x] "Mode selection (main.rs)" section: rewritten for the `server`/`client`
        subcommands and the `ArgGroup`-enforced client mode selection. Updated the
        `--config-dir`/`-d` paragraph to describe it as a global flag.
  - [x] Fixed the stale env-var list in that section — was missing `PROXY_RS_NAME` and
        `PROXY_RS_TARGET` (added in earlier work on this branch), independent of the
        subcommand change itself.
  - [x] Skimmed the rest of the file — no other flat-flag examples found outside the
        two sections above.
  - Not touched, flagged as separate pre-existing doc debt rather than pulled into
    this diff: the "Protocol layer" table in CLAUDE.md still only lists 3 protocols
    (Ping/TCP Proxy/File Send) and omits `List Volumes` and the `volume`/`target_dir`
    fields added to `FileSendHeader` in earlier work on this branch. Worth a follow-up
    pass, but unrelated to the CLI subcommand split.

- [x] **Manual smoke test** (in addition to the e2e test above), against a scratch
      `--config-dir` in both global-flag positions:
  - [x] `proxy-rs server -v home:/some/dir` starts and exposes the volume.
  - [x] `proxy-rs client --list-volumes -n <id>` lists it.
  - [x] `proxy-rs client -f <path> -n <id> -t home/sub` sends a file — landed at
        `home/sub/note.log` as expected.
  - [x] `proxy-rs client -l socks5://127.0.0.1:1080 -n <id>` still proxies — listener
        confirmed up via a raw TCP connect.
  - [x] `proxy-rs client -n <id>` (no mode flag) errors clearly instead of silently
        doing nothing (see earlier ArgGroup verification).
  - [x] `proxy-rs client -l ... -f ... -n <id>` (two mode flags) errors clearly (see
        earlier ArgGroup verification).
  - [x] `proxy-rs -d <dir> server ...` and `proxy-rs server -d <dir> ...` both work
        (verified earlier alongside the `ArgGroup` work).
