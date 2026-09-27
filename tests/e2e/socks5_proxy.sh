#!/usr/bin/env bash
# End-to-end smoke test: start a proxy-rs server + a proxy-rs SOCKS5 client,
# then curl a local ncat HTTP listener (started by this script) through the
# proxy and check the response.
# The server only allows `-t $ALLOWED_TARGET`, so a second curl to a target
# outside that allowlist must be refused with SOCKS5 reply 0x02 ("not allowed").
# The server only admits the test client's node id (--allow), so a client with a
# different key (its own --config-dir) must be refused at its startup ping.
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
TARGET_PORT="${PROXY_TEST_TARGET_PORT:-4001}"  # the ncat listener this script starts
TARGET_URL="${PROXY_TEST_TARGET:-http://localhost:$TARGET_PORT/}"
EXPECTED_BODY="${PROXY_TEST_EXPECTED:-hello}"
ALLOWED_TARGET="${PROXY_TEST_ALLOWED:-localhost:$TARGET_PORT}"  # -t pattern that admits TARGET_URL
DENIED_URL="${PROXY_TEST_DENIED_URL:-http://localhost:$((TARGET_PORT + 1))/}"  # must NOT match ALLOWED_TARGET
SOCKS5_PORT=1080

if [[ ! -x "$BIN" ]]; then
    echo "proxy-rs binary not found at $BIN (run \`cargo build\` first, or set PROXY_RS_BIN)" >&2
    exit 1
fi
if ! command -v ncat >/dev/null; then
    echo "ncat not found (install the nmap ncat package)" >&2
    exit 1
fi

WORKDIR=$(mktemp -d)
cd "$WORKDIR"
export HOME="$WORKDIR" # isolate ~/.proxy-rs/{server-key,nodes.yaml} from the real dev files

NCAT_PID=""
SERVER_PID=""
CLIENT_PID=""
cleanup() {
    [[ -n "$NCAT_PID" ]] && kill "$NCAT_PID" 2>/dev/null || true
    [[ -n "$CLIENT_PID" ]] && kill "$CLIENT_PID" 2>/dev/null || true
    [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null || true
    wait 2>/dev/null || true
    rm -rf "$WORKDIR"
}
trap cleanup EXIT

echo "== starting ncat HTTP target on localhost:$TARGET_PORT ($WORKDIR/ncat.log) =="
ncat -lk 127.0.0.1 "$TARGET_PORT" \
    -c 'printf "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 6\r\nConnection: close\r\n\r\nhello\n"' \
    >ncat.log 2>&1 &
NCAT_PID=$!
for _ in $(seq 1 50); do
    (echo >"/dev/tcp/127.0.0.1/$TARGET_PORT") 2>/dev/null && break
    if ! kill -0 "$NCAT_PID" 2>/dev/null; then
        echo "ncat exited early:" >&2
        cat ncat.log >&2
        exit 1
    fi
    sleep 0.2
done

CLIENT_ID=$("$BIN" client whoami)
echo "client NodeId: $CLIENT_ID"

echo "== starting proxy-rs server ($WORKDIR/server.log) =="
"$BIN" server -t "$ALLOWED_TARGET" --allow "$CLIENT_ID" >server.log 2>&1 &
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

# --socks5-hostname (not --socks5): the server's -t allowlist matches the hostname
# the client sends, so curl must not resolve it to an IP locally.
echo "== curling $TARGET_URL through SOCKS5 127.0.0.1:$SOCKS5_PORT (allowed by -t $ALLOWED_TARGET) =="
if ! BODY=$(curl -sf --max-time 10 --socks5-hostname "127.0.0.1:$SOCKS5_PORT" "$TARGET_URL"); then
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

echo "== curling $DENIED_URL through SOCKS5 127.0.0.1:$SOCKS5_PORT (not allowed) =="
CURL_EXIT=0
DENIED_ERR=$(curl -sS --max-time 10 --socks5-hostname "127.0.0.1:$SOCKS5_PORT" "$DENIED_URL" 2>&1 >/dev/null) || CURL_EXIT=$?
# curl exit 97 = proxy handshake failed; "(2)" is the SOCKS5 reply code
# "connection not allowed by ruleset".
if [[ "$CURL_EXIT" -ne 97 || "$DENIED_ERR" != *"(2)"* ]]; then
    echo "expected SOCKS5 'not allowed' refusal (curl exit 97, reply 2), got exit $CURL_EXIT: $DENIED_ERR" >&2
    echo "-- server.log --" >&2; cat server.log >&2
    echo "-- client.log --" >&2; cat client.log >&2
    exit 1
fi

echo "OK: target outside the allowlist was refused: $DENIED_ERR"

echo "== starting a client that is not in the server's --allow list =="
STRANGER_OUT=$(timeout 30 "$BIN" --config-dir "$WORKDIR/stranger" client socks5 --listen "127.0.0.1:$((SOCKS5_PORT + 1))" -n "$NODE_ID" 2>&1) && STRANGER_EXIT=0 || STRANGER_EXIT=$?
if [[ "$STRANGER_EXIT" -eq 0 || "$STRANGER_EXIT" -eq 124 || "$STRANGER_OUT" != *"not in its allowlist"* ]]; then
    echo "expected the unlisted client to be refused, got exit $STRANGER_EXIT: $STRANGER_OUT" >&2
    exit 1
fi

echo "OK: unlisted client was refused"
