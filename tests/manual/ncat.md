# Manual testing with ncat

`ncat` listeners that make handy targets for the TCP proxy. `-l` listens, `-k` keeps it open for more than one connection, and `-c` runs a shell command per connection with its stdin/stdout wired to the socket.

Each test below has a listener and a client command that connects to it **directly**. Run the direct client first to see what "working" looks like, then run the same command through the proxy (see [Going through the proxy](#going-through-the-proxy)) and check you get the same result.

## Tests

**1. Echo server** (checks basic two-way traffic)
```bash
ncat -lk 4000 -c cat
```
Client: type lines, each one should come straight back.
```bash
ncat localhost 4000
```

**2. Fixed HTTP response** (works with `curl` through the SOCKS5 or HTTP client)
```bash
ncat -lk 4001 -c 'printf "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 6\r\nConnection: close\r\n\r\nhello\n"'
```
Client: should print `hello`.
```bash
curl http://localhost:4001/
```

**3. Serve a file** (compare checksums on both ends)
```bash
head -c 50M /dev/urandom > /tmp/blob && sha256sum /tmp/blob
ncat -lk 4002 -c 'cat /tmp/blob'
```
Client: the hash should match the one printed above.
```bash
ncat --recv-only localhost 4002 | sha256sum
```

**4. Large stream** (throughput test)
```bash
ncat -lk 4003 -c 'head -c 1G /dev/zero'
```
Client: should count 1073741824 bytes. Compare the time with and without the proxy (or pipe into `pv` instead of `wc -c` to watch the rate).
```bash
time ncat --recv-only localhost 4003 | wc -c
```

**5. Upload sink that replies with a checksum** (checks the client→server direction, and that half-close gets through the proxy)
```bash
python3 -c '
import socket, hashlib
l = socket.create_server(("127.0.0.1", 4004))
while True:
    c, _ = l.accept(); h = hashlib.sha256()
    while (d := c.recv(65536)): h.update(d)
    c.sendall((h.hexdigest() + "  -\n").encode()); c.close()'
```
The server only replies after the client closes its write side. (Not `ncat -lk 4004 -c sha256sum`: ncat doesn't pass the socket's EOF on to the `-c` command, so that never replies, even without the proxy.)

Client: sends `/tmp/blob` (from #3), half-closes, then prints the reply. It should match `sha256sum /tmp/blob`. If you get no checksum back, the proxy isn't passing half-close through.
```bash
ncat localhost 4004 < /tmp/blob
```

**6. Slow drip** (checks that data streams through instead of being buffered until the end)
```bash
ncat -lk 4005 -c 'for i in $(seq 10); do echo "tick $i $(date +%T)"; sleep 1; done'
```
Client: ticks should arrive one per second, not all at once after 10 seconds.
```bash
ncat --recv-only localhost 4005
```

**7. Interactive chat** (several clients typing to each other)
```bash
ncat -lk 4006 --chat
```
Client: run it in two or more terminals. Lines typed in one should show up in the others.
```bash
ncat localhost 4006
```

## Going through the proxy

Start a server that allows the test ports:

```bash
cargo run -- server --allow <client-id> -t 'localhost:4000-4010'
```

Then take any client command from above and route it through one of the proxy clients:

| Proxy client | `ncat` client: add | `curl` client: add |
|---|---|---|
| `client socks5 --listen 127.0.0.1:1080` | `--proxy 127.0.0.1:1080 --proxy-type socks5 --proxy-dns remote` | `--socks5-hostname 127.0.0.1:1080` |
| `client http --listen 127.0.0.1:8080` | `--proxy 127.0.0.1:8080 --proxy-type http` (uses CONNECT) | `-x http://127.0.0.1:8080` (plain-HTTP, non-CONNECT path) |
| `client tunnel --listen 127.0.0.1:9000 --remote-host localhost --remote-port <port>` | nothing, but replace `localhost <port>` with `127.0.0.1 9000` | nothing, but replace `localhost:<port>` with `127.0.0.1:9000` |

Keep the target hostname as `localhost` (not `127.0.0.1`). The `-t` allowlist matches the hostname string the client sends, so for SOCKS5 use `--proxy-dns remote` / `--socks5-hostname` so it isn't resolved locally first.

For example, #3 through each:

```bash
ncat --recv-only --proxy 127.0.0.1:1080 --proxy-type socks5 --proxy-dns remote localhost 4002 | sha256sum
ncat --recv-only --proxy 127.0.0.1:8080 --proxy-type http localhost 4002 | sha256sum
ncat --recv-only 127.0.0.1 9000 | sha256sum     # tunnel with --remote-port 4002
```

Refusal check: port 4011 is outside the `-t` range, so the server should refuse it (SOCKS5 reply 2 "not allowed").

```bash
ncat --proxy 127.0.0.1:1080 --proxy-type socks5 --proxy-dns remote localhost 4011
```

Add `-v` to either end of `ncat` to see connection and close events. It's especially useful on #5 to watch the half-close happen.
