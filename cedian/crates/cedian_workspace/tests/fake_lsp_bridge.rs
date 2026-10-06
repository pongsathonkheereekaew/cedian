//! Hermetic bridge test: fake LSP → pump → HostTools (S1).
//!
//! Proves the pump/broadcast path WITHOUT rust-analyzer: the fake emits 3
//! diagnostic waves; HostTools must end with the 3-item wave keyed by the
//! workspace key. If THIS passes but live rust-analyzer doesn't, the bug is in
//! the rust-analyzer interaction — not the pump.

use cedian_workspace::{HostTools, LspBridge, WorkspaceHost};
use std::path::Path;
use std::time::Duration;

#[test]
fn fake_bridge_publishes_diagnostics() {
    let workdir = Path::new("/tmp/fake-ws");
    std::fs::create_dir_all(workdir).unwrap();
    let host = HostTools::shared(workdir);
    host.open(Path::new("/fake.rs"), "fn main() {}");

    let dir = env!("CARGO_MANIFEST_DIR").to_string();
    let _ = dir;
    // Bridge::spawn hardcodes the server cmd — spawn the client directly and
    // drive the pump path manually: attach via HostTools::attach_lsp needs a
    // bridge. Instead: build a bridge against the fake by spawning the fake
    // through a wrapper argv. LspBridge::spawn takes a bare command; use `sh`
    // wrapper? No — simplest: test pump logic via LspClient + manual publish.
    //
    // Full fake-bridge path needs LspBridge::spawn_argv — added below if missing.
    let fake = "/tmp/fake-lsp-entry.sh";
    std::fs::write(
        fake,
        "#!/bin/sh\nexec python3 /Users/pond/cedian/crates/cedian_lsp/tests/fake-lsp-server.py\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let argv = [
        "python3",
        "/Users/pond/cedian/crates/cedian_lsp/tests/fake-lsp-server.py",
    ];
    let bridge = LspBridge::spawn_argv(&argv, workdir, &host).expect("fake bridge");
    bridge.sync_buffer(&workdir.join("fake.rs"), "fn main() {}");

    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut found = Vec::new();
    while std::time::Instant::now() < deadline {
        let diags = host.diagnostics(Path::new("/fake.rs"));
        if diags.len() >= 3 {
            found = diags;
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    assert!(found.len() >= 3, "3-item wave landed in HostTools");
    assert!(found.iter().any(|d| d.message.contains("mismatched")));
    let _ = bridge;
}
