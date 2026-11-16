# proxy-rs

**A client/server CLI that carries TCP proxying, port tunnels and rsync over [iroh](https://github.com/n0-computer/iroh) streams.**

`proxy-rs` is one binary:

- **A server** (`proxy-rs socks5`, `http`, `tunnel` or `rsync`) runs an iroh endpoint in one mode. The mode decides what every client connection does: speak SOCKS5 or HTTP proxy and dial the requested target, connect to one fixed `host:port`, or run an rsync push into a volume.
- **A client** (`proxy-rs --listen <addr>`) binds a local port and forwards every connection to a server, byte for byte. It doesn't know or care which mode the server runs, so the client command is the same for all of them.

Clients address the server by its iroh **node ID** rather than an IP address. iroh handles NAT traversal, relay fallback and end-to-end encryption, so the server needs no open ports, port forwarding or dynamic DNS.

```text
  your laptop                                                 remote machine
┌──────────────────┐                                   ┌───────────────────┐
│ browser, curl,   │    iroh / QUIC (E2E encrypted)    │  proxy-rs socks5  │ ─► LAN / internet
│ ssh, ...         │ ════════════════════════════════► │         | http    │
│   ─► 127.0.0.1   │   hole-punched, or via relay      │         | tunnel  │ ─► one host:port
│ rsync ─► --pipe  │                                   │         | rsync   │ ─► /media
└──────────────────┘                                   └───────────────────┘
  proxy-rs --listen  ──── dials by node ID ────►  3f9a…c1e2
```

## Features

- **SOCKS5 proxy server.** Point a browser or `curl --socks5-hostname` at the client's port and browse from the server's network.
- **HTTP proxy server.** Supports both `CONNECT` tunnelling and plain HTTP forwarding.
- **Tunnel server.** Every client connection goes to one fixed `host:port` that the server can reach (sshd, a web UI, a database).
- **rsync server.** Use `proxy-rs --pipe` as rsync's `-e` transport in place of ssh, to push into named volumes.
- **The server owns the policy.** The server's mode and flags define everything a client can do; SOCKS5/HTTP targets are limited to an allowlist (`-t localhost:22`, `-t '*.lan:*'`, …).
- **Client allowlist.** Each client has its own stable node ID. The server only admits the ones you list, and you can add or revoke clients without a restart.
- **One config file per node.** Key, saved servers and allowed clients live in one TOML file; a second server on the same host is just another `--config` file.
- **Saved servers.** Clients remember node IDs under friendly names (`Brave Otter`, `Sleepy Heron`, …) and show a picker when you don't pass one.
- **arm32 builds.** A Dockerfile is included that cross-compiles for armv7.

## Quick start

We'll run a server that tunnels every client connection to sshd on the same host.

### 1. Start a server

On the machine running sshd:

```bash
proxy-rs tunnel --remote-host localhost --remote-port 22
```

```text
proxy-rs v0.2.0 tunnel -> localhost:22 clients=any config=/home/user/.proxy-rs/config.toml
WARNING: no allowlist: anyone with this node id can connect (see --allow, or [clients] in /home/user/.proxy-rs/config.toml)
2ee0c38203f21eada4dcc8d744053f5a0952cb7f4833ed16f387f588e0f39977
```

Note the `WARNING`: with no client allowlist any client that knows the node ID can connect. That's the simplest setup, not a safe one (see [Allowing clients](#allowing-clients)).

The first line sums up the server: version, mode, what it reaches, how many clients are allowed (`clients=any` means no allowlist) and its config file. The last line is the server's iroh **node ID**.

### 2. Connect a client

On your machine, open a local port and forward it to the server by its node ID:

```bash
proxy-rs --listen 127.0.0.1:2222 \
  --node-id 2ee0c38203f21eada4dcc8d744053f5a0952cb7f4833ed16f387f588e0f39977
```

```text
proxy-rs v0.2.0 listen 127.0.0.1:2222 config=/home/user/.proxy-rs/config.toml
Connected to "Plucky Narwhal" [2ee0c...977] (direct)
```

Now ssh to the server through the tunnel:

```bash
ssh -p 2222 user@127.0.0.1
```

Putting it together:

```text
           your machine                                  server machine
┌─────────────┐   ┌─────────────┐   .~~~~~~.   ┌─────────────┐   ┌─────────────┐
│ ssh         │──►│ proxy-rs    │──(  iroh  )─►│ proxy-rs    │──►│ sshd        │
│ -p 2222     │   │ --listen    │   '~~~~~~'   │ tunnel      │   │ port 22     │
└─────────────┘   └─────────────┘              └─────────────┘   └─────────────┘
```

## Concepts

### The config file

Each node keeps everything in one TOML file, `~/.proxy-rs/config.toml` by default, created on first run:

```toml
# This node's secret key; its node id derives from it. Keep it private.
secret-key = "90ab…c432"

# Servers this node connects to (client side): <node-id> = "<name>"
[servers]
2ee0c38203f21eada4dcc8d744053f5a0952cb7f4833ed16f387f588e0f39977 = "Plucky Narwhal"

# Clients allowed to connect (server side): <node-id> = "<label>"
[clients]
7c41d2…9e0b = "laptop"
```

Pass `--config <file>` (or `PROXY_RS_CONFIG`) to use another file. Because the key lives in the file, **each config file is a separate node with its own node ID**. That's how you run several servers on one host, or a server and a client on the same machine (a node can't connect to itself):

```bash
proxy-rs socks5 -t '*' --config ~/.proxy-rs/socks.toml
proxy-rs tunnel --remote-host localhost --remote-port 22 --config ~/.proxy-rs/ssh.toml
```

`proxy-rs whoami [--config <file>]` prints a node's ID and nothing else.

### Saved servers

The first time you use a `--node-id`, the client saves it in `[servers]` under a generated name. That's where `"Plucky Narwhal"` in the Quick start came from. Edit the file to rename or remove servers. After that, you can refer to the server by name:

```bash
proxy-rs --listen 127.0.0.1:2222 --name "Plucky Narwhal"
```

With neither `--node-id` nor `--name`, the client shows a menu of saved servers.

### Allowing clients

A server admits the node IDs in its config file's `[clients]` table plus any given with `--allow <client-node-id>`. The value in `[clients]` is a label, shown in the server's `Client connected:` output. The table is re-read on every connection, so adding or removing an entry takes effect without a restart.

If neither lists any client, the server accepts **any** client (unsafe) and prints a WARNING.

```bash
# on the client
proxy-rs whoami
# on the server
proxy-rs socks5 -t '*' --allow 7c41d2…9e0b
```

## Usage

```text
proxy-rs <socks5|http|tunnel|rsync> [OPTIONS]     run a server in that mode
proxy-rs --listen <ADDR> [-n <NODE_ID>]           run a client: forward a local port to a server
proxy-rs --pipe -n <NODE_ID> [ARGS]...            run a client: forward stdin/stdout to a server
proxy-rs whoami                                   print this node's id
```

### Server modes

A server runs exactly one mode. All modes take `--allow <NODE_ID>` (repeatable; env `PROXY_RS_ALLOW`, comma-separated).

---

#### `socks5`: SOCKS5 proxy

```bash
proxy-rs socks5 -t '*'                                 # clients can reach any target
proxy-rs socks5 -t '*.lan:*' -t '10.0.0.5:8000-8100'   # only part of the LAN
```

Then, with a client listening on `127.0.0.1:1080`: `curl --socks5-hostname 127.0.0.1:1080 http://example.com/`.

`-t, --target <PATTERN>` (required, repeatable; env `PROXY_RS_TARGET`, comma-separated) lists the targets clients may reach, as `host:port`. The host may be `*` or `*.domain`, the port `*` or `lo-hi`; bare `*` allows everything. Patterns are matched against the hostname the application sends, before DNS, so `localhost` doesn't admit `127.0.0.1`. A refused target gets SOCKS5 reply `0x02`.

---

#### `http`: HTTP proxy

```bash
proxy-rs http -t '*'
# on the client side: export https_proxy=http://127.0.0.1:8080 http_proxy=http://127.0.0.1:8080
```

Takes the same `-t` patterns as `socks5`. A refused target gets `403 Forbidden`, an unreachable one `502 Bad Gateway`.

---

#### `tunnel`: one fixed target

```bash
proxy-rs tunnel --remote-host 192.168.1.50 --remote-port 8123   # Home Assistant on the server's LAN
proxy-rs tunnel --remote-host localhost --remote-port 22        # sshd on the server itself
```

Every client connection is connected to `--remote-host:--remote-port` (env `PROXY_RS_REMOTE_HOST`, `PROXY_RS_REMOTE_PORT`), resolved and dialled from the server. There is nothing for a client to choose, so no `-t`.

---

#### `rsync`: push into volumes

```bash
proxy-rs rsync -v media:/srv/media -v home:$HOME/in
```

`-v, --volume name:path` (required, repeatable) declares a directory clients may push into. The path must exist, and `rsync` must be installed. Clients push with `rsync -e` (see [`--pipe`](#--pipe-forward-stdinstdout)).

---

### Client

#### `--listen`: forward a local port

```bash
proxy-rs --listen 127.0.0.1:1080 --name "Brave Otter"
```

Every connection to the address is forwarded to the server. What you point at it depends on the server's mode: a SOCKS5 client, an HTTP proxy client, or (for `tunnel`) whatever speaks the target's protocol.

| Flag | Env var | Description |
|---|---|---|
| `-l, --listen <ADDR>` | `PROXY_RS_LISTEN` | Local address to listen on. |
| `-n, --node-id <NODE_ID>` | `PROXY_RS_NODE_ID` | The server to connect to. |
| `--name <NAME>` | `PROXY_RS_NAME` | A saved server's name (or the name to save `--node-id` under). |
| `--retry` | `PROXY_RS_RETRY` | Keep running if the server is unreachable or lost. |

Like `ncat` or `telnet`, a client exits with an error if the server isn't online, or when it goes away. With `--retry` it stays up instead: it listens right away, waits for the server, and reconnects whenever it's lost. Use it for clients that should keep running, e.g. as a service.

#### `--pipe`: forward stdin/stdout

`--pipe` connects stdin/stdout to the server instead of listening, so another program can use `proxy-rs` as its transport. `-n` or `--name` is required (stdin is the data stream, so there's no menu).

rsync, against an `rsync` server:

```bash
rsync -av -e "proxy-rs --pipe --name 'Brave Otter'" ./photos/ "x:media/photos/"
```

- The host before the `:` (`x`) is a placeholder and is ignored, but required.
- The path after the `:` must start with one of the server's volume names (an unknown name is refused with the list of valid ones).
- Only push is supported. `rsync` must be installed on **both** ends.

ssh, against a `tunnel` server pointing at sshd, with no local port:

```bash
ssh -o ProxyCommand="proxy-rs --pipe --name 'Brave Otter'" user@anything
```

### Global options

| Flag | Env var | Description |
|---|---|---|
| `-c, --config <FILE>` | `PROXY_RS_CONFIG` | Config file (default `~/.proxy-rs/config.toml`). |

A flag on the command line always wins over its environment variable.

The default log level is `error`. For more detail, use `RUST_LOG`:

```bash
RUST_LOG=info proxy-rs socks5 -t '*'
RUST_LOG=proxy_rs=debug,iroh=info proxy-rs -l 127.0.0.1:1080 -n <id>
```

## ⚠️ Security model

Read this before exposing a server.

- **Clients are authenticated by their node ID.** iroh proves every client holds the secret key for the node ID it presents, and the server only admits node IDs on its allowlist (`--allow` / `[clients]`). The server's node ID alone is not enough to connect, **unless the allowlist is empty**, in which case every client is admitted.
- **Every allowed client can use everything the server's mode offers:**
  - `socks5` / `http`: open TCP connections to any target the `-t` patterns allow. With `-t '*'` that is **any host and port the server can reach**, including `127.0.0.1` on the server and its whole LAN;
  - `tunnel`: connect to the one configured target;
  - `rsync`: write into the volumes.
- Keep `-t` patterns as narrow as you can, or use `tunnel` when clients need a single target.
- The config file holds the node's secret key. Anyone who copies it can connect as that client (or impersonate that server). It is created with mode `0600`; don't share it. To revoke a client, remove it from `[clients]` (or drop its `--allow` and restart).
- Avoid running without an allowlist: the server's node ID becomes the only credential. Treat it like a password. Removing the last id from `[clients]` (with no `--allow`) opens the server to everyone.
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

The e2e test ([`tests/e2e/socks5_proxy.sh`](tests/e2e/socks5_proxy.sh)) starts a real `socks5` server and a client (`--listen`) in a throwaway `HOME`, each with its own `--config` file. It also starts a local `ncat` HTTP listener on port 4001 as the target. The server only allows `-t localhost:4001` and the test client's node ID (`--allow`). The test curls that listener through the proxy, then checks that a target outside the allowlist (`localhost:4002`) is refused with SOCKS5 reply `0x02`, and that a client with a different key is refused at startup. It needs `ncat` installed (the devcontainer has it).
