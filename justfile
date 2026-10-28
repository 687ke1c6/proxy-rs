# Run `just` to list recipes. Extra arguments after a recipe's own
# parameters are passed through to proxy-rs (e.g. `just socks5 Brave Otter -d /tmp/cfg`).

bin := "target/debug/proxy-rs"

default:
    @just --list

# Build (debug)
build:
    cargo build

# Build an optimized binary into target/release/proxy-rs
release:
    cargo build --release

# Lint and run unit tests
check: lint test

# Run clippy on the whole workspace, warnings as errors
lint:
    cargo clippy --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# Unit tests
test *args:
    cargo test --workspace {{args}}

# End-to-end SOCKS5 smoke test (needs ncat)
e2e:
    cargo test --test e2e -- --ignored --nocapture

# Print this client's node id (for a server's --allow)
whoami:
    cargo run -q -- client whoami

# Run a server; pass flags through, e.g. `just server --allow <id> -t localhost:22`
server *args:
    cargo run -- server {{args}}

# Run an open-proxy server (any target) allowing only this machine's client key
server-local *args:
    cargo run -- server --allow "$(cargo run -q -- client whoami)" -t '*' {{args}}

# SOCKS5 proxy client to the saved node <name> (omit name for the interactive menu)
socks5 name="" listen="127.0.0.1:1080" *args:
    cargo run -- client socks5 --listen {{listen}} {{ if name != "" { "--name '" + name + "'" } else { "" } }} {{args}}

# HTTP proxy client to the saved node <name> (omit name for the interactive menu)
http name="" listen="127.0.0.1:8080" *args:
    cargo run -- client http --listen {{listen}} {{ if name != "" { "--name '" + name + "'" } else { "" } }} {{args}}

# ssh -L style tunnel: local <listen> -> <remote> (host:port) on the server's side
tunnel node remote listen="127.0.0.1:9000" *args:
    cargo run -- client tunnel -n {{node}} --listen {{listen}} \
        --remote-host "$(echo '{{remote}}' | sed 's/:[^:]*$//')" \
        --remote-port "$(echo '{{remote}}' | sed 's/.*://')" {{args}}

# List a server's volumes
volumes node:
    cargo run -- client volumes -n {{node}}

# Send a file into a server volume, e.g. `just send <id> ./a.mkv media/movies`
send node path target *args:
    cargo run -- client file {{path}} -n {{node}} -t {{target}} {{args}}

# rsync push a directory into a server volume, e.g. `just sync <id> ./dir/ media/backup/`
sync node src dest: build
    rsync -av -e "'{{justfile_directory()}}/{{bin}}' client sync-rsh -n {{node}}" {{src}} "x:{{dest}}"

# Build the runtime Docker image (Alpine, musl)
docker tag="proxy-rs":
    docker build -t {{tag}} .

# Cross-compile for armv7 (Raspberry Pi) via Docker; binary lands in target/armv7/
armv7:
    docker build -f Dockerfile.armv7 --output type=local,dest=target/armv7 .

clean:
    cargo clean
