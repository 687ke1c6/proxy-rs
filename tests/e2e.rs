//! End-to-end smoke tests.
//!
//! These spin up real proxy-rs server + client processes and hit the bun
//! test-server (see `.devcontainer/docker-compose.yaml`, service `bun`)
//! through the proxy, so they need the devcontainer's docker network and are
//! `#[ignore]`d by default. Run explicitly with:
//!
//!     cargo test -- --ignored

use std::process::Command;

#[test]
#[ignore]
fn socks5_proxy_smoke_test() {
    let status = Command::new("bash")
        .arg("tests/e2e/socks5_proxy.sh")
        .env("PROXY_RS_BIN", env!("CARGO_BIN_EXE_proxy-rs"))
        .status()
        .expect("failed to launch tests/e2e/socks5_proxy.sh");

    assert!(status.success(), "socks5 proxy e2e script failed");
}
