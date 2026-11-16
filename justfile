# Run `just` to list recipes. Extra arguments after a recipe's own
# parameters are passed through to proxy-rs (e.g. `just listen "Brave Otter" 127.0.0.1:1080 --retry`).

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

# Config file for the server recipes. Separate from the default one the client recipes
# use, so a server and a client can run on this machine (each file is its own node id).
server_config := env_var('HOME') + "/.proxy-rs/server.toml"

# Print this machine's client node id (for a server's --allow)
whoami:
    cargo run -q -- whoami

# Run a server: `just server socks5 -t '*'`, `just server tunnel --remote-host localhost --remote-port 22`
server mode *args:
    cargo run -- {{mode}} --config "{{server_config}}" {{args}}

# Run an open SOCKS5 server (any target) allowing only this machine's client key
server-local *args:
    cargo run -- socks5 --config "{{server_config}}" --allow "$(cargo run -q -- whoami)" -t '*' {{args}}

# Client: forward local <listen> to the saved server <name> (omit name for the interactive menu)
listen name="" listen="127.0.0.1:1080" *args:
    cargo run -- --listen {{listen}} {{ if name != "" { "--name '" + name + "'" } else { "" } }} {{args}}

# rsync push a directory into a volume of an rsync server, e.g. `just sync <id> ./dir/ media/backup/`
sync node src dest: build
    rsync -av -e "'{{justfile_directory()}}/{{bin}}' --pipe -n {{node}}" {{src}} "x:{{dest}}"

# Build the runtime Docker image (Alpine, musl)
docker tag="proxy-rs":
    docker build -t {{tag}} .

clean:
    cargo clean
