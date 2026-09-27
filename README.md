# proxy-rs

**A client/server CLI that carries TCP proxying, port tunnels and file transfer over [iroh](https://github.com/n0-computer/iroh) streams.**

`proxy-rs` is one binary with two modes:

- **`server`** runs an iroh endpoint. It accepts only the protocols you enable with flags, each on its own ALPN, and does the work on the server's side: dialling allowed TCP targets and writing files into exposed directories.
- **`client`** opens iroh streams to that server and connects them to something local: a SOCKS5 or HTTP proxy listener, a forwarded port, a file, or rsync.

Clients address the server by its iroh **node ID** rather than an IP address. iroh handles NAT traversal, relay fallback and end-to-end encryption, so the server needs no open ports, port forwarding or dynamic DNS.

```text
  your laptop                                                 remote machine
┌──────────────────┐                                   ┌───────────────────┐
│ browser ─► SOCKS5│    iroh / QUIC (E2E encrypted)    │  proxy-rs server  │ ─► LAN / internet
│ curl ─► HTTP     │ ════════════════════════════════► │                   │
│ ssh ─► tunnel    │   hole-punched, or via relay      │  -t … -f -v media │ ─► /media
│ rsync / file     │                                   │                   │
└──────────────────┘                                   └───────────────────┘
      proxy-rs client  ──── dials by node ID ────►  3f9a…c1e2
```

## Features

- **SOCKS5 proxy.** Point a browser or `curl --socks5-hostname` at it and browse from the server's network.
- **HTTP proxy.** Supports both `CONNECT` tunnelling and plain HTTP forwarding.
- **Port tunnels (`ssh -L` style).** Forward a local port to a fixed `host:port` that the server can reach.
- **Opt-in server features.** Nothing is exposed unless you enable it, and TCP targets are limited to an allowlist (`-t localhost:22`, `-t '*.lan:*'`, …).
- **Client allowlist.** Each client has its own stable node ID. The server only admits the ones you list, and you can add or revoke clients without a restart.
- **File send.** Push a file into a named server volume, with a progress bar.
- **rsync over iroh.** Use `proxy-rs` as rsync's `-e` transport in place of ssh.
- **Stable identity.** The server keeps its key across restarts, so its node ID never changes.
- **Saved servers.** Clients remember node IDs under friendly names (`Brave Otter`, `Sleepy Heron`, …) and show a picker when you don't pass one.
- **arm32 builds.** A Dockerfile is included that cross-compiles for armv7.

## Quick start

### 1. Build

```bash
cargo build --release
# binary: target/release/proxy-rs
```

Or install it onto your `PATH`:

```bash
cargo install --path .
```

### 2. Get the client's node ID

On the machine you'll connect **from**:

```bash
proxy-rs client whoami
# 7c41d2…9e0b
```

### 3. Start a server

On the machine you'll connect **to**, allowing that client:

```bash
proxy-rs server -t '*' --allow 7c41d2…9e0b    # proxy to any target; see "Server" below to narrow this
```

```text
proxy-rs v0.1.0
Mode: server
Features:
  tcp proxy -> *:*
WARNING: open proxy enabled (-t '*'): any client with this node id can reach anything this host can, including localhost and the LAN
Clients: 1 via --allow, 0 in /home/you/.proxy-rs/authorized-clients (re-read on every connection)
Iroh node listening [3f9a0b…c1e2]
```

Copy the server's node ID and give it to your clients. It is also written to `~/.proxy-rs/server-key.pub`.

### 4. Connect a client

```bash
proxy-rs client socks5 --listen 127.0.0.1:1080 -n 3f9a0b…c1e2
```

```bash
curl --socks5-hostname 127.0.0.1:1080 https://ifconfig.me   # prints the server's public IP
```

That's it. You are now browsing from the server's network.

## Usage

```text
proxy-rs [--config-dir DIR] <server|client> ...
```

### Server

The server exposes **nothing but a health check** unless you enable features with flags, and only to the clients you allow (see [Client allowlist](#client-allowlist)):

```bash
proxy-rs server --allow <client-id> -t localhost:22                          # ssh tunnel only
proxy-rs server --allow <client-id> -t '*'                                   # open SOCKS5/HTTP proxy
proxy-rs server --allow <client-id> -t '*.lan:*' -t '10.0.0.5:8000-8100'     # proxy into part of the LAN
proxy-rs server --allow <client-id> -f -r -v media:/srv/media -v home:$HOME/in  # file send + rsync into two volumes
```

| Flag | Env var | Description |
|---|---|---|
| `-t, --tunnel <PATTERN>` | `PROXY_RS_TUNNEL` (comma-separated) | Allow TCP proxying (SOCKS5, HTTP and tunnel clients) to targets matching `PATTERN`. Repeatable. |
| `-f, --file` | `PROXY_RS_SERVE_FILE` | Allow clients to send files into volumes. Needs at least one `-v`. |
| `-r, --rsync` | `PROXY_RS_RSYNC` | Allow rsync pushes into volumes. Needs at least one `-v`, and `rsync` on the server. |
| `-v, --volume name:path` | — | Declare a directory clients can write into, under `name`. Repeatable. The path must exist. |
| `--allow <NODE_ID>` | `PROXY_RS_ALLOW` (comma-separated) | Allow a client to connect. Repeatable. Merged with the `authorized-clients` file. |
| `--allow-any` | `PROXY_RS_ALLOW_ANY` | Let any client that knows the server's node ID connect. Can't be combined with `--allow`. |

Listing volumes (`client volumes`) is available whenever `-f` or `-r` is on. The server refuses to start if no feature is enabled, if `-v` is given without `-f`/`-r`, or if `-f`/`-r` is given without `-v`. It prints exactly what it exposes on startup.

#### `-t` patterns

A pattern is `host:port`, or a bare `*` for anything:

| Pattern | Allows |
|---|---|
| `*` or `*:*` | Any host, any port. **This is an open proxy**, and the server prints a warning. |
| `localhost:22` | Exactly that host and port. |
| `*:443` | Any host, port 443. |
| `*.lan:*` | Any subdomain of `lan` (`nas.lan`, `a.b.lan`, but not `lan` itself), any port. |
| `db:8000-8100` | Ports 8000 to 8100 inclusive. |
| `[::1]:22` | IPv6 addresses must be in brackets. |

Hosts are case-insensitive. `*.*` is rejected: use `*`.

Patterns match the **hostname the client sends, before DNS**. `-t localhost:22` does not allow `127.0.0.1:22`, and a SOCKS5 client that resolves names locally (`curl --socks5` rather than `--socks5-hostname`) sends an IP, which only matches an IP pattern. `*.example.com` is only as trustworthy as whoever controls that domain's DNS.

A client whose target isn't allowed gets a clear refusal: SOCKS5 reply `0x02` ("not allowed by ruleset"), HTTP `403 Forbidden`, or, for `tunnel`, a closed connection and an error in the client's output. If the target is allowed but unreachable, SOCKS5 gets `0x05` and HTTP gets `502 Bad Gateway`.

#### Client allowlist

Every client has its own key (`~/.proxy-rs/client-key`, created on first use), so its node ID stays the same across runs. `proxy-rs client whoami` prints it.

The server admits a client if its node ID is given with `--allow`, or listed in `~/.proxy-rs/authorized-clients` (or under `--config-dir`). The server creates that file on startup if it's missing, containing only explanatory comments, so it allows no one until you add IDs. It works like ssh's `authorized_keys`:

```text
# one node id per line; anything after it is a comment
7c41d2…9e0b  laptop
a90f3e…41c7  phone
```

- The file is **re-read on every connection**, so you can add or revoke a client by editing it, with no restart. Revoking stops new connections; connections already open are not cut.
- If the file can't be read or parsed, the server logs an error and admits only the `--allow` clients.
- The server refuses to start if no client is allowed at all, unless you pass `--allow-any`, which prints a warning.
- The check applies to every protocol, including the health check. A refused client fails at startup with its node ID and what to ask the operator for:

```text
Error: server refused this client: node id 7c41d2…9e0b is not in its allowlist; ask the server operator to add it with --allow or to their authorized-clients file
```

### Client

Every client mode needs to know which server to use:

| Flag | Env var | Description |
|---|---|---|
| `-n, --node-id <ID>` | `PROXY_RS_NODE_ID` | Server node ID. It is saved under an auto-generated name the first time you use it. |
| `--name <NAME>` | `PROXY_RS_NAME` | Pick a previously saved server by name. |

If you pass neither, you get an interactive picker with your saved servers and a `<new>` entry for adding one:

```text
? Select server node ID
> Brave Otter [3f9a0b1c2d3e4f50...]
  Sleepy Heron [9c8b7a6f5e4d3c2b...]
  <new>
```

#### `socks5`: local SOCKS5 proxy

```bash
proxy-rs client socks5 --listen 127.0.0.1:1080 --name "Brave Otter"
```

#### `http`: local HTTP proxy

```bash
proxy-rs client http --listen 127.0.0.1:8080 -n <node-id>
export https_proxy=http://127.0.0.1:8080 http_proxy=http://127.0.0.1:8080
```

#### `tunnel`: forward one port (like `ssh -L`)

Every connection to `--listen` is forwarded to `--remote-host:--remote-port`, which is resolved and dialled **from the server**. The server must allow that target with `-t` (for the examples below, `-t 192.168.1.50:8123` and `-t 127.0.0.1:22`).

```bash
# Reach the Home Assistant instance on the server's LAN
proxy-rs client tunnel --listen 127.0.0.1:8123 \
  --remote-host 192.168.1.50 --remote-port 8123 -n <node-id>

# SSH into the server box itself, with no open ports
proxy-rs client tunnel --listen 127.0.0.1:2222 \
  --remote-host 127.0.0.1 --remote-port 22 -n <node-id>
ssh -p 2222 user@127.0.0.1
```

#### `whoami`: print this client's node ID

```bash
$ proxy-rs client whoami
7c41d2…9e0b
```

It prints only the ID, so you can use it in scripts, e.g. `ssh server "echo $(proxy-rs client whoami) laptop >> ~/.proxy-rs/authorized-clients"`.

#### `volumes`: see what the server exposes

```bash
$ proxy-rs client volumes -n <node-id>
media:/srv/media
home:/home/user/in
```

#### `file`: send a file

```bash
proxy-rs client file ./holiday.mkv -t media/videos/2026 -n <node-id>
```

| Flag | Description |
|---|---|
| `-t, --target volume[/dir]` | Destination. Subdirectories are created as needed. You can omit it if the server has exactly one volume. |
| `-o, --overwrite` | Replace the file if it already exists. By default the transfer is refused. |

#### `sync-rsh`: rsync over iroh

You don't run `sync-rsh` directly. You give it to rsync's `-e` flag in place of `ssh`:

```bash
rsync -av -e "'$(which proxy-rs)' client sync-rsh -n <node-id>" \
  ./photos/ "x:media/photos/"
```

- The host before the `:` (`x`) is a placeholder and is ignored. The server is chosen by `-n` or `--name`.
- The path after the `:` must start with a volume name.
- `-n` or `--name` is **required** here, because stdin belongs to rsync and the picker can't be shown.
- Only push is supported. `rsync` must be installed on **both** ends.

### Global options

| Flag | Env var | Description |
|---|---|---|
| `-d, --config-dir <DIR>` | `PROXY_RS_CONFIG_DIR` | State directory (default `~/.proxy-rs`). |

You can set almost every flag with an environment variable: `PROXY_RS_LISTEN`, `PROXY_RS_REMOTE_HOST`, `PROXY_RS_REMOTE_PORT`, `PROXY_RS_FILE`, `PROXY_RS_TARGET` and `PROXY_RS_OVERWRITE` on the client, and `PROXY_RS_TUNNEL`, `PROXY_RS_SERVE_FILE`, `PROXY_RS_RSYNC`, `PROXY_RS_ALLOW` and `PROXY_RS_ALLOW_ANY` on the server. A flag on the command line always wins.

The default log level is `error`. For more detail, use `RUST_LOG`:

```bash
RUST_LOG=info proxy-rs server
RUST_LOG=proxy_rs=debug,iroh=info proxy-rs client socks5 -l 127.0.0.1:1080 -n <id>
```

## State on disk

The state directory is `~/.proxy-rs/` by default, or whatever `--config-dir` points to.

| File | Side | Contents |
|---|---|---|
| `server-key` | server | Hex-encoded secret key, created with mode `600`. **This is the server's identity. Keep it private and back it up.** |
| `server-key.pub` | server | The server's node ID, rewritten on every start. |
| `authorized-clients` | server | Client node IDs allowed to connect, one per line. Created with only comments on first start (unless `--allow-any`); never overwritten. |
| `client-key` | client | Hex-encoded secret key, created with mode `600`. The client's identity for servers' allowlists. |
| `client-key.pub` | client | The client's node ID (what `client whoami` prints), rewritten on every run. |
| `nodes.yaml` | client | Saved servers, stored as a list of `{ name, key }`. |

If you delete `server-key`, the server gets a new node ID and every client has to be updated. If you delete `client-key`, the client gets a new node ID and has to be allowed again on every server.

## ⚠️ Security model

Read this before exposing a server.

- **Clients are authenticated by their node ID.** iroh proves every client holds the secret key for the node ID it presents, and the server only admits node IDs on its allowlist (`--allow` / `authorized-clients`). The server's node ID alone is not enough to connect.
- **Every allowed client can use every enabled feature.** There are no per-client permissions yet:
  - with `-t`, open TCP connections to any target the patterns allow. With `-t '*'` that is **any host and port the server can reach**, including `127.0.0.1` on the server and its whole LAN;
  - with `-f` or `-r`, list the server's volumes and write files into them.
- Enable only what you need, and keep `-t` patterns as narrow as you can.
- `client-key` is a credential: anyone who copies it can connect as that client. To revoke a client, remove it from `authorized-clients` (or drop its `--allow` and restart).
- With `--allow-any`, the server's node ID becomes the only credential. Treat it like a password.
- Volume writes are confined to the volume root. `..` segments are rejected, and symlink escapes are caught by canonicalising the path before it is checked.
- rsync flags are passed through to `rsync --server` as they are, **including `--delete`**. A client can delete files inside a volume.
- Transport encryption and server authenticity come from iroh. Your client only talks to the holder of that node's secret key.

Per-client feature grants are on the roadmap, and contributions are welcome.

## Cross-compiling for arm32

`Dockerfile_arm32` cross-compiles for `armv7-unknown-linux-gnueabihf` on your host, with no emulation, and outputs just the binary:

```bash
docker build -f Dockerfile_arm32 --output type=local,dest=out .
scp out/proxy-rs user@host:~/
```

To keep a server running, wrap it in a systemd unit. It holds the same node ID across restarts.

## How it works

Each feature is its own iroh **ALPN protocol**, so one endpoint serves all of them over multiplexed QUIC. The server only registers the ALPNs for features you enabled; any other is refused during the QUIC handshake. `ping` is always on. Every registered ALPN is wrapped in iroh's `AccessLimit`, which closes connections from clients not on the allowlist before the protocol sees them:

| ALPN | Purpose |
|---|---|
| `proxy-rs/ping/1` | Pre-flight check that runs before every client operation |
| `proxy-rs/tcp/2` | Sends a `{ host, port }` header; the server checks it against `-t`, replies allowed / not allowed / connect failed, then pipes raw bytes both ways |
| `proxy-rs/file/1` | Header → ack → file bytes → final ack |
| `proxy-rs/list-volumes/1` | Returns the server's volume map |
| `proxy-rs/rsync/1` | Carries an `rsync --server` session over a QUIC stream |

SOCKS5, HTTP and tunnel mode all use the **same** server protocol. They differ only in how the client works out the target `(host, port)`: a SOCKS5 handshake, an HTTP request line, or a fixed value. That's why the server gates them with one target allowlist (`-t`) rather than per-mode flags.

Messages use a small custom binary codec (`StreamCodec`, in [`src/protocols/codec.rs`](src/protocols/codec.rs)). Its struct impls are generated by the `#[derive(StreamCodec)]` proc macro in [`proxy-rs-derive/`](proxy-rs-derive/).

```text
src/
├── main.rs, cli.rs        # clap CLI and mode dispatch
├── server.rs              # iroh endpoint and protocol router
├── client/                # client modes, saved-server picker
├── socks5.rs, http.rs     # local proxy handshakes
└── protocols/             # one directory per ALPN protocol
```

## Development

```bash
cargo build
cargo clippy
cargo test                  # unit tests
cargo test -- --ignored     # end-to-end SOCKS5 test (needs the devcontainer)
```

The e2e test ([`tests/e2e/socks5_proxy.sh`](tests/e2e/socks5_proxy.sh)) starts a real server and a SOCKS5 client in a throwaway `HOME`. It also starts a local `ncat` HTTP listener on port 4001 as the target. The server only allows `-t localhost:4001` and the test client's node ID (`--allow`). The test curls that listener through the proxy, then checks that a target outside the allowlist (`localhost:4002`) is refused with SOCKS5 reply `0x02`, and that a client with a different key is refused at startup. It needs `ncat` installed (the devcontainer has it).
