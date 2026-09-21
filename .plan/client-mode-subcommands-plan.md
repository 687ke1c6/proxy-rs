# Split client `-l <url>` into per-protocol subcommands — implementation plan

Background: `-l`/`--listen <url>` currently overloads one string with two jobs — the
local bind address, *and* protocol selection via URL scheme (`socks5://`, `http://`,
`tunnel://`), with `tunnel://` additionally smuggling a remote target
(`local_host:local_port:remote_host:remote_port`) into what reads like a listen address.
This plan makes `client`'s five modes (`socks5`, `http`, `tunnel`, `file`, `volumes`)
siblings under one nested `#[derive(Subcommand)]`, the same pattern
[cli-subcommands-plan.md](cli-subcommands-plan.md) used for `server`/`client` — so it
supersedes that plan's `ClientArgs` `ArgGroup` (a subcommand enum is a more idiomatic way
to express "exactly one of five mutually-exclusive things" than an `ArgGroup` ever was;
we only used `ArgGroup` last time because `-l`/`-f`/`--list-volumes` hadn't yet been
pulled apart into their own argument sets).

Target CLI:

```
proxy-rs client socks5 --listen 127.0.0.1:1080 -n <node-id>
proxy-rs client http --listen 127.0.0.1:8080 -n <node-id>
proxy-rs client tunnel --listen 127.0.0.1:9000 --remote-host remote_host --remote-port 3000 -n <node-id>
proxy-rs client file <path> -n <node-id> -t home/subdir
proxy-rs client volumes -n <node-id>
```

`-n`/`--node-id` and `--name` become global flags on `client` (inherited by all five
modes, same `global = true` mechanism already proven for `-d`/`--config-dir` on `Cli` in
the last plan) instead of being duplicated per-variant.

## Decisions

| Question | Decision |
|---|---|
| Tunnel's remote target: one flag or two? | **Two**: `--remote-host <HOST>` + `--remote-port <PORT>`. Maps 1:1 onto `ProxyHeaderV1 { host, port }`, no string-splitting, fixes a latent IPv6 parsing bug. |
| `client file`'s path: positional or `-f`/`--file` flag? | **Positional**: `proxy-rs client file <path>`. `PROXY_RS_FILE` env var still works via `#[arg(env = ...)]` on a positional. |
| `client list-volumes`'s subcommand name | **`volumes`** (shortened, now that it's a subcommand rather than a flag) — every reference to `list-volumes` below means `volumes`. |

## Steps, in order

- [x] **Settle the open questions above** — see Decisions table.

- [x] **`main.rs`: nest a nested `ClientMode` subcommand inside `ClientArgs`.**
  - [x] `ClientArgs` keeps `node_id` and `name`, both `#[arg(global = true)]`, and adds
        `#[command(subcommand)] mode: ClientMode`. Drop the `#[command(group(...))]`
        `ArgGroup` entirely — the enum makes it redundant.
  - [x] `enum ClientMode { Socks5(Socks5Args), Http(HttpArgs), Tunnel(TunnelArgs),
        File(FileArgs), Volumes(VolumesArgs) }` — clap kebab-cases `Volumes` to the
        `volumes` subcommand name.
  - [x] `Socks5Args { listen: String }`, `HttpArgs { listen: String }` — both just
        `-l`/`--listen <ADDR>` (env `PROXY_RS_LISTEN`), now genuinely only a bind
        address.
  - [x] `TunnelArgs { listen: String, remote_host: String, remote_port: u16 }` — per
        the open question above; `--remote-host`/`--remote-port` with new env vars
        `PROXY_RS_REMOTE_HOST`/`PROXY_RS_REMOTE_PORT` for symmetry with every other
        flag in this codebase.
  - [x] `FileArgs { file: String, overwrite: bool, target: Option<String> }` — `file`
        as a positional, `-o`/`--overwrite` and `-t`/`--target` unchanged from today.
  - [x] `VolumesArgs {}` — no fields; exists purely so `volumes` is a normal
        `ClientMode` variant like the other four.

- [x] **`client.rs`: remove URL-scheme parsing, thread typed fields through instead.**
  - [x] Delete `get_proxy_addr_and_type` and its `expect`/`panic!` on an unrecognized
        scheme entirely — clap now rejects an invalid/missing mode name itself, with a
        proper usage error and exit code, before any of this code runs. (Today's
        `panic!("Unsupported proxy type: {}", prefix)` is the only panic in the
        client's dispatch path and is inconsistent with the rest of the codebase's
        `anyhow::Result` convention; this refactor removes it rather than just
        hardening it.)
  - [x] `parse_tunnel_addr` shrinks to (at most) validating `remote_port` — or goes
        away entirely if `TunnelArgs.remote_port` is already a typed `u16` field (clap
        validates the parse for us, no manual `.parse::<u16>()` needed).
  - [x] `run_tcp_client`'s shared core (the `TcpListener` accept loop + per-connection
        dispatch to `handle_socks5`/`handle_http`/`handle_tunnel`) stays as one
        function — socks5/http/tunnel still share essentially all of their logic, this
        refactor is CLI-surface-only. Change its signature to take the already-typed
        `ProxyType` + bind address + optional `(remote_host, remote_port)` directly,
        instead of parsing them out of a URL string internally.
  - [x] Three thin call sites (in `main.rs`'s match arms, or three tiny wrapper
        functions in `client.rs` — decide during implementation, whichever reads
        cleaner) construct the right `ProxyType` variant and call the shared core.

- [x] **Wire dispatch in `main()`**: `Command::Client(ClientArgs { node_id, name, mode
      })` matches on `mode` and calls the appropriate `run_*` function, same
      `.or_else(...)` error-context wrapping as today.

- [x] **Update [tests/e2e/socks5_proxy.sh](../tests/e2e/socks5_proxy.sh)**:
  - [x] `"$BIN" client -l "socks5://127.0.0.1:$SOCKS5_PORT" -n "$NODE_ID"` →
        `"$BIN" client socks5 --listen "127.0.0.1:$SOCKS5_PORT" -n "$NODE_ID"`.
  - [x] Run the script directly (docker was reachable in this session last time) to
        confirm the SOCKS5 path still works end to end.

- [x] **Update [CLAUDE.md](../CLAUDE.md)** (third revision of these sections across the
      last two plans — that's expected, the CLI is actively being reshaped):
  - [x] "Commands" section: `client -l socks5://...` / `client -l http://...` /
        `client -l tunnel://...` examples become `client socks5 --listen ...` /
        `client http --listen ...` / `client tunnel --listen ... --remote-host ...
        --remote-port ...`.
  - [x] "Mode selection (main.rs)" section: describe the nested `ClientMode`
        subcommand instead of the `ArgGroup`, and that `-n`/`--name` are now global
        under `client`.
  - [x] Env var list: add `PROXY_RS_REMOTE_HOST`/`PROXY_RS_REMOTE_PORT` if the
        two-flag tunnel design is kept; note `PROXY_RS_LISTEN` is now shared by three
        separate `Socks5Args`/`HttpArgs`/`TunnelArgs` structs rather than one flag
        (safe to reuse the same env var name across all three — only one is ever
        active per invocation).
  - [x] "Client flow" section's tunnel description already documents
        `tunnel://local_host:local_port:remote_host:remote_port`; update to the new
        flag shape.

- [x] **Manual smoke test**, against a scratch `--config-dir`:
  - [x] `client socks5 --listen 127.0.0.1:1080 -n <id>` — listener comes up, proxies
        correctly (reuse the raw-TCP-connect check from the last plan's smoke test).
  - [x] `client http --listen 127.0.0.1:8080 -n <id>` — same.
  - [x] `client tunnel --listen 127.0.0.1:9000 --remote-host <host> --remote-port
        <port> -n <id>` — forwards correctly.
  - [x] `client file <path> -n <id> -t <volume>/<dir>` — sends correctly (reuse
        earlier plan's send/verify).
  - [x] `client volumes -n <id>` — lists correctly.
  - [x] `client` with no mode at all → clean clap error, not a panic or silent no-op.
  - [x] `client socks5 --help` shows *only* `--listen`, `-n`, `--name` — confirms
        `-o`/`-t`/`--volume` no longer leak into unrelated modes' `--help` (this was
        the actual complaint that started this plan).
  - [x] `client -n <id> socks5 --listen ...` and `client socks5 -n <id> --listen ...`
        (global flag before vs. after the mode subcommand) both work.
