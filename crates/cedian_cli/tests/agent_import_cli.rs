//! `cedian agent import` through the real binary with a temp HOME: the
//! person's `~/.omp/agent` skills land in `~/.config/cedian/agent`, auth
//! never does, and `~/.omp` is only read (ADR-0078 decision 5).
#![allow(
    clippy::disallowed_methods,
    reason = "headless, synchronous process control: Zed's async spawn helpers do not apply"
)]

use std::path::PathBuf;
use std::process::{Command, Output};

fn run(home: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cedian"))
        .arg("agent")
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap()
}

#[test]
fn import_dry_runs_then_copies_skills_into_cedians_config_dir() {
    let root = std::env::temp_dir().join(format!("cedian-agent-import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let home = root.join("home");
    let omp = home.join(".omp/agent");
    std::fs::create_dir_all(omp.join("skills/alpha")).unwrap();
    std::fs::write(omp.join("skills/alpha/SKILL.md"), "alpha").unwrap();
    std::fs::write(omp.join("agent.db"), "TOKENS").unwrap();
    let dest = home.join(".config/cedian/agent");

    let out = run(&home, &["import"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("would copy skills/alpha"), "{stdout}");
    assert!(!dest.exists(), "a dry run writes nothing");

    let out = run(&home, &["import", "--apply"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("skills/alpha/SKILL.md")).unwrap(),
        "alpha"
    );
    assert!(!dest.join("agent.db").exists());

    let out = run(&home, &["import", "--apply"]);
    assert!(
        !out.status.success(),
        "a second import is refused without --force"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("--force"));
    let _ = std::fs::remove_dir_all(&root);
}
