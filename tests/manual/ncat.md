# Manual testing with ncat

`ncat` listeners that make handy targets for the TCP proxy. `-l` listens, `-k` keeps it open for more than one connection, and `-c` runs a shell command per connection with its stdin/stdout wired to the socket.

## Listeners (test targets)

**1. Echo server** (checks basic two-way traffic)
```bash
ncat -lk 4000 -c cat
```

**2. Fixed HTTP response** (works with `curl` through the SOCKS5 or HTTP client)
```bash
ncat -lk 4001 -c 'printf "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 6\r\nConnection: close\r\n\r\nhello\n"'
```

**3. Serve a file** (compare checksums on both ends)
```bash
head -c 50M /dev/urandom > /tmp/blob && sha256sum /tmp/blob
ncat -lk 4002 -c 'cat /tmp/blob'
```

**4. Large stream** (throughput test)
```bash
ncat -lk 4003 -c 'head -c 1G /dev/zero'
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
The server only replies after the client closes its write side. If you get no checksum back, the proxy isn't passing half-close through. (Not `ncat -lk 4004 -c sha256sum`: ncat doesn't pass the socket's EOF on to the `-c` command, so that never replies to a half-close, even without the proxy.)

**6. Slow drip** (checks that data streams through instead of being buffered until the end)
```bash
ncat -lk 4005 -c 'for i in $(seq 10); do echo "tick $i $(date +%T)"; sleep 1; done'
```

**7. Interactive chat** (several clients typing to each other)
```bash
ncat -lk 4006 --chat
```

## Proxy server to reach them

```bash
cargo run -- server --allow <client-id> -t 'localhost:4000-4010'
```

## Connecting through the proxy client

`ncat` can act as a SOCKS5 or HTTP CONNECT client itself. Use `--proxy-dns remote` so it sends the hostname `localhost` rather than resolving it first. The `-t` allowlist matches the hostname string, so a resolved `127.0.0.1` would be refused.

```bash
# via `client socks5 --listen 127.0.0.1:1080`
ncat --proxy 127.0.0.1:1080 --proxy-type socks5 --proxy-dns remote localhost 4000

# via `client http --listen 127.0.0.1:8080` (uses CONNECT)
ncat --proxy 127.0.0.1:8080 --proxy-type http localhost 4000

# via `client tunnel --listen 127.0.0.1:9000 --remote-host localhost --remote-port 4000`
ncat 127.0.0.1 9000
```

Useful combinations:

```bash
# #2 with curl
curl --socks5-hostname 127.0.0.1:1080 http://localhost:4001/
curl -x http://127.0.0.1:8080 http://localhost:4001/        # plain-HTTP (non-CONNECT) path

# #3 integrity: should match the sha256sum printed earlier
ncat --proxy 127.0.0.1:1080 --proxy-type socks5 --proxy-dns remote localhost 4002 | sha256sum

# #4 throughput (pv if installed, otherwise wc -c)
time ncat --proxy 127.0.0.1:1080 --proxy-type socks5 --proxy-dns remote localhost 4003 | pv > /dev/null

# #5 upload and get the checksum back. Needs a client that half-closes and then keeps
# reading; ncat exits on stdin EOF instead, so use python:
python3 -c '
import socket, sys
s = socket.create_connection(("127.0.0.1", 9000))   # e.g. `client tunnel --listen 127.0.0.1:9000 --remote-host localhost --remote-port 4004`
s.sendall(open("/tmp/blob", "rb").read()); s.shutdown(socket.SHUT_WR)
while (d := s.recv(65536)): sys.stdout.buffer.write(d)'

# refusal check: port 4011 is outside the -t range, so the server should refuse it
ncat --proxy 127.0.0.1:1080 --proxy-type socks5 --proxy-dns remote localhost 4011
```

Add `-v` to either end of `ncat` to see connection and close events. It's especially useful on #5 to watch the half-close happen.
