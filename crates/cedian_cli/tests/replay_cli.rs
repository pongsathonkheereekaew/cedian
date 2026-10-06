//! P2 hermetic S0 loop through the real CLI binary (§86):
//! `cedian prompt` (recorded OMP turn calling `cedian_apply_edit`) → tool card
//! → buffer edit synced to disk → `cedian review` shows the task-attributed
//! hunk → `cedian reject` restores the baseline — each step its own process,
//! so the persisted `.cedian/review.json` is exercised too.
//!
//! `harness = false`: when the CLI spawns OMP, it spawns THIS test binary
//! (`CEDIAN_OMP_BINARY`), which then acts as fake-omp.
//!
//! Hermetic: `cargo test -p cedian_cli --test replay_cli`
//! Re-record (real OMP + auth): `CEDIAN_P2_RECORD=1 cargo test -p cedian_cli --test replay_cli`

use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/cli_host_edit.jsonl"
);
const ORIGINAL: &str = "alpha\nbeta\ngamma\n";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--mode") {
        std::process::exit(cedian_fake_omp::run(&args));
    }
    let record = std::env::var("CEDIAN_P2_RECORD").as_deref() == Ok("1");
    print!(
        "test replay_cli_host_edit_review_reject ({}) ... ",
        if record { "record" } else { "replay" }
    );
    scenario(record);
    println!("ok");
}

fn cedian(root: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_cedian"))
        .args(args)
        .env("CEDIAN_WORKDIR", root.join("ws"))
        .env("CEDIAN_SESSION_DIR", root.join("sessions"))
        .env("CEDIAN_OMP_BINARY", std::env::current_exe().unwrap())
        .output()
        .expect("run cedian");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "cedian {args:?} failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    stdout
}

fn scenario(record: bool) {
    let root: PathBuf = std::env::temp_dir().join(format!("cedian-p2-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("ws")).unwrap();
    let notes = root.join("ws/notes.txt");
    std::fs::write(&notes, ORIGINAL).unwrap();
    let root = root.canonicalize().unwrap();
    let sessions = root.join("sessions");
    if record {
        let real = cedian_omp_path();
        cedian_fake_omp::arm_record(&sessions, &real).unwrap();
    } else {
        cedian_fake_omp::install_replay(&sessions, Path::new(FIXTURE)).unwrap();
    }

    let out = cedian(
        &root,
        &[
            "prompt",
            "Call cedian_apply_edit with path 'notes.txt', expected_version 0, start 6, end 10, \
             replacement 'BETA'. Do not use any other tool. Then reply with only: edited-ok",
        ],
    );
    if record {
        std::fs::copy(sessions.join(cedian_fake_omp::RECORDED_FILE), FIXTURE).unwrap();
    }
    assert!(out.contains("edited-ok"), "assistant text rendered:\n{out}");
    assert!(out.contains("[✓]"), "a done tool card rendered:\n{out}");
    assert_eq!(
        std::fs::read_to_string(&notes).unwrap(),
        "alpha\nBETA\ngamma\n"
    );

    let review = cedian(&root, &["review"]);
    assert!(
        review.contains("notes.txt (1 hunk(s))"),
        "one attributed hunk:\n{review}"
    );

    cedian(&root, &["reject", "/notes.txt", "0"]);
    assert_eq!(
        std::fs::read_to_string(&notes).unwrap(),
        ORIGINAL,
        "reject restored baseline"
    );
    let review = cedian(&root, &["review"]);
    assert!(!review.contains("(1 hunk(s))"), "hunk resolved:\n{review}");
}

fn cedian_omp_path() -> PathBuf {
    std::env::var_os("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .map(|d| d.join("omp"))
        .find(|p| p.is_file())
        .expect("omp on PATH for recording")
}
