# 9P directory sharing over tunnel — implementation plan

Scope: **Plan A only** — prove a 9P2000.L filesystem server can be reached through the
existing `tunnel://` client mode and mounted locally, then embed that server into the
`proxy-rs` binary. A dedicated 9P ALPN (removing the loopback hop) is deliberately out
of scope here — see "Future work" at the bottom.

## Background

- `tunnel://local_host:local_port:remote_host:remote_port` (already implemented in
  [src/client/client.rs](src/client/client.rs)) forwards arbitrary raw TCP bytes. The
  server's `ProxyServerProtocolV1`
  ([src/protocols/proxy/proxy_protocol_handler.rs](src/protocols/proxy/proxy_protocol_handler.rs))
  just dials whatever `host:port` it's told via `ProxyHeaderV1` — it needs **zero
  changes** to carry 9P traffic.
- 9P2000.L is a single stateful TCP connection with no side-channel/portmapper step, so
  it fits the tunnel model the same way `sshfs` (single TCP connection to port 22)
  would.
- Crate: [`rs9p`](https://github.com/rs9p/rs9p) (crates.io, v0.13.0, tokio-based,
  BSD-3-licensed, `#![forbid(unsafe_code)]`). You implement `rs9p::srv::Filesystem`
  (async trait — `rattach`, `rwalk`, `ropen`, `rread`, `rwrite`, `rreaddir`, etc.) and
  start it with `rs9p::srv_async(filesystem, "tcp!<host>!<port>")`.
- Reference passthrough impl: the same repo publishes `unpfs` (crates.io, v0.13.0) — a
  real directory-passthrough `Filesystem` implementation. It's **binary-only**
  (`crates/unpfs/src/main.rs`, no `lib.rs`), so it can't be pulled in as a Rust
  dependency. Still useful two ways:
  1. Install it standalone (`cargo install unpfs`) for a zero-code Phase 0 spike.
  2. Read its `main.rs`/`utils.rs` as a reference when writing our own embedded
     passthrough `Filesystem` in Phase 1 (BSD-3 permits adapting it with attribution).

## Phase 0 — wire-level spike (no proxy-rs code changes)

Goal: prove 9P2000.L survives the iroh tunnel and a real kernel mount works, before
writing any embedding code.

**Confirmed working end-to-end** — `unpfs` serving `/downloads`, tunneled through the
proxy client, mounted from the host. ✅

- [x] `cargo install unpfs` on the server host (devcontainer is fine).
- [x] Run `unpfs` serving a scratch directory (`/downloads`).
- [x] On the client (existing, unmodified tunnel mode):
      `cargo run -- -l tunnel://127.0.0.1:9000:127.0.0.1:<unpfs-port> -n <node-id>`
- [x] Mount it from the host and confirm it works.
- [ ] Sanity ops through the mount: `ls`, read a file, write a file, `mkdir`, confirm
      changes land in the real directory on the server host and vice versa. (Basic
      mount confirmed; still worth exercising write/mkdir specifically before calling
      Phase 0 fully done.)
- [ ] Note round-trip feel (QUIC framing + per-op 9P round trips can be chattier than
      raw TCP for lots of small files) — eyeball it, no formal benchmark needed yet.

**Note for Phase 1**: this spike ran `unpfs` bound so the port was reachable publicly
on the container (not loopback-only) — fine for a quick spike, but the plan's stated
design is loopback-only + reach it exclusively via the iroh tunnel. Make sure Phase 1's
embedded server binds `127.0.0.1` specifically, not `0.0.0.0`/a public interface.

## Phase 1 — embed into the proxy-rs server binary

Goal: a single self-contained `proxy-rs` binary can serve a directory itself, still
reached via the unmodified tunnel client — no external `unpfs` process to install or
manage.

- [ ] **Dependency**: add `rs9p = "0.13"` to [Cargo.toml](Cargo.toml).
- [ ] **CLI**: add to `Args` in [src/main.rs](src/main.rs):
  - [ ] `--serve-dir <PATH>` (env `PROXY_RS_SERVE_DIR`) — directory to export. Only
        meaningful in server mode (i.e. no `--listen`/`--file`); bail with a clear error
        if combined with either.
  - [ ] `--serve-dir-port <PORT>` (env `PROXY_RS_SERVE_DIR_PORT`), default to an
        unprivileged port (e.g. `5640` — 9P's classic port `564` needs root, avoid that).
- [ ] **New module** (naming TBD, e.g. `src/ninep.rs`): a `Filesystem` impl doing real
      passthrough to the `--serve-dir` path (walk/open/read/write/create/remove/readdir/
      stat/wstat), ported from `unpfs`'s reference implementation. Scope it to what's
      needed for a working mount (list/read/write files and dirs); skip anything unpfs
      itself doesn't need for a basic export unless Phase 0/1 testing shows it's
      required.
- [ ] **Wire into `run_server()`** ([src/server.rs](src/server.rs)): when `--serve-dir`
      is set, spawn a tokio task running
      `rs9p::srv_async(fs, format!("tcp!127.0.0.1!{port}"))` alongside the existing
      `Router` — bound to loopback only, so it's unreachable except through the iroh
      tunnel.
- [ ] **Docs**: new section in [CLAUDE.md](CLAUDE.md) describing `--serve-dir` and the
      `tunnel://...:127.0.0.1:<port>` + `mount -t 9p` recipe, replacing the Phase 0
      standalone-`unpfs` instructions once this lands.
- [ ] **Testing**: repeat the Phase 0 mount/read/write/mkdir smoke test against the
      embedded server instead of standalone `unpfs`; confirm identical behavior.
      Consider whether this is worth an `#[ignore]`d e2e test alongside
      `tests/e2e/socks5_proxy.sh` (needs the `9p` kernel module in the test
      environment — check availability first).

## Future work (explicitly out of scope here) — "Plan B"

A dedicated `proxy-rs/9p/1` ALPN that hands the iroh bidi stream **directly** to an
in-process `rs9p` session, keyed by a server-defined *share name* instead of raw
`host:port`:

- Removes the loopback TCP hop entirely (server never opens a local port for this).
- Share names map to directories server-side, which is safer UX than tunneling to an
  arbitrary `host:port` the way `tunnel://` does today.
- Mechanically it's `proxy_streams`-shaped (same bridging code already used by
  socks5/http/tunnel), just with the "dial `TcpStream::connect(host, port)`" backend
  swapped for "hand this stream to `rs9p` for the requested share."

Revisit once Phase 1 is proven out end-to-end.
