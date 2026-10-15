//! The person's `~/.omp` stays a protected root next to cedian's own agent
//! dirs (ADR-0078 moved the agent out of it; a bash write into it is still
//! reverted and audited, as before). A temp HOME, never the real one.

use cedian_omp::{ProtectedKind, ProtectedRoots, approval_targets_protected};

#[test]
fn a_bash_write_into_the_persons_omp_dir_is_caught() {
    let root = std::env::temp_dir().join(format!("cedian-protected-omp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let home = root.join("home");
    let ws = root.join("ws");
    std::fs::create_dir_all(home.join(".omp/agent")).unwrap();
    std::fs::create_dir_all(&ws).unwrap();
    unsafe {
        // SAFETY: the only test in this binary.
        for name in ["XDG_CONFIG_HOME", "XDG_STATE_HOME", "CEDIAN_STATE_DIR"] {
            std::env::remove_var(name);
        }
        std::env::set_var("HOME", &home);
    }
    let roots = ProtectedRoots::for_launch(ws.clone(), &root.join("state"), None);
    let target = home.join(".omp/agent/config.yml");
    assert_eq!(roots.classify(&target), Some(ProtectedKind::Session));
    let title = format!("Allow tool: bash\nCommand: echo x > {}", target.display());
    assert!(approval_targets_protected(&title, &roots), "{title}");
    assert_eq!(roots.classify(&ws.join("src/main.rs")), None);
    let _ = std::fs::remove_dir_all(&root);
}
