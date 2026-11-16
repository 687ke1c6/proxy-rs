# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                   # build
cargo run -- socks5 --allow <client-id> -t '*'                          # server: SOCKS5 proxy to any target
cargo run -- http --allow <client-id> -t '*.lan:*'                      # server: HTTP proxy to part of the LAN
cargo run -- tunnel --allow <client-id> --remote-host localhost --remote-port 22  # server: every connection goes to one target
cargo run -- rsync --allow <client-id> -v media:/media                  # server: rsync pushes into a volume
cargo run -- --listen 127.0.0.1:1080 -n <node-id>                       # client: forward a local port to a server (any mode)
cargo run -- whoami                                                     # print this node's id (for a server's --allow)
rsync -av -e "'target/debug/proxy-rs' --pipe -n <node-id>" ./dir/ "x:media/subdir/"  # rsync push, client in pipe mode
cargo clippy                  # lint
cargo test                    # run unit tests (-t pattern matcher in src/protocols/stream/target_policy.rs, config file in src/config.rs, allowlist in src/allowlist.rs, message framing in src/protocols/codec.rs)
cargo test -- --ignored       # run the e2e smoke test (see below)
just                          # list the justfile recipes (build/check/e2e/server/listen/sync/docker/armv7 wrappers)
```

The binary is `proxy-rs`. A server and a client on the same machine need different `--config` files (a config file is a node identity, and a node can't connect to itself).

### End-to-end test (`tests/e2e/`)

`tests/e2e.rs` has one `#[ignore]`d test, `socks5_proxy_smoke_test`, wrapping [tests/e2e/socks5_proxy.sh](tests/e2e/socks5_proxy.sh). It's `#[ignore]`d (not run by plain `cargo test`) because it needs `ncat` installed and real processes/ports: it starts an `ncat` HTTP listener on `127.0.0.1:4001` that replies `hello`, gets the client's node id via `whoami --config client.toml`, starts a `proxy-rs socks5` server (`--config server.toml`, with `-t localhost:4001` and `--allow <that id>` only) and a `proxy-rs --listen` client (both built by `cargo test` via `CARGO_BIN_EXE_proxy-rs`, run in a temp working directory with `HOME` also pointed at it, so nothing touches the real `~/.proxy-rs/config.toml`), waits for the server to print its NodeId and the client's listener to come up, then curls `http://localhost:4001/` through the proxy with `--socks5-hostname` (the `-t` allowlist matches hostnames, so curl must not resolve locally) and asserts on the response body, then curls `localhost:4002` and asserts it's refused with SOCKS5 reply `0x02` (curl exit 97). Then it starts a second client with its own `--config` (so a different key) and asserts it fails with "not in its allowlist". Finally it stops the server with SIGINT and asserts the client (no `--retry`) exits non-zero with `Disconnected from`. Run it with `cargo test -- --ignored`.

## Architecture

This is a P2P proxy built on [iroh](https://github.com/n0-computer/iroh). The server exposes itself as an iroh node; clients connect by node ID rather than IP address.

**The server owns the mode; the client is a dumb pipe.** A server runs exactly one mode (`socks5`, `http`, `tunnel`, `rsync`), which decides what every client stream does. A client only binds a local address (or uses stdin/stdout) and forwards bytes; it neither knows nor chooses the server's mode, so one client command works against every server. SOCKS5/HTTP are parsed on the server, and a tunnel's target is server configuration.

### Mode selection (`main.rs`, `cli.rs`)

Top-level subcommands are the server modes plus `whoami`; **no subcommand means client**:
- `socks5 -t <pattern>` / `http -t <pattern>` → proxy server; `-t`/`--target` (required, repeatable) is the allowlist of targets clients' applications may ask for
- `tunnel --remote-host <h> --remote-port <p>` → every client stream is connected to that one target (no `-t`: there's nothing for a client to choose)
- `rsync -v name:path` → rsync push server; `-v`/`--volume` (required, repeatable) declares the directories pushes may write into
- `whoami` → prints this node's id (from the config file's key) and nothing else on stdout: the `proxy-rs vX.Y.Z whoami` banner and logs go to stderr, so `$(proxy-rs whoami)` works; no network
- `--listen <addr>` (no subcommand) → client: forwards every connection accepted on `<addr>` to the server; `--retry` keeps it running without the server
- `--pipe [args...]` (no subcommand) → client: forwards stdin/stdout to the server over one stream, for use as another program's transport: rsync's `-e` (`rsync -av -e "'<proxy-rs>' --pipe -n <node-id>" ./dir/ "x:<volume>/<dir>/"`, against an `rsync` server; rsync appends `<host> rsync --server <flags> . <path>`, which the client forwards untouched as `Open.args`) or ssh's `ProxyCommand` (against a `tunnel` server). Requires `-n`/`--name` (stdin is the data stream, so no interactive menu)

All server modes take `--allow`. The client flags (`ClientArgs`: `--listen`, `--pipe`, `-n`/`--node-id`, `--name`, `--retry`, trailing `args`) are flattened into the top-level `Cli` next to the optional subcommand. clap can't express "these only without a subcommand" while keeping env vars usable, so `main.rs` validates by hand using `ArgMatches::value_source`: only flags given **on the command line** count, so an exported `PROXY_RS_LISTEN` neither breaks a server invocation nor conflicts with `--pipe`. A first positional that isn't a subcommand lands in `args`; without `--pipe` that's reported as `unrecognized command '<x>'`. `--config`/`-c` is global (valid before or after a subcommand).

Env vars: `PROXY_RS_CONFIG`; client `PROXY_RS_LISTEN`, `PROXY_RS_NODE_ID`, `PROXY_RS_NAME`, `PROXY_RS_RETRY`; server `PROXY_RS_TARGET`, `PROXY_RS_ALLOW` (both comma-separated), `PROXY_RS_REMOTE_HOST`, `PROXY_RS_REMOTE_PORT`. An explicit CLI flag takes precedence. `-v`/`--volume` and `--pipe` have no env var.

### Server (`src/server.rs`, `src/protocols/stream/`)

`run_server(mode, allow)` registers two ALPNs on the iroh `Router`: ping (the client's session) and the stream protocol, handled by `StreamServerProtocol { mode: Arc<Mode> }`. `Mode` is `Socks5(TargetPolicy) | Http(TargetPolicy) | Tunnel { host, port } | Rsync(Volumes)`. Startup prints one summary line (`proxy-rs vX.Y.Z socks5 targets=*:* clients=any config=<file>`, `... tunnel -> host:22 ...`, `... rsync ...` followed by `  volume <name> -> <path>` lines), WARNINGs (`-t '*'` open proxy; no allowlist; `rsync` not on `PATH`), then the bare node id as the last line.

**Per stream** (`stream_protocol_handler.rs`): the client's first frame is `Open { args }`, and the server answers with an `Ack` before the stream becomes raw bytes:
- `tunnel`: dials the fixed target *then* acks (0, or `ACK_CONNECT_FAILED` with the error, so the client can report a dead target), then `proxy_streams`.
- `socks5` / `http`: acks at once, then runs `socks5::handshake` / `http::handshake` **over the iroh stream** (`tokio::io::join(recv, send)`; both are generic over `AsyncRead + AsyncWrite`), checks the target against `-t`, dials, and answers in that protocol: SOCKS5 `REP 0x00` / `0x02` (not allowed) / `0x05` (connect failed); HTTP `200 Connection Established` for CONNECT (plain requests get the upstream's own response; their original request bytes are replayed to the upstream as `preamble`) / `403` / `502` / `400`. The handshake is bounded by `HANDSHAKE_TIMEOUT`, since it's now remote input.
- `rsync` (`rsync.rs`): validates `Open.args` (`<host> rsync --server <flags> . <volume[/dir]>`; the host is ignored), resolves the destination inside a volume (`volume_paths.rs`: no `..`, symlink escapes caught by canonicalizing), acks, then splices the stream onto a spawned `rsync --server` (`proxy_process_streams`). An unknown volume is refused with the list of valid names (there is no separate volume-listing protocol).
- Non-empty `args` to any other mode is refused (`ACK_REFUSED`).

After a final refusal the handler `linger`s (finish the send stream, wait up to 10s for the client to close), so the refusal isn't lost when the handler returns and the connection drops. One console line per proxied connection: `tcp [<short id>] -> host:port` (the target exactly as asked for, no DNS lookup shown), `... refused: not allowed by -t`, `... failed: <error>`; rsync prints `rsync [<short id>] -> <dir>`.

The `Open` frame exists because a QUIC stream is invisible to the acceptor until the opener writes: with a tunnel to a server-speaks-first protocol (ssh, SMTP) the local application sends nothing, so without it the server would never see the stream.

**Client allowlist** (`src/allowlist.rs`): enforced by `ClientGate`, an iroh `EndpointHooks` impl installed on the server endpoint (`.hooks(...)`), whose `after_handshake` sees every incoming connection (remote id + ALPN) before the `Router` hands it to a protocol handler, so protocol handlers know nothing about it. Allowed = `--allow` ids (parsed at startup) ∪ ids in the config file's `[clients]` table (`<node-id> = "<label>"`), and the file is **re-read on every connection** (`ClientAllowlist::check`) so edits add/revoke/relabel without a restart; a read/parse error logs and fails closed to `--allow` only. Refused connections are closed with reason `not allowed` (same as iroh's `AccessLimit`); the client's `ping_server` checks `conn.close_reason()` for that and reports its own node id with a hint. For accepted connections on the ping ALPN only (every client holds its ping connection open as a session for as long as it runs; the stream ALPN, one connection per forwarded socket, would be too noisy) the gate prints `Client connected: <label> [<short id>]` (or just the short id if unlabeled), and spawns a task on `conn.weak_handle().closed()` (a weak handle, never a `Connection` clone: iroh's hook docs warn that a strong handle kept from a hook keeps the connection alive) that prints `Client disconnected: ...` with a ` (reason)` suffix unless the close was deliberate (`stream_helpers::close_reason_suffix`). The connected line ends with ` (relay)`/` (direct)` (the connection's selected iroh path, `stream_helpers::PathKindWatch::suffix`; omitted if none is selected yet), and that same task prints `Client <client> now direct` (or `now relay`) whenever the selected path switches kind (`PathKindWatch::run`, driven by iroh's `Connection::path_events()` `Selected` events; it subscribes before reading the initial kind so no switch is missed, and re-reads on `Lagged`) — connections usually start on the relay and get upgraded once holepunching succeeds. If both sources are empty (checked per connection, so it tracks file edits), the gate admits everyone; startup prints `clients=any` and a WARNING. A file read/parse error still fails closed, even with no `--allow`.

`-t` patterns (`target_policy.rs`): `host:port` or bare `*` (= `*:*`). Host is exact, `*`, `*.domain` (one or more labels in front, not the bare domain), or bracketed IPv6 `[::1]`. Port is exact, `*`, or inclusive `lo-hi`. `*.*` and unbracketed IPv6 are rejected at parse time. Hosts are compared lowercased with a trailing `.` stripped, against the **hostname string the application sends, before DNS**, so `localhost` ≠ `127.0.0.1`.

### Protocol layer (`src/protocols/`)

Two protocols, each a directory with `alpn.rs` (constant string), `*_header.rs` (message struct), `*_protocol_handler.rs` (server-side `iroh::protocol::ProtocolHandler` impl) and `mod.rs`; `stream/` also holds the mode-specific pieces (`rsync.rs`, `target_policy.rs`, `volume_paths.rs`).

**Wire format** (`src/protocols/codec.rs`) — every protocol message is a plain `#[derive(Serialize, Deserialize)]` struct, sent as a frame: `u32` big-endian length, then that many bytes of [postcard](https://docs.rs/postcard). The blanket `StreamCodec` impl gives every such type `msg.encode(&mut send)` / `T::decode(&mut recv)`. Frames over `MAX_FRAME_LEN` (64 KiB) are refused on encode (nothing written) and on decode (before allocating), so no length is ever silently truncated, and a reader always consumes exactly one whole message.

**Versioning is by ALPN only** — messages carry no version field. Postcard isn't self-describing (no field tags, no skipping unknown fields), so *any* change to a message struct (including `Ack`) is a breaking change: bump the trailing number of that protocol's ALPN in its `alpn.rs` (constants are unversioned, e.g. `STREAM_ALPN`; the version lives only in the string). A mismatched peer then fails the QUIC handshake (TLS alert 120, "no application protocol") instead of mis-decoding bytes; the client's `connect` helper in `client.rs` turns that into `server does not accept protocol <alpn>: it runs an incompatible proxy-rs version` (both ALPNs are always on, so it can't be a disabled feature).

| Protocol | ALPN | Purpose |
|---|---|---|
| Ping | `proxy-rs/ping/2` | Pre-flight health check, then held open as the client's session |
| Stream | `proxy-rs/stream/1` | Client sends `Open { args }`, receives `Ack` (0 go ahead, 1 refused, 2 tunnel connect failed), then raw bytes interpreted by the server's mode |

### Client flow (`src/client/`)

Every mode starts with a one-line summary: `BANNER` (`proxy-rs vX.Y.Z`, in `main.rs`) plus mode and settings, e.g. `proxy-rs v0.2.0 listen 127.0.0.1:1080 retry`; servers and `run_listen` print their own, whoami/pipe's is printed by `main.rs` to stderr. Every banner ends with ` config=<file>` (`config::banner_suffix`, absolute path, shown whether default or set by `--config`/`PROXY_RS_CONFIG`); `main.rs` applies the override before anything prints. Node ids on the console are shortened with `identity::short_id` (`3f1eb...58d`: first 5 and last 3 chars) everywhere except the two places they must be copied: the server's last startup line (the bare id) and `whoami`.

`client.rs` always pings the server first. `ping_server` returns the ping connection, which callers keep open (`_session`) for their whole run; a `NotAllowed` error type marks an allowlist refusal. Clients behave like ncat/telnet by default: no server, or losing it, means exit with an error (non-zero).

**`--listen`** (`run_listen`): shows `Connecting to <server>` right after resolving the server, before the endpoint comes up (getting online is part of the wait), as a `client::spinner::Spinner`: an indicatif spinner (each new spinner takes the next frame set from `FRAME_SETS` in turn, and with `--retry` `maintain_session` calls `Spinner::next_frames()` after each failed attempt, so every retry is visible; message and elapsed time carry on) with elapsed time on stdout when it's a terminal, else one plain `Connecting to <server>...` line; it's cleared on `finish()` or drop (so an error starts on a clean line) and replaced by `Connected to ...`. It runs `maintain_session` alongside its `TcpListener`; that prints `Connected to <server> (relay|direct)` / `Connection to <server> now direct|relay` (on a path switch, like the server's gate). Without `--retry` (the default) it pings *before* binding the listener (so an offline server exits before anything listens) and hands that connection to `maintain_session`, which returns `Err(Disconnected from <server>[ (reason)])` when it closes, ending the client. With `--retry` (the banner gets a ` retry` suffix) the listener binds immediately (the spinner keeps going: `maintain_session` takes it and finishes it on connect) and `maintain_session` prints `Disconnected from ...` then starts a `Reconnecting to <server>` spinner instead, prints `Waiting for <server>: <err>` above the spinner (which keeps its `Connecting to`/`Reconnecting to` text) on the first failed attempt (at startup, and after each loss) only, retries 2s after each attempt ends, and only returns on `NotAllowed`. How long one attempt takes is up to iroh: a node id with no published address fails in ~3-5s (the client's `connect` helper reports it as `server [...] is not online (no address found for its node id)`), while a server killed without a clean close (its DNS record outlives it) times out after QUIC's idle timeout (~30s), which is also when a connected client notices such a server is gone. A server stopped with a signal closes cleanly and is noticed at once.

Each accepted socket gets a task (`forward_socket`): a new connection on `STREAM_ALPN`, `Open { args: [] }`, then `forward_after_ack`, which sends local bytes to the server **without waiting for the `Ack`** (waiting would cost every connection a round trip; the server just buffers them) while the other direction reads the `Ack` first and then copies back. TCP-style: half-close propagates and it's done when both directions are. A refused `Ack` ends both directions and drops the local socket; it surfaces as a logged `error!` (there's no local protocol to report it through). SOCKS5/HTTP refusals never reach this path: they're in-band replies from the server.

**`--pipe`** (`run_pipe`): pings, opens one stream with the trailing args, waits for the `Ack` (a refusal is printed as `proxy-rs pipe failed: server refused the connection: <msg>`), then `pipe_stdio`: ssh-style, done when the server's side ends; EOF on stdin only half-closes (the reply still comes back) and stdin staying open doesn't keep it running (rsync only closes it after we exit). It only logs (stdout is the data stream), never retries, and `main.rs` exits explicitly afterwards (tokio's stdin reader sits on a blocking thread).

Byte-shuffling helpers live in `stream_helpers.rs`: `proxy_streams` (`tokio::io::copy_bidirectional`, server side of TCP modes) and `proxy_process_streams` (`select!`, done when either side ends; server side of rsync). Both finish the send stream and await `stopped()` before returning, because the caller then drops the `Connection`, which would otherwise discard in-flight data.

`client_helpers.rs` resolves the server node id from `-n`/`--name` against the config file's `[servers]` table (`<node-id> = "<name>"`, `""` for unnamed; keyed by id, so one name per id). Saving goes through `config::save_server`, which changes an existing entry in place or adds one, and drops the entries of servers whose name is being taken over; everything else in the file, comments included, is kept. Names are case-sensitive. Unnamed servers are shown as just `[3f1eb...58d]` (`server_label`). Names are auto-generated as `Adjective Animal` using built-in word lists (no extra deps). Passing `--node-id` always skips the menu below and uses that ID directly (saving it under an auto-generated name if not already known). Otherwise (including when only one, or zero, IDs are saved) `dialoguer::Select` always prompts the user with every saved entry as `Name [3f1eb...58d]`, plus a trailing `<new>` entry; picking `<new>` prompts with `dialoguer::Input` for a node ID, validated as a parseable `EndpointId`, then prompts for a name (defaulting to an auto-generated `Adjective Animal`, shown in `[brackets]` and used as-is on empty Enter), then saves both (`add_new_server`). If the id is already saved, the name prompt defaults to its current name. If a *different* saved server already has the entered name, a `dialoguer::Confirm` (`[y/N]`, default no) asks whether to replace it: yes removes that server's entry, no re-prompts for the name. (With both `-n` and `--name` on the command line, the same clash is an error instead, since there's no one to ask.)

### Signals (`src/signals.rs`)

Ctrl-C (SIGINT), SIGTERM and SIGHUP are caught and reported as `Received <signal>, shutting down` (`signals::announce`; on a terminal it first clears the line holding the echoed `^C`). SIGKILL can't be caught, so it can't be reported. The server awaits `shutdown_signal()`, announces, then `router.shutdown()` closes every client connection cleanly (clients notice at once) and exits 0. The `--listen` client wraps its work, including `endpoint.online()`, in `until_signal(...)`: on a signal it drops the work (clearing any spinner), announces, and returns `signals::Interrupted`, after which the existing `endpoint.close()` still runs (so the server logs `Client disconnected` at once, not after the idle timeout); `main.rs`'s `failed()` turns `Interrupted` into a quiet exit with `128 + signo` (130 Ctrl-C, 143 SIGTERM, 129 SIGHUP) instead of an `Error:` line. `--pipe` and whoami keep the default signal behavior (`--pipe`'s stdout is the data stream, and the parent program, e.g. rsync, reports interruptions itself).

### Persistent state: the config file (`src/config.rs`)

One TOML file per node, `~/.proxy-rs/config.toml` by default (`config_path()`), or `--config`/`PROXY_RS_CONFIG` (`main.rs` calls `config::set_config_override()` before anything reads it). There are no separate key or list files, and no server/client distinction in the file: **a config file is a node identity**, used by whichever role runs with it. Several servers on one host, or a server plus a client, each need their own file.

```toml
secret-key = "<hex iroh::SecretKey>"   # determines the node id

[servers]                               # client side: saved servers, <node-id> = "<name>"
[clients]                               # server side: allowlist, <node-id> = "<label>"
```

- `load_secret_key()` creates the file on first use (commented template, mode 0600), race-safely: write a temp file, `hard_link` it into place, which fails if another process won; everyone then reads the winner's key. A hand-written file without `secret-key` gets one added.
- All edits go through `toml_edit` and are written atomically (temp file + `rename`), so comments and hand edits survive and a crash can't truncate the file holding the key. The server never writes the file after creating it; the client writes only `[servers]`.
- `read_ids(path, table)` parses either table (file order; empty string = no name/label); a missing file is empty.

### SOCKS5 / HTTP handling (`src/socks5.rs`, `src/http.rs`)

Both run on the **server**, over the client's iroh stream. `socks5::handshake()` returns `(host, port)` after the method negotiation and CONNECT request, and writes no final reply; the handler sends exactly one `socks5::reply()`. `http::handshake()` returns a `ProxyRequest { host, port, is_connect, preamble }` and writes nothing back. CONNECT tunnels (`is_connect`, empty preamble) get `http::respond_connected()` once the target is connected; plain HTTP requests carry the original request bytes as preamble, which the handler writes to the upstream. Refusals go out via `http::respond_error()`.
