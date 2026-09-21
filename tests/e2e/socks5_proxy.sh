#!/usr/bin/env bash
# End-to-end smoke test: start a proxy-rs server + a proxy-rs SOCKS5 client,
# then curl the bun test-server (docker-compose service `bun`, see
# .devcontainer/docker-compose.yaml) through the proxy and check the response.
#
# Usage:
#   bash tests/e2e/socks5_proxy.sh
#   PROXY_RS_BIN=/path/to/proxy-rs bash tests/e2e/socks5_proxy.sh
#
# Normally invoked via `cargo test -- --ignored` (see tests/e2e.rs), which
# sets PROXY_RS_BIN for you.

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/../.." && pwd)

BIN="${PROXY_RS_BIN:-$REPO_ROOT/target/debug/proxy-rs}"
TARGET_URL="${PROXY_TEST_TARGET:-http://bun:3000/}"
EXPECTED_BODY="${PROXY_TEST_EXPECTED:-200 Hello, World!}"
SOCKS5_PORT=1080

if [[ ! -x "$BIN" ]]; then
    echo "proxy-rs binary not found at $BIN (run \`cargo build\` first, or set PROXY_RS_BIN)" >&2
    exit 1
fi

WORKDIR=$(mktemp -d)
cd "$WORKDIR"
export HOME="$WORKDIR" # isolate ~/.proxy-rs/{server-key,node-ids.yaml} from the real dev files

SERVER_PID=""
CLIENT_PID=""
cleanup() {
    [[ -n "$CLIENT_PID" ]] && kill "$CLIENT_PID" 2>/dev/null || true
    [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null || true
    wait 2>/dev/null || true
    rm -rf "$WORKDIR"
}
trap cleanup EXIT

echo "== starting proxy-rs server ($WORKDIR/server.log) =="
"$BIN" server >server.log 2>&1 &
SERVER_PID=$!

NODE_ID=""
for _ in $(seq 1 50); do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "server exited early:" >&2
        cat server.log >&2
        exit 1
    fi
    NODE_ID=$(grep -oP 'Iroh node listening \[\K[0-9a-f]+' server.log || true)
    [[ -n "$NODE_ID" ]] && break
    sleep 0.2
done
if [[ -z "$NODE_ID" ]]; then
    echo "server never printed its NodeId:" >&2
    cat server.log >&2
    exit 1
fi
echo "server NodeId: $NODE_ID"

echo "== starting proxy-rs SOCKS5 client ($WORKDIR/client.log) =="
"$BIN" client socks5 --listen "127.0.0.1:$SOCKS5_PORT" -n "$NODE_ID" >client.log 2>&1 &
CLIENT_PID=$!

READY=0
for _ in $(seq 1 50); do
    if ! kill -0 "$CLIENT_PID" 2>/dev/null; then
        echo "client exited early:" >&2
        cat client.log >&2
        exit 1
    fi
    if (echo >"/dev/tcp/127.0.0.1/$SOCKS5_PORT") 2>/dev/null; then
        READY=1
        break
    fi
    sleep 0.2
done
if [[ "$READY" -ne 1 ]]; then
    echo "client never opened SOCKS5 listener on port $SOCKS5_PORT:" >&2
    cat client.log >&2
    exit 1
fi

echo "== curling $TARGET_URL through SOCKS5 127.0.0.1:$SOCKS5_PORT =="
if ! BODY=$(curl -sf --max-time 10 --socks5 "127.0.0.1:$SOCKS5_PORT" "$TARGET_URL"); then
    echo "curl through proxy failed" >&2
    echo "-- server.log --" >&2; cat server.log >&2
    echo "-- client.log --" >&2; cat client.log >&2
    exit 1
fi

if [[ "$BODY" != *"$EXPECTED_BODY"* ]]; then
    echo "unexpected response body: $BODY" >&2
    exit 1
fi

echo "OK: got expected response through SOCKS5 proxy: $BODY"
