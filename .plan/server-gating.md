# Plan: opt-in server features

Status: **P0–P3 done**

## Problem

`server` registers every protocol unconditionally ([src/server.rs](../src/server.rs) `Router::builder`).
In particular `TCP_PROXY_ALPN_V1` is an open proxy: anyone with the node ID can make the
server dial any `host:port`, including its own localhost and LAN.

## Decisions

| Topic | Decision |
|---|---|
| Proxy gating | One TCP ALPN, gated by a `-t` target allowlist. No per-mode ALPNs, because socks5/http/tunnel are identical on the server |
| `-t` env var | `PROXY_RS_TUNNEL`, comma-separated |
| Port ranges | Supported now (`host:8000-8100`) |
| `tcp-proxy/1` | Hard cut; server only speaks `proxy-rs/tcp/2` |
| `-t '*'` | Allowed, with a prominent startup `WARN` |
| File send | Own opt-in flag `-f/--file`; `-v` only declares volumes |
| Client allowlist | Separate follow-up change |

### Feature → gate mapping

| Feature | ALPN | Enabled by |
|---|---|---|
| Ping | `proxy-rs/ping/1` | always |
| TCP proxy (socks5 / http / tunnel) | `proxy-rs/tcp/2` | ≥1 `-t` |
| File send | `proxy-rs/file/1` | `-f` (requires ≥1 `-v`) |
| rsync push | `proxy-rs/rsync/1` | `-r` (requires ≥1 `-v`) |
| List volumes | `proxy-rs/list-volumes/1` | `-f` or `-r` |

Startup errors:
- `-f` or `-r` with no `-v`
- `-v` with neither `-f` nor `-r`: the volumes would be unreachable
- no feature enabled at all
- any invalid `-t` pattern

### `-t` pattern grammar

`<host-pattern>:<port-pattern>`, or bare `*` meaning `*:*`.

- host: exact (`localhost`), glob (`*.lan`, where `*` matches one or more labels), `*`, or bracketed IPv6 (`[::1]`)
- port: exact (`22`), range (`8000-8100`, inclusive, lo ≤ hi), or `*`
- `*.*` rejected, with a hint to use `*`
- host matching is case-insensitive, trailing `.` stripped
- matched against the hostname **string the client sends, before DNS**: `-t localhost:22` does not admit `127.0.0.1:22`

### `proxy-rs/tcp/2` wire format

1. client → server: `ProxyHeader { host, port }`
2. server → client: `Ack` (`src/protocols/ack.rs`)
   - `0`: allowed and connected
   - `1`: target not allowed (`msg` = reason)
   - `2`: allowed but connect failed (`msg` = io error)
   - `3`: unsupported header version
3. on `0`: raw bidirectional copy as today

---

## Tasks (in priority order)

### P0: close the open proxy

- [x] **Target policy module**: `src/protocols/proxy/target_policy.rs`
  - [x] `TargetPattern::from_str` (host exact/glob/`*`/IPv6, port exact/range/`*`, reject `*.*`)
  - [x] `TargetPolicy { patterns }` with `allows(host, port) -> bool` and `is_open()` (any `*:*`)
  - [x] Unit tests (first in the repo): exact, glob depth, case, trailing dot, IPv6, ranges, `*`, `*.*` rejection, `localhost` ≠ `127.0.0.1`
- [x] **Server CLI**: `ServerArgs` in `src/cli.rs`
  - [x] `-t/--tunnel <pattern>`: `Vec<String>`, `env = "PROXY_RS_TUNNEL"`, `value_delimiter = ','`
  - [x] `-f/--file`: bool, env `PROXY_RS_SERVE_FILE` (**not** `PROXY_RS_FILE`, which is already the client's file-path var)
  - [x] `-r/--rsync`: bool, env `PROXY_RS_RSYNC`
- [x] **Conditional router**: `src/server.rs`
  - [x] Build `Router` from enabled features only
  - [x] Startup validation errors (see list above)
  - [x] Log the enabled features and `-t` patterns on start; `WARN` if the policy is open
  - [x] Optional: warn if `-r` is set and `rsync` isn't on `PATH`
- [x] v1 handler already enforces the policy (a rejected target just gets its stream dropped, so the client sees a closed connection until P1 adds the `Ack`)
- [x] e2e script starts the server with `-t '*'` (otherwise it wouldn't start)

### P1: proper rejection path

- [x] **Server handler v2**: `src/protocols/proxy/`
  - [x] Add `TCP_PROXY_ALPN_V2 = b"proxy-rs/tcp/2"` and remove V1
  - [x] Handler holds `Arc<TargetPolicy>`, checks it before dialing, sends `Ack` 0/1/2
  - [x] Log rejected attempts with the remote node ID and target
  - [x] Replace the `.unwrap()`/`.expect()` calls in the handler with proper errors
  - [x] Header renamed `ProxyHeaderV1` → `ProxyHeader { version: 2, .. }`; wrong version → `Ack` 3
  - [x] Dial `(host, port)` instead of a formatted `host:port` string (bare IPv6 hosts from SOCKS5 now resolve)
- [x] **Client v2**: `src/client/client.rs` (`handle_socks5`, `handle_http`, `handle_tunnel`)
  - [x] Connect on V2 and read the `Ack` before `proxy_streams`
  - [x] SOCKS5: move the success reply out of `socks5::handshake` to after the `Ack`; map 1 → `REP 0x02`, 2 → `REP 0x05`
  - [x] HTTP CONNECT: move `200 Connection Established` out of `http::handshake` to after the `Ack`; map 1 → `403`, 2 → `502`. Plain HTTP gets the same status codes.
  - [x] Tunnel: log the `Ack` message and close the local socket

### P2: tests & docs

- [x] **e2e**: `tests/e2e/socks5_proxy.sh`
  - [x] Target switched from the removed `bun` docker-compose service to an `ncat` HTTP listener the script starts itself on `127.0.0.1:4001`
  - [x] Negative case: server now runs with `-t localhost:4001` only; `localhost:4001` must succeed and `localhost:4002` must be refused with SOCKS5 reply 2 (curl exit 97). Positive curl switched to `--socks5-hostname` so the hostname reaches the allowlist. Checked the test fails with `PROXY_TEST_ALLOWED='*'`
- [x] **Docs**: `CLAUDE.md` and README
  - [x] Server flags, env vars, gate table
  - [x] Pattern grammar and the pre-DNS matching caveat
  - [x] Update the example commands (`server -v media:/media` → `server -v media:/media -f -r`, etc.)

### Found along the way (not in scope)

- [x] `node-ids.yaml` got corrupted when several clients started at once: every start rewrote the file to update `last_used`
  - [x] Removed `last_used` entirely (`--name` covers selecting a server); starts with an already-saved server no longer write the file. Old files with a `last_used:` key still load; it's dropped on the next write
  - [x] `write_node_id_to_file` no longer swallows a read error with `unwrap_or_default()` (a torn read used to wipe every other saved entry)
  - [ ] Still possible: parallel *first-time* starts with the same new `-n` all write. Fix if it bites: atomic temp-file + rename, plus an exclusive `File::lock` on a separate `node-ids.yaml.lock` around re-read/modify/save (never held across prompts)

### P3: client allowlist

Decisions: no allowlist configured → server refuses to start unless `--allow-any`; ids come from `--allow` **and** `<config-dir>/authorized-clients` (merged); the file is re-read on every connection.

- [x] Persistent client key `~/.proxy-rs/client-key`, used by every client endpoint
  - [x] Shared `src/identity.rs` `load_or_create_secret_key` for server and client: mode 0600, race-safe (temp file + `hard_link`, loser reads the winner's key). 10 parallel first-time starts → 1 key
- [x] `proxy-rs client whoami` prints the client's node ID (stdout only the id; no banner, logs to stderr)
- [x] Server `--allow <node-id>` (repeatable, `PROXY_RS_ALLOW` comma-separated) + `--allow-any` (`PROXY_RS_ALLOW_ANY`, conflicts with `--allow`, WARNING)
- [x] `authorized-clients` file (`src/authorized_clients.rs`): one id per line + optional comment, `#` comments; re-read per connection; read/parse error fails closed to `--allow` only; unit tests
- [x] Every ALPN (ping included) wrapped in iroh's `AccessLimit`; refused clients get closed with `not allowed`
- [x] Client `ping_server` turns `not allowed` into an actionable error that includes its own node id
- [x] Startup: refuse if no client allowed (and no `--allow-any`); print `Clients: N via --allow, M in <file>`
- [x] e2e: server `--allow $(client whoami)`; a second client with its own `--config-dir` must be refused
- [x] Docs: README (quick start, flags, allowlist section, whoami, state on disk, security model) and CLAUDE.md
- [ ] Later: per-client feature grants
- [ ] Later (maybe): revoking a client doesn't cut its already-open connections, only new ones
