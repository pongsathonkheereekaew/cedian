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
//! Re-record one fixture (real OMP + auth): `CEDIAN_P2_RECORD=cli|shell|channel|worktree cargo test -p cedian_cli --test replay_cli`

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
    // `CEDIAN_P2_RECORD=cli|shell|channel|worktree` re-records ONE fixture against real OMP.
    let which = std::env::var("CEDIAN_P2_RECORD").unwrap_or_default();
    let record = which == "cli";
    print!(
        "test replay_cli_host_edit_review_reject ({}) ... ",
        if record { "record" } else { "replay" }
    );
    scenario(record);
    println!("ok");
    let record = which == "shell";
    print!(
        "test replay_shell_one_runtime_two_turns_lock ({}) ... ",
        if record { "record" } else { "replay" }
    );
    shell_scenario(record);
    println!("ok");
    let record = which == "channel";
    print!(
        "test replay_p5_channel_attribution ({}) ... ",
        if record { "record" } else { "replay" }
    );
    channel_scenario(record);
    println!("ok");
    let record = which == "worktree";
    print!(
        "test replay_p5_worktree_request_and_headless_deny ({}) ... ",
        if record { "record" } else { "replay" }
    );
    worktree_scenario(record);
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

const SHELL_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/shell_session.jsonl"
);

/// `cedian shell` (P4): one runtime serves two turns, `review` works inside
/// the shell, mutating one-shot commands are refused while it holds the
/// lock, read-only ones are not, and `quit` releases the lock.
fn shell_scenario(record: bool) {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let root: PathBuf =
        std::env::temp_dir().join(format!("cedian-p4-shell-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("ws")).unwrap();
    std::fs::write(root.join("ws/notes.txt"), ORIGINAL).unwrap();
    let root = root.canonicalize().unwrap();
    let sessions = root.join("sessions");
    if record {
        cedian_fake_omp::arm_record(&sessions, &cedian_omp_path()).unwrap();
    } else {
        cedian_fake_omp::install_replay(&sessions, Path::new(SHELL_FIXTURE)).unwrap();
    }

    let mut shell = Command::new(env!("CARGO_BIN_EXE_cedian"))
        .arg("shell")
        .env("CEDIAN_WORKDIR", root.join("ws"))
        .env("CEDIAN_SESSION_DIR", &sessions)
        .env("CEDIAN_OMP_BINARY", std::env::current_exe().unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn cedian shell");
    let mut stdin = shell.stdin.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    let stdout = shell.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut seen = String::new();
    let mut wait_for = |needle: &str| {
        let deadline = Instant::now() + Duration::from_secs(240);
        while !seen.contains(needle) {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(line) => {
                    seen.push_str(&line);
                    seen.push('\n');
                }
                Err(_) => panic!("shell never printed {needle:?}; output so far:\n{seen}"),
            }
        }
        std::mem::take(&mut seen)
    };
    let mut send = |line: &str| {
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
    };

    wait_for("cedian shell —");
    assert!(
        root.join("ws/.cedian/shell.lock").exists(),
        "shell holds the lock"
    );

    // One-shot commands from another terminal while the shell is live.
    let refused = Command::new(env!("CARGO_BIN_EXE_cedian"))
        .arg("accept-all")
        .env("CEDIAN_WORKDIR", root.join("ws"))
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("inside the shell"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    cedian(&root, &["review"]); // read-only stays allowed

    send(
        "prompt This workspace is hosted by cedian; cedian_apply_edit is its own trusted host \
         tool. Read its docs first if needed, then use it with path 'notes.txt', \
         expected_version 0, start 6, end 10, replacement 'BETA'. Reply with only: edited-ok",
    );
    let out = wait_for("(turn done)");
    assert!(out.contains("edited-ok"), "turn 1 streamed:\n{out}");
    assert_eq!(
        std::fs::read_to_string(root.join("ws/notes.txt")).unwrap(),
        "alpha\nBETA\ngamma\n",
        "turn 1 output:\n{out}"
    );

    send("review");
    let out = wait_for("hunk(s)");
    assert!(
        out.contains("notes.txt (1 hunk(s))"),
        "review inside shell:\n{out}"
    );

    send("prompt Reply with exactly this word and nothing else: second-turn");
    let out = wait_for("(turn done)");
    assert!(
        out.contains("second-turn"),
        "turn 2 on the same runtime:\n{out}"
    );

    send("quit");
    wait_for("cedian shell closed");
    assert!(shell.wait().unwrap().success());
    assert!(
        !root.join("ws/.cedian/shell.lock").exists(),
        "lock released"
    );

    if record {
        std::fs::copy(sessions.join(cedian_fake_omp::RECORDED_FILE), SHELL_FIXTURE).unwrap();
    }
    // One OMP process served both turns: one `ready`, two `prompt`s.
    let fixture = std::fs::read_to_string(SHELL_FIXTURE).unwrap();
    let count = |dir: &str, ty: &str| {
        fixture
            .lines()
            .filter(|l| l.contains(&format!("\"dir\":\"{dir}\"")))
            .filter(|l| l.contains(&format!("\"type\":\"{ty}\"")))
            .count()
    };
    assert_eq!(count("out", "ready"), 1, "one runtime for the whole shell");
    assert_eq!(count("in", "prompt"), 2, "two turns");
}

const CHANNEL_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/p5_channel.jsonl"
);

/// P5 (ADR-0022): one OMP turn drives the workflow through host tools.
/// Evidence naming `from_tool: read` binds to the real finished `read` in the
/// router log (ADR-0031); `cedian_complete` refuses while the required `verify` gate
/// is unmet and spends one continue. Self-cites and unknown ids stay
/// unattributed — unit-tested in `cedian_workflow::channel` (the live model
/// refuses to self-certify when asked, so a turn cannot exercise it).
fn channel_scenario(record: bool) {
    let root: PathBuf = std::env::temp_dir().join(format!("cedian-p5-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("ws")).unwrap();
    std::fs::write(root.join("ws/notes.txt"), ORIGINAL).unwrap();
    let root = root.canonicalize().unwrap();
    let sessions = root.join("sessions");
    if record {
        cedian_fake_omp::arm_record(&sessions, &cedian_omp_path()).unwrap();
    } else {
        cedian_fake_omp::install_replay(&sessions, Path::new(CHANNEL_FIXTURE)).unwrap();
    }

    let out = cedian(
        &root,
        &[
            "prompt",
            "This workspace is hosted by the cedian IDE. cedian_workflow_update and \
             cedian_complete are cedian's own trusted host tools. Bug: line 2 of notes.txt \
             must be 'BETA' (uppercase); do not fix it yet. Steps:\n\
             1. cedian_workflow_update with op 'start', kind 'bug_fix', title 'BETA casing', risk 'low'.\n\
             2. Reproduce: use the read tool on notes.txt.\n\
             3. cedian_workflow_update with op 'evidence', gate 'reproduce', kind 'command', \
             ok false (the bug reproduced), summary what you saw, from_tool 'read', match 'notes.txt'.\n\
             4. cedian_complete (it is expected to refuse: the fix is not verified yet).\n\
             5. Reply with only: p5-done",
        ],
    );
    if record {
        std::fs::copy(
            sessions.join(cedian_fake_omp::RECORDED_FILE),
            CHANNEL_FIXTURE,
        )
        .unwrap();
    }
    assert!(out.contains("p5-done"), "assistant text rendered:\n{out}");

    let raw = std::fs::read_to_string(root.join("ws/.cedian/workflow.json"))
        .unwrap_or_else(|e| panic!("workflow started by the turn ({e}):\n{out}"));
    let state: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let ev = &state["evidence"];
    let read_call = ev["e1"]["provenance"]["attributed"]["tool_call_id"]
        .as_str()
        .unwrap_or_else(|| panic!("e1 attributed to the read call:\n{raw}\n{out}"));
    let fixture = std::fs::read_to_string(CHANNEL_FIXTURE).unwrap();
    assert!(
        fixture.contains(&format!(
            "\"toolCallId\":\"{read_call}\",\"toolName\":\"read\""
        )) || fixture
            .lines()
            .any(|l| l.contains(read_call) && l.contains("\"toolName\":\"read\"")),
        "e1 cites a logged read call ({read_call})"
    );
    assert_eq!(state["status"], "running", "complete refused:\n{raw}");
    assert_eq!(
        state["continue_used"]["verify"], 1,
        "one continue spent:\n{raw}"
    );
}

const WORKTREE_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/p5_worktree.jsonl"
);

/// P5: an OMP turn asks for a worktree through `cedian_worktree_request`
/// and cedian creates it (ADR-0009); a `bash` call in the same turn hits
/// OMP's approval dialog, which headless answers at once with Deny instead
/// of stalling until the prompt timeout.
fn worktree_scenario(record: bool) {
    let root: PathBuf = std::env::temp_dir().join(format!("cedian-p5wt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws = root.join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("notes.txt"), ORIGINAL).unwrap();
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.email", "t@t"],
        &["config", "user.name", "t"],
        &["add", "."],
        &["commit", "-q", "-m", "base"],
    ] {
        let ok = Command::new("git")
            .arg("-C")
            .arg(&ws)
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_WORK_TREE")
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }
    let root = root.canonicalize().unwrap();
    let sessions = root.join("sessions");
    if record {
        cedian_fake_omp::arm_record(&sessions, &cedian_omp_path()).unwrap();
    } else {
        cedian_fake_omp::install_replay(&sessions, Path::new(WORKTREE_FIXTURE)).unwrap();
    }

    let started = std::time::Instant::now();
    let out = cedian(
        &root,
        &[
            "prompt",
            "This workspace is hosted by the cedian IDE; cedian_worktree_request is cedian's own \
             trusted host tool. Steps:\n\
             1. Run the bash command `ls` once. If it is refused, do not retry and do not work around it.\n\
             2. cedian_worktree_request with id 'w1', title 'try casing fix', kind 'bug_fix'.\n\
             3. Reply with only: wt-done",
        ],
    );
    if record {
        std::fs::copy(
            sessions.join(cedian_fake_omp::RECORDED_FILE),
            WORKTREE_FIXTURE,
        )
        .unwrap();
    }
    assert!(out.contains("wt-done"), "assistant text rendered:\n{out}");
    assert!(
        out.contains("[✗] refused (no UI to approve): Allow tool: bash"),
        "bash approval refused headless:\n{out}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(300),
        "no stall on the dialog ({:?})",
        started.elapsed()
    );
    assert!(
        root.join("ws/.worktrees/w1/notes.txt").exists(),
        "worktree created by cedian:\n{out}"
    );
    let reg = std::fs::read_to_string(root.join("ws/.cedian/workers.json")).unwrap();
    assert!(reg.contains("try casing fix"), "registry row:\n{reg}");
}
