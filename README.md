# proxy-rs

**Reach your machines by public key, not by IP address.**

`proxy-rs` is a peer-to-peer proxy, tunnel and file-transfer tool built on [iroh](https://github.com/n0-computer/iroh). Run the server on a box behind NAT, CGNAT, a firewall or a flaky home connection. Clients dial it by its **node ID** and iroh does the rest: hole-punching, relay fallback and end-to-end encrypted QUIC.

No port forwarding, no dynamic DNS and no VPN to set up. One binary covers both ends.

```text
  your laptop                                              your Pi at home
┌──────────────────┐                                   ┌───────────────────┐
│ browser ─► SOCKS5│    iroh / QUIC (E2E encrypted)    │  proxy-rs server  │ ─► LAN / internet
│ curl ─► HTTP     │ ════════════════════════════════► │                   │
│ ssh ─► tunnel    │   hole-punched, or via relay      │  -v media:/media  │ ─► /media
│ rsync / file     │                                   │                   │
└──────────────────┘                                   └───────────────────┘
      proxy-rs client  ──── dials by node ID ────►  3f9a…c1e2
```

## Features

- **SOCKS5 proxy.** Point a browser or `curl --socks5-hostname` at it and browse from the server's network.
- **HTTP proxy.** Supports both `CONNECT` tunnelling and plain HTTP forwarding.
- **Port tunnels (`ssh -L` style).** Forward a local port to a fixed `host:port` that the server can reach.
- **File send.** Push a file into a named server volume, with a progress bar.
- **rsync over iroh.** Use `proxy-rs` as rsync's `-e` transport in place of ssh.
- **Stable identity.** The server keeps its key across restarts, so its node ID never changes.
- **Saved servers.** Clients remember node IDs under friendly names (`Brave Otter`, `Sleepy Heron`, …) and show a picker when you don't pass one.
- **Runs on a Raspberry Pi.** A Dockerfile is included that cross-compiles for armv7.

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

### 2. Start a server

```bash
proxy-rs server
```

```text
proxy-rs v0.1.0
Mode: server
Volumes: (none exposed — file transfers will be rejected)
Iroh node listening [3f9a0b…c1e2]
```

Copy that node ID and give it to your clients. It is also written to `~/.proxy-rs/server-key.pub`.

### 3. Connect a client

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

```bash
proxy-rs server                                   # proxy only
proxy-rs server -v media:/srv/media -v home:$HOME/in  # also expose two volumes
```

| Flag | Description |
|---|---|
| `-v, --volume name:path` | Expose a directory to clients under `name`. Repeatable. The path must exist. |

The server always accepts proxy, tunnel and ping connections. Volumes are only needed for `file` and `sync-rsh`.

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

Every connection to `--listen` is forwarded to `--remote-host:--remote-port`, which is resolved and dialled **from the server**.

```bash
# Reach the Home Assistant instance on the server's LAN
proxy-rs client tunnel --listen 127.0.0.1:8123 \
  --remote-host 192.168.1.50 --remote-port 8123 -n <node-id>

# SSH into the server box itself, with no open ports
proxy-rs client tunnel --listen 127.0.0.1:2222 \
  --remote-host 127.0.0.1 --remote-port 22 -n <node-id>
ssh -p 2222 user@127.0.0.1
```

#### `volumes`: see what the server exposes

```bash
$ proxy-rs client volumes -n <node-id>
media:/srv/media
home:/home/pi/in
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

You can set almost every flag with an environment variable: `PROXY_RS_LISTEN`, `PROXY_RS_REMOTE_HOST`, `PROXY_RS_REMOTE_PORT`, `PROXY_RS_FILE`, `PROXY_RS_TARGET` and `PROXY_RS_OVERWRITE`. A flag on the command line always wins.

The default log level is `error`. For more detail, use `RUST_LOG`:

```bash
RUST_LOG=info proxy-rs server
RUST_LOG=proxy_rs=debug,iroh=info proxy-rs client socks5 -l 127.0.0.1:1080 -n <id>
```

## State on disk

The state directory is `~/.proxy-rs/` by default, or whatever `--config-dir` points to.

| File | Side | Contents |
|---|---|---|
| `server-key` | server | Hex-encoded secret key. **This is the server's identity. Keep it private and back it up.** |
| `server-key.pub` | server | The server's node ID, rewritten on every start. |
| `node-ids.yaml` | client | Saved servers, stored as a list of `{ name, key }`. |

If you delete `server-key`, the server gets a new node ID and every client has to be updated.

## ⚠️ Security model

Read this before exposing a server.

- **The node ID is the only credential.** The server does **not** authenticate or allowlist clients. Anyone who knows the node ID can:
  - open TCP connections to **any host and port the server can reach**, including `127.0.0.1` on the server and its whole LAN;
  - list the server's volumes, and write files into them with `file` or `sync-rsh`.
- Treat the node ID like a password. Share it only with people you would give a shell account.
- Volume writes are confined to the volume root. `..` segments are rejected, and symlink escapes are caught by canonicalising the path before it is checked.
- rsync flags are passed through to `rsync --server` as they are, **including `--delete`**. A client can delete files inside a volume.
- Transport encryption and server authenticity come from iroh. Your client only talks to the holder of that node's secret key.

Client authentication is on the roadmap, and contributions are welcome.

## Running on a Raspberry Pi

`Dockerfile_arm32` cross-compiles for `armv7-unknown-linux-gnueabihf` on your host, with no emulation, and outputs just the binary:

```bash
docker build -f Dockerfile_arm32 --output type=local,dest=out .
scp out/proxy-rs pi@raspberrypi:~/
ssh pi@raspberrypi './proxy-rs server -v media:/media'
```

To keep it running, wrap it in a systemd unit. It holds the same node ID across reboots.

## How it works

Each feature is its own iroh **ALPN protocol**, so one endpoint serves all of them over multiplexed QUIC:

| ALPN | Purpose |
|---|---|
| `proxy-rs/ping/1` | Pre-flight check that runs before every client operation |
| `proxy-rs/tcp-proxy/1` | Sends a `{ host, port }` header, then pipes raw bytes both ways |
| `proxy-rs/file/1` | Header → ack → file bytes → final ack |
| `proxy-rs/list-volumes/1` | Returns the server's volume map |
| `proxy-rs/rsync/1` | Carries an `rsync --server` session over a QUIC stream |

SOCKS5, HTTP and tunnel mode all use the **same** server protocol. They differ only in how the client works out the target `(host, port)`: a SOCKS5 handshake, an HTTP request line, or a fixed value.

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
cargo test -- --ignored     # end-to-end SOCKS5 smoke test (needs the devcontainer)
```

The e2e test ([`tests/e2e/socks5_proxy.sh`](tests/e2e/socks5_proxy.sh)) starts a real server and a SOCKS5 client in a throwaway `HOME`. It then curls a test HTTP server from `.devcontainer/docker-compose.yaml` through the proxy. Open the repo in the devcontainer to get that network.
