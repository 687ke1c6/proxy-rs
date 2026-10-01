# proxy-rs

**A client/server CLI that carries TCP proxying, port tunnels and file transfer over [iroh](https://github.com/n0-computer/iroh) streams.**

`proxy-rs` is one binary with two modes:

- **`server`** runs an iroh endpoint. It accepts only the protocols you enable with flags, each on its own ALPN, and does the work on the server's side: dialling allowed TCP targets and writing files into volume-mapped directories.
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

We'll start by configuring a server that accepts iroh connections and tunnels those connections to a target. In this case we'll tunnel to sshd running on the same host:

### 1. Start a server

On the target ssh server:

```bash
proxy-rs server --allow-any --target '*'
```

```text
proxy-rs v0.1.1
Mode: server
Features:
  tcp proxy -> *:*
WARNING: open proxy enabled (--target '*'): any client with this node id can reach anything this host can, including localhost and the LAN
WARNING: --allow-any: any client that knows this server's node id can connect
Iroh node listening [2ee0c38203f21eada4dcc8d744053f5a0952cb7f4833ed16f387f588e0f39977]
```

Note the two `WARNINGS`: `--allow-any` lets any client connect, and `--target '*'` lets it reach any host and port. That's the simplest setup, not a safe one.

The ID in brackets is the server's iroh **node ID**.

### 2. Connect a client

Locally open a port, and connect to our proxy-rs server by its iroh node id.

```bash
proxy-rs client tunnel --listen 127.0.0.1:2222 --remote-host localhost --remote-port 22 \
  --node-id 2ee0c38203f21eada4dcc8d744053f5a0952cb7f4833ed16f387f588e0f39977
```

```text
proxy-rs v0.1.1
Mode: client tunnel
Listening on: 127.0.0.1:2222
Forwarding to: localhost:22
Connected to "Plucky Narwhal" [2ee0c38203f21eada4dcc8d744053f5a0952cb7f4833ed16f387f588e0f39977]
```

`--remote-host` is resolved on the server, so `localhost` means the server itself.

Now ssh to the server through the tunnel:

```bash
ssh -p 2222 user@127.0.0.1
```

Putting it together:

```text
           your machine                                  server machine
┌─────────────┐   ┌─────────────┐   .~~~~~~.   ┌─────────────┐   ┌─────────────┐
│ ssh         │──►│ proxy-rs    │──(  iroh  )─►│ proxy-rs    │──►│ sshd        │
│ -p 2222     │   │ client      │   '~~~~~~'   │ server      │   │ port 22     │
│             │   │ tunnel      │              │             │   │             │
└─────────────┘   └─────────────┘              └─────────────┘   └─────────────┘
```


## Concepts

### Node IDs

The server and each client have their own iroh key pair, created on first run and reused after that, so their node IDs never change. The keys are stored in `~/.proxy-rs/` (or `--config-dir`): `server-key` and `server-key.pub` on the server, `client-key` and `client-key.pub` on the client. `proxy-rs client whoami` prints the client's node ID, which a server needs once you replace `--allow-any` with an allowlist (`--allow`).

### Saved servers

Clients remember the servers they connect to, in `~/.proxy-rs/nodes.yaml`. The first time you use a `--node-id`, the client saves it under a generated name. That's where `"Plucky Narwhal"` in the Quick start came from:

```yaml
node_entries:
- name: Plucky Narwhal
  key: 2ee0c38203f21eada4dcc8d744053f5a0952cb7f4833ed16f387f588e0f39977
```

After that, you can refer to the server by name:

```bash
proxy-rs client tunnel --listen 127.0.0.1:2222 --remote-host localhost --remote-port 22 \
  --name "Plucky Narwhal"
```

You will be prompted to supply a node-id/name where none were found/provided.

## Usage

```text
proxy-rs <server|client> ...
```

### Server

Whitelist clients by specifying client node id's in `~/proxy-rs/authorized-clients`. Anything after the id on a line is a label (e.g. `3f1eb531...58d Dan Bowers PC`), shown in the server's `Client connected:` output. Or explicitly on the command line `proxy-rs server --allow <client-node-id>`. Or allow any client with `--allow-any` (unsafe).

The server refuses to start if no client is allowed and `--allow-any` isn't set.

The `-t --target` flag (repeatable) enables the server to make outbound tcp connections to given targets.

```bash
proxy-rs server -t '*'                                      # outbound connections to any target
proxy-rs server -t localhost:22                             # will only allow connecting to local sshd
proxy-rs server -t '*.lan:*' -t '10.0.0.5:8000-8100'        # proxy into part of the LAN
proxy-rs server -f -r -v media:/srv/media -v home:$HOME/in  # file send + rsync to mapped volumes
```

All server arguments:

| Flag | Env var | Description |
|---|---|---|
| `-t, --target <PATTERN>` | `PROXY_RS_ALLOW_TARGET` (comma-separated) | Allow TCP proxying (SOCKS5, HTTP and tunnel clients) to targets matching `PATTERN`. Repeatable. |
| `-f, --file` | `PROXY_RS_SERVE_FILE` | Allow clients to send files into volumes. Needs at least one `-v`. |
| `-r, --rsync` | `PROXY_RS_RSYNC` | Allow rsync pushes into volumes. Needs at least one `-v`, and `rsync` on the server. |
| `-v, --volume name:path` | — | Declare a directory clients can write into, under `name`. Repeatable. The path must exist. |
| `--allow <NODE_ID>` | `PROXY_RS_ALLOW` (comma-separated) | Allow a client to connect. Repeatable. Merged with the `authorized-clients` file. |
| `--allow-any` | `PROXY_RS_ALLOW_ANY` | Let any client that knows the server's node ID connect. Can't be combined with `--allow`. |

### Client

The client always runs with a subcommand that picks what it does, `proxy-rs client <subcommand> [options]`:

| Subcommand | What it does |
|---|---|
| [`socks5`](#socks5-local-socks5-proxy) | Run a local SOCKS5 proxy that connects out through the server |
| [`http`](#http-local-http-proxy) | Run a local HTTP proxy that connects out through the server |
| [`tunnel`](#tunnel-forward-one-port-like-ssh--l) | Forward one local port to a fixed target reached from the server, like `ssh -L` |
| [`whoami`](#whoami-print-this-clients-node-id) | Print this client's node ID |
| [`volumes`](#volumes-see-what-the-server-exposes) | List the volumes the server exposes |
| [`file`](#file-send-a-file) | Send a file into a server volume |
| [`sync-rsh`](#sync-rsh-rsync-over-iroh) | Transport for `rsync -e`, to push directories into a server volume |

Every subcommand except `whoami` connects to a server, chosen by its node ID `-n <server-node-id>` or its saved name `--name <name>`. Generally the name is more convenient.

---

#### `socks5`: local SOCKS5 proxy

```bash
proxy-rs client socks5 --listen 127.0.0.1:1080 --name "Brave Otter"
```

---

#### `http`: local HTTP proxy

```bash
proxy-rs client http --listen 127.0.0.1:8080 --name "Brave Otter"
# export https_proxy=http://127.0.0.1:8080 http_proxy=http://127.0.0.1:8080
```

---

#### `tunnel`: forward one port (like `ssh -L`)

Every connection to `--listen` is forwarded to `--remote-host:--remote-port`, which is resolved and dialled **from the server**. The server must allow that target with `-t` (for the examples below, `-t 192.168.1.50:8123` and `-t 127.0.0.1:22`).

```bash
# Reach the Home Assistant instance on the server's LAN
proxy-rs client tunnel --listen 127.0.0.1:8123 \
  --remote-host 192.168.1.50 --remote-port 8123 --name "Brave Otter"

# SSH into the server box itself, with no open ports
proxy-rs client tunnel --listen 127.0.0.1:2222 \
  --remote-host 127.0.0.1 --remote-port 22 --name "Brave Otter"
ssh -p 2222 user@127.0.0.1
```

---

#### `whoami`: print this client's node ID

```bash
$ proxy-rs client whoami
7c41d2…9e0b
```

It prints only the ID, so you can use it in scripts, e.g. `ssh server "echo $(proxy-rs client whoami) laptop >> ~/.proxy-rs/authorized-clients"`.

---

#### `volumes`: see what the server exposes

```bash
$ proxy-rs client volumes --name "Brave Otter"
media:/srv/media
home:/home/user/in
```

---

#### `file`: send a file

```bash
proxy-rs client file ./holiday.mkv -t media/videos/2026 --name "Brave Otter"
```

| Flag | Description |
|---|---|
| `-t, --target volume[/dir]` | Destination. Subdirectories are created as needed. You can omit it if the server has exactly one volume. |
| `-o, --overwrite` | Replace the file if it already exists. By default the transfer is refused. |

---

#### `sync-rsh`: rsync over iroh

You don't run `sync-rsh` directly. You give it to rsync's `-e` flag in place of `ssh`:

```bash
rsync -av -e "proxy-rs client sync-rsh --name 'Brave Otter'" \
  ./photos/ "x:media/photos/"
```

- The host before the `:` (`x`) is a placeholder and is ignored, but required.
- The path after the `:` must start with an exposed volume name.
- `-n` or `--name` is **required** here. Stdin belongs to rsync.
- Only push is supported. `rsync` must be installed on **both** ends.

---

### Global options

| Flag | Env var | Description |
|---|---|---|
| `-d, --config-dir <DIR>` | `PROXY_RS_CONFIG_DIR` | State directory (default `~/.proxy-rs`). |

You can set almost every flag with an environment variable: `PROXY_RS_LISTEN`, `PROXY_RS_REMOTE_HOST`, `PROXY_RS_REMOTE_PORT`, `PROXY_RS_FILE`, `PROXY_RS_TARGET` and `PROXY_RS_OVERWRITE` on the client, and `PROXY_RS_ALLOW_TARGET`, `PROXY_RS_SERVE_FILE`, `PROXY_RS_RSYNC`, `PROXY_RS_ALLOW` and `PROXY_RS_ALLOW_ANY` on the server. A flag on the command line always wins.

The default log level is `error`. For more detail, use `RUST_LOG`:

```bash
RUST_LOG=info proxy-rs server
RUST_LOG=proxy_rs=debug,iroh=info proxy-rs client socks5 -l 127.0.0.1:1080 -n <id>
```

## ⚠️ Security model

Read this before exposing a server.

- **Clients are authenticated by their node ID.** iroh proves every client holds the secret key for the node ID it presents, and the server only admits node IDs on its allowlist (`--allow` / `authorized-clients`). The server's node ID alone is not enough to connect.
- **Every allowed client can use every enabled feature.**:
  - with `-t`, open TCP connections to any target the patterns allow. With `-t '*'` that is **any host and port the server can reach**, including `127.0.0.1` on the server and its whole LAN;
  - with `-f` or `-r`, list the server's volumes and write files into them.
- Enable only what you need, and keep `-t` patterns as narrow as you can.
- `client-key` is a credential: anyone who copies it can connect as that client. To revoke a client, remove it from `authorized-clients` (or drop its `--allow` and restart).
- Avoid using `--allow-any`, the server's node ID becomes the only credential. Treat it like a password.
- Volume writes are confined to the volume root. `..` segments are rejected, and symlink escapes are caught by canonicalising the path before it is checked.
- rsync flags are passed through to `rsync --server` as they are, **including `--delete`**. A client can delete files inside a volume.
- Transport encryption and server authenticity come from iroh. Your client only talks to the holder of that node's secret key.

## Cross-compiling for arm32

`Dockerfile_arm32` cross-compiles for `armv7-unknown-linux-gnueabihf` on your host, with no emulation, and outputs just the binary:

```bash
docker build -f Dockerfile_arm32 --output type=local,dest=target/armv7 . 
```

## Development

```bash
cargo build
cargo clippy
cargo test                  # unit tests
cargo test -- --ignored     # end-to-end SOCKS5 test (needs the devcontainer)
```

The e2e test ([`tests/e2e/socks5_proxy.sh`](tests/e2e/socks5_proxy.sh)) starts a real server and a SOCKS5 client in a throwaway `HOME`. It also starts a local `ncat` HTTP listener on port 4001 as the target. The server only allows `-t localhost:4001` and the test client's node ID (`--allow`). The test curls that listener through the proxy, then checks that a target outside the allowlist (`localhost:4002`) is refused with SOCKS5 reply `0x02`, and that a client with a different key is refused at startup. It needs `ncat` installed (the devcontainer has it).
