# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                   # build
cargo run -- server --allow <client-id> -t '*'                             # run as server, open TCP proxy (socks5/http/tunnel to anything)
cargo run -- server --allow <client-id> -t localhost:22 -f -r -v media:/media  # run as server: tunnel to one target, file send + rsync into a volume
cargo run -- client socks5 --listen 127.0.0.1:1080 -n <node-id>            # run as SOCKS5 proxy client
cargo run -- client http --listen 127.0.0.1:8080 -n <node-id>              # run as HTTP proxy client
cargo run -- client tunnel --listen 127.0.0.1:9000 --remote-host remote_host --remote-port 3000 -n <node-id>  # run as tunnel (ssh -L style) client
cargo run -- client file <path> -n <node-id> -t media/subdir               # send a file to a server volume
cargo run -- client volumes -n <node-id>                                   # list the server's exposed volumes
cargo run -- client whoami                                                 # print this client's node id (for the server's --allow)
rsync -av -e "'target/debug/proxy-rs' client sync-rsh -n <node-id>" ./dir/ "x:media/subdir/"  # rsync push over iroh
cargo clippy                  # lint
cargo test                    # run unit tests (-t pattern matcher in src/protocols/proxy/target_policy.rs, authorized-clients parsing in src/authorized_clients.rs)
cargo test -- --ignored       # run the e2e smoke test (see below)
```

The binary is `proxy-rs`.

### End-to-end test (`tests/e2e/`)

`tests/e2e.rs` has one `#[ignore]`d test, `socks5_proxy_smoke_test`, wrapping [tests/e2e/socks5_proxy.sh](tests/e2e/socks5_proxy.sh). It's `#[ignore]`d (not run by plain `cargo test`) because it needs `ncat` installed and real processes/ports: it starts an `ncat` HTTP listener on `127.0.0.1:4001` that replies `hello`, gets the client's node id via `client whoami`, starts a `proxy-rs` server (with `-t localhost:4001` and `--allow <that id>` only) and a SOCKS5 `proxy-rs` client (both built by `cargo test` via `CARGO_BIN_EXE_proxy-rs`, run against a temp working directory with `HOME` also pointed at it, so `~/.proxy-rs/server-key` and `~/.proxy-rs/nodes.yaml` don't touch the real dev files), waits for the server to print its NodeId and the client's SOCKS5 listener to come up, then curls `http://localhost:4001/` through the proxy with `--socks5-hostname` (the `-t` allowlist matches hostnames, so curl must not resolve locally) and asserts on the response body, then curls `localhost:4002` and asserts it's refused with SOCKS5 reply `0x02` (curl exit 97). Finally it starts a second client with its own `--config-dir` (so a different key) and asserts it fails with "not in its allowlist". Run it with `cargo test -- --ignored`.

## Architecture

This is a P2P proxy built on [iroh](https://github.com/n0-computer/iroh). The server exposes itself as an iroh node; clients connect by node ID rather than IP address. All traffic is multiplexed over iroh's QUIC-based transport using ALPN protocol negotiation.

### Mode selection (main.rs)

The single binary takes a `server` or `client` subcommand (`clap::Subcommand`, each with its own `#[derive(Args)]` struct). `client` itself carries a second, nested subcommand (`ClientMode`) selecting exactly one of its seven modes — no `ArgGroup` needed, the subcommand enum is the mutual-exclusion mechanism:
- `server` → exposes only the features enabled by flags (see "Server feature gating" below); `-v`/`--volume` declares directories for `-f`/`-r` to write into
- `client socks5` / `client http` → TCP proxy client, `-l`/`--listen <addr>` is purely a local bind address now (previously a `protocol://host:port` URL whose scheme selected the mode — replaced by these being separate subcommands)
- `client tunnel` → ssh `-L`-style forwarding; `-l`/`--listen <addr>` (local bind) plus `--remote-host`/`--remote-port` (fixed remote target)
- `client file <path>` → file sender client (positional path), writing into a server volume (`-t`/`--target` selects which)
- `client volumes` → prints the server's exposed volumes as `name:path`, one per line
- `client whoami` → prints this client's node id (from `client-key`) and nothing else: no banner, logs to stderr, like `sync-rsh`, so `$(proxy-rs client whoami)` works; no network
- `client sync-rsh` → rsync transport over iroh (`proxy-rs/rsync/1`), not run directly but passed to rsync's `-e` in place of `ssh`: `rsync -av -e "'<proxy-rs>' client sync-rsh -n <node-id>" ./dir/ "x:<volume>/<dir>/"`. The host before `:` is a placeholder; the path must start with a server volume name. Push only; requires `-n`/`--name` (stdin is rsync's pipe, so no interactive menu), and `rsync` installed on both ends

`-n`/`--node-id` and `--name` are global on `client` (valid before or after the mode subcommand), since every mode needs to resolve a server node id. `--config-dir`/`-d` is global on the whole binary (valid before or after `server`/`client` and, for `client`, before or after the mode too) and overrides the persistent state directory (see below); if omitted, it defaults to `~/.proxy-rs`.

Every other flag can also be set via an env var (`PROXY_RS_NODE_ID`, `PROXY_RS_NAME`, `PROXY_RS_LISTEN`, `PROXY_RS_REMOTE_HOST`, `PROXY_RS_REMOTE_PORT`, `PROXY_RS_FILE`, `PROXY_RS_OVERWRITE`, `PROXY_RS_TARGET`, `PROXY_RS_CONFIG_DIR`, and on the server `PROXY_RS_ALLOW_TARGET`, `PROXY_RS_ALLOW` (both comma-separated), `PROXY_RS_SERVE_FILE`, `PROXY_RS_RSYNC`, `PROXY_RS_ALLOW_ANY`); an explicit CLI flag takes precedence. The server's `-f` uses `PROXY_RS_SERVE_FILE`, not `PROXY_RS_FILE`, because the latter is already the client's file-path var; likewise the server's `-t/--target` uses `PROXY_RS_ALLOW_TARGET`, not the client `file` mode's `PROXY_RS_TARGET`, so a shell that exports the client var can't silently open a server's proxy. `PROXY_RS_LISTEN` is shared by `socks5`/`http`/`tunnel`'s separate `--listen` fields — safe to reuse, since only one mode is ever active per invocation. `-v`/`--volume` (server) has no env var, since it's repeatable.

### Server feature gating (`src/server.rs`, `src/protocols/proxy/target_policy.rs`)

`run_server` only registers the ALPNs for enabled features on the iroh `Router`; unregistered ALPNs fail the QUIC handshake. Ping is always on. `-t/--target <pattern>` (repeatable; `--tunnel` is a hidden alias, its old name) enables TCP proxying via `TargetPolicy`, `-f/--file` enables file send, `-r/--rsync` enables rsync push, and list-volumes is on if `-f` or `-r` is. Startup fails if nothing is enabled, if `-v` is given without `-f`/`-r`, or `-f`/`-r` without `-v`. `-t '*'` prints a WARNING (open proxy); `-r` without `rsync` on `PATH` prints a WARNING.

**Client allowlist** (`src/authorized_clients.rs`): enforced by `ClientGate`, an iroh `EndpointHooks` impl installed on the server endpoint (`.hooks(...)`), whose `after_handshake` sees every incoming connection (remote id + ALPN) before the `Router` hands it to a protocol handler, so protocol handlers know nothing about it. Allowed = `--allow` ids (parsed at startup) ∪ ids in `<config-dir>/authorized-clients` (one per line, `#` comments; text after the id is an optional label), and the file is **re-read on every connection** (`ClientAllowlist::check`) so edits add/revoke/relabel without a restart; a read/parse error logs and fails closed to `--allow` only. Refused connections are closed with reason `not allowed` (same as iroh's `AccessLimit`); the client's `ping_server` checks `conn.close_reason()` for that and reports its own node id with a hint. For accepted connections on the ping ALPN only (every client operation pings first; other ALPNs, e.g. one connection per proxied socket, would be too noisy) the gate prints `Client connected: <label> [<short id>]`, or the full id if unlabeled. Startup fails if both sources are empty, unless `--allow-any` (which `conflicts_with` `--allow`, gives the gate no allowlist so it admits everyone, and prints a WARNING).

There's deliberately one TCP ALPN for SOCKS5/HTTP/tunnel, not one per mode: they're identical on the server (dial `host:port`), so per-mode gating couldn't separate anything; a client could just use whichever ALPN is enabled. The real distinction is which targets are reachable, hence the allowlist.

`-t` patterns: `host:port` or bare `*` (= `*:*`). Host is exact, `*`, `*.domain` (one or more labels in front, not the bare domain), or bracketed IPv6 `[::1]`. Port is exact, `*`, or inclusive `lo-hi`. `*.*` and unbracketed IPv6 are rejected at parse time. Hosts are compared lowercased with a trailing `.` stripped, against the **hostname string the client sends, before DNS**, so `localhost` ≠ `127.0.0.1`.

### Protocol layer (`src/protocols/`)

Each protocol is a self-contained directory with four files: `alpn.rs` (constant string), `*_header.rs` (message struct), `*_protocol_handler.rs` (server-side `iroh::protocol::ProtocolHandler` impl), and `mod.rs`.

**Custom wire format** — all protocol messages use `StreamCodec`, a custom async encode/decode trait in `src/protocols/codec.rs`. Primitive impls are hand-written; struct impls are generated by `#[derive(StreamCodec)]` from the `proxy-rs-derive` proc macro. The `#[codec(bitpack)]` attribute packs consecutive `bool` fields into a single byte on the wire.

The main protocols and their ALPN strings (also `proxy-rs/list-volumes/1` and `proxy-rs/rsync/1`):
| Protocol | ALPN | Purpose |
|---|---|---|
| Ping | `proxy-rs/ping/1` | Pre-flight health check before every client operation |
| TCP Proxy | `proxy-rs/tcp/2` | Sends `ProxyHeader { version: 2, host, port }`, receives `Ack` (0 connected, 1 not allowed by `-t`, 2 connect failed, 3 bad version), then raw bidirectional TCP |
| File Send | `proxy-rs/file/1` | Sends `FileSendHeader`, receives `Ack`, streams bytes, receives final `Ack` |

### Client flow (`src/client/`)

Node ids on the console are shortened with `identity::short_id` (`3f1eb...58d`: first 5 and last 3 chars) everywhere except the two places they must be copied: the server's `Iroh node listening [...]` startup line and `client whoami`. `client.rs` always pings the server first, then opens the appropriate ALPN connection. For TCP proxy mode it runs a `TcpListener` and spawns a task per connection; each task does a SOCKS5 or HTTP handshake locally before opening the iroh stream. The local success reply (SOCKS5 `REP 0x00`, HTTP `200 Connection Established`) is **not** sent by `socks5::handshake`/`http::handshake`; the handlers send it only after `open_proxy_stream` gets the server's `Ack`, and map refusals to SOCKS5 `0x02`/`0x05`/`0x01` or HTTP `403`/`502`. Tunnel mode has no local protocol, so a refusal just closes the local socket and logs an error. `client tunnel --listen <local> --remote-host <host> --remote-port <port>` skips the local handshake entirely (ssh `-L`-style): the listener binds `<local>` and every accepted connection is proxied straight to the fixed `<host>:<port>` via `TCP_PROXY_ALPN_V2`. The server side needs no changes for this — `ProxyHeader { host, port }` is an opaque target the server checks against its `-t` policy and dials, so SOCKS5/HTTP/tunnel differ only in how the client-local `(host, port)` is obtained; `main.rs` picks which via the `ClientMode` subcommand and passes a typed `ProxyType` + bind address + optional `(remote_host, remote_port)` into the single shared `run_tcp_client`, rather than parsing a `protocol://` URL scheme at runtime.

Byte-shuffling (iroh bi-stream ↔ local read/write) lives in `stream_helpers.rs`: TCP uses `proxy_streams` (`tokio::io::copy_bidirectional`, so half-close propagates and it waits for both directions); rsync uses `proxy_process_streams` (`select!`, ssh-style: done when either side ends, since rsync only closes our stdin after we exit). Both finish the send stream and await `stopped()` before returning, because the caller then drops the `Connection`, which would otherwise discard in-flight data.

`client_helpers.rs` manages saved node IDs in `~/.proxy-rs/nodes.yaml` (YAML list of `{ name, key }` entries). Names are auto-generated as `Adjective Animal` using built-in word lists (no extra deps). Passing `--node-id` always skips the menu below and uses that ID directly (saving it under an auto-generated name if not already known). Otherwise (including when only one, or zero, IDs are saved) `dialoguer::Select` always prompts the user with every saved entry as `Name [3f1eb...58d]`, plus a trailing `<new>` entry; picking `<new>` prompts with `dialoguer::Input` for a node ID, validated as a parseable `EndpointId`, then prompts for a name (defaulting to an auto-generated `Adjective Animal`, shown in `[brackets]` and used as-is on empty Enter), then saves both exactly as if the ID had been passed via `-n`.

### Persistent state (`~/.proxy-rs/`)

These files live in `~/.proxy-rs` by default, resolved by `config_dir()` in `src/config_dir.rs`, created on first run:
- `server-key` — hex-encoded `iroh::SecretKey`; determines the server's stable node ID.
- `client-key` / `client-key.pub` — same pair, for the client; every client endpoint uses it so the server's allowlist can recognize it. Both keys are loaded by `identity::load_or_create_secret_key`, which also refreshes the `.pub` file, and are created with mode 0600, race-safely (write temp file, `hard_link` into place, which fails if another process won; everyone then reads the winner's key).
- `authorized-clients` — server-side client allowlist (see "Server feature gating"); if missing, the server creates it at startup (`create_new`, never overwrites) with a comments-only template that allows no one; skipped with `--allow-any`.
- `server-key.pub` — the server's node ID (public key), written/refreshed alongside `server-key` on every server start, so it's readable without parsing the secret key.
- `nodes.yaml` — client-side list of known server node IDs (written by the client).

`main.rs` calls `config_dir::set_config_dir_override()` before dispatching to a mode when `--config-dir`/`-d` is passed, pointing `config_dir()` at that directory instead (used as-is, not joined with `.proxy-rs`).

### HTTP proxy handling (`src/http.rs`)

`http::handshake()` returns a `ProxyRequest { host, port, is_connect, preamble }` and writes nothing back. CONNECT tunnels (`is_connect`, empty preamble) get `http::respond_connected()` from the caller once the server has acked; plain HTTP requests carry the original request bytes as preamble so the server forwards them to the upstream. Refusals go out via `http::respond_error()`.
