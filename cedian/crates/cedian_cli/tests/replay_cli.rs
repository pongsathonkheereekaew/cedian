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
//! Re-record one fixture (real OMP + auth): `CEDIAN_P2_RECORD=cli|shell|channel|worktree|revert|s2 cargo test -p cedian_cli --test replay_cli`

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
    // `CEDIAN_P2_RECORD=cli|shell|channel|worktree|revert|s2` re-records ONE fixture against real OMP.
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
    let record = which == "revert";
    print!(
        "test replay_p6_inline_edit_revert_turn ({}) ... ",
        if record { "record" } else { "replay" }
    );
    revert_scenario(record);
    println!("ok");
    let record = which == "s2";
    print!(
        "test replay_s2_bugfix_skill_blocked_claim ({}) ... ",
        if record { "record" } else { "replay" }
    );
    s2_blocked_scenario(record);
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
    // S2 fast lane (ADR-0026): a trivial edit with no floor gate lands with
    // no workflow at all — nothing started, nothing blocked.
    assert!(
        !root.join("ws/.cedian/workflow.json").exists(),
        "fast lane: no workflow"
    );
    assert!(!out.contains("workflow"), "no workflow noise:\n{out}");
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
/// is unmet and spends one continue; the turn ends on that refusal, so the
/// workflow is blocked (ADR-0036). Self-cites and unknown ids stay
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
    // ADR-0036: the turn ended on a refused claim → blocked, gates listed.
    assert_eq!(state["status"], "blocked", "refused claim blocks:\n{raw}");
    assert!(
        out.contains("workflow BLOCKED") && out.contains("required gate \"verify\""),
        "missing gates printed after the turn:\n{out}"
    );
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

const REVERT_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/p6_revert.jsonl"
);

/// P6 (ADR-0026): in one `cedian shell`, an inline `edit` turn changes only
/// its target line; a prompt turn changes two more lines; the user then
/// rewrites one of them on disk. `revert-turn 2` puts back the other line
/// and keeps the user's (STALE); reverting that revert redoes it; reverting
/// the inline edit restores its line. Every step is an observed file state.
fn revert_scenario(record: bool) {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let root: PathBuf = std::env::temp_dir().join(format!("cedian-p6-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("ws")).unwrap();
    let notes = root.join("ws/notes.txt");
    std::fs::write(&notes, "alpha\nbeta\ngamma\ndelta\n").unwrap();
    let root = root.canonicalize().unwrap();
    let notes = root.join("ws/notes.txt");
    let sessions = root.join("sessions");
    if record {
        cedian_fake_omp::arm_record(&sessions, &cedian_omp_path()).unwrap();
    } else {
        cedian_fake_omp::install_replay(&sessions, Path::new(REVERT_FIXTURE)).unwrap();
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
    let text = || std::fs::read_to_string(&notes).unwrap();
    wait_for("cedian shell");

    send("edit notes.txt 2-2 \"make this line uppercase\"");
    let out = wait_for("(turn done)");
    assert_eq!(text(), "alpha\nBETA\ngamma\ndelta\n", "inline edit:\n{out}");
    assert!(out.contains("(turn 1 recorded"), "{out}");
    assert!(!out.contains("warning:"), "edit stayed in range:\n{out}");

    send(
        "prompt In notes.txt change line 1 'alpha' to 'ALPHA' and line 4 'delta' to 'DELTA' \
         with your edit tool. Change nothing else. Reply with only: t2-done",
    );
    let out = wait_for("(turn done)");
    assert_eq!(text(), "ALPHA\nBETA\ngamma\nDELTA\n", "prompt turn:\n{out}");
    assert!(out.contains("(turn 2 recorded"), "{out}");

    // The user rewrites line 4 after the agent.
    std::fs::write(&notes, "ALPHA\nBETA\ngamma\nDELTA (mine)\n").unwrap();

    send("revert-turn 2");
    let out = wait_for("revert-turn 3` redoes");
    assert!(
        out.contains("STALE notes.txt"),
        "user line reported STALE:\n{out}"
    );
    assert_eq!(text(), "alpha\nBETA\ngamma\nDELTA (mine)\n", "{out}");

    send("revert-turn 3");
    wait_for("revert-turn 4` redoes");
    assert_eq!(text(), "ALPHA\nBETA\ngamma\nDELTA (mine)\n", "redo");

    send("revert-turn 1");
    wait_for("revert-turn 5` redoes");
    assert_eq!(
        text(),
        "ALPHA\nbeta\ngamma\nDELTA (mine)\n",
        "inline edit reverted"
    );

    send("turns");
    let out = wait_for("[revert of 1]");
    assert!(out.contains("1 [edit] edit notes.txt:2-2"), "{out}");

    send("quit");
    wait_for("cedian shell closed");
    assert!(shell.wait().unwrap().success());
    if record {
        std::fs::copy(
            sessions.join(cedian_fake_omp::RECORDED_FILE),
            REVERT_FIXTURE,
        )
        .unwrap();
    }
}

const S2_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/s2_blocked.jsonl"
);
const BUG_FIX_SKILL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/skills/bug-fix/SKILL.md"
);

/// S2 exit (d): OMP runs the bug-fix playbook skill (the workspace carries
/// its own copy in `.omp/skills/`; cedian never writes `.omp/`, §77). It
/// reproduces, fixes, but can't verify (`check.sh` needs `bash`, which
/// headless denies), claims done anyway → after the turn the workflow is
/// `blocked` and the missing `verify` gate is printed.
fn s2_blocked_scenario(record: bool) {
    let root: PathBuf = std::env::temp_dir().join(format!("cedian-s2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let skill_dir = root.join("ws/.omp/skills/bug-fix"); // pre-canonical: setup only
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::copy(BUG_FIX_SKILL, skill_dir.join("SKILL.md")).unwrap();
    std::fs::write(root.join("ws/notes.txt"), ORIGINAL).unwrap();
    std::fs::write(
        root.join("ws/check.sh"),
        "#!/bin/sh\n# passes when line 2 of notes.txt is BETA\n[ \"$(sed -n 2p notes.txt)\" = BETA ]\n",
    )
    .unwrap();
    let root = root.canonicalize().unwrap();
    let sessions = root.join("sessions");
    if record {
        cedian_fake_omp::arm_record(&sessions, &cedian_omp_path()).unwrap();
    } else {
        cedian_fake_omp::install_replay(&sessions, Path::new(S2_FIXTURE)).unwrap();
    }

    let out = cedian(
        &root,
        &[
            "prompt",
            "Bug: `sh check.sh` fails because line 2 of notes.txt is wrong. Fix it. \
             Follow the bug-fix skill in .omp/skills/bug-fix/SKILL.md exactly.",
        ],
    );
    if record {
        std::fs::copy(sessions.join(cedian_fake_omp::RECORDED_FILE), S2_FIXTURE).unwrap();
    }

    let raw = std::fs::read_to_string(root.join("ws/.cedian/workflow.json"))
        .unwrap_or_else(|e| panic!("the skill started a workflow ({e}):\n{out}"));
    let state: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(state["task"]["kind"], "bug_fix", "{raw}");
    assert_eq!(
        state["status"], "blocked",
        "claimed done with verify unmet:\n{raw}\n{out}"
    );
    assert!(
        out.contains("workflow BLOCKED") && out.contains("required gate \"verify\""),
        "missing gate printed after the turn:\n{out}"
    );
    assert_eq!(
        state["last_completion"]["accepted"], false,
        "the turn called cedian_complete:\n{raw}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("ws/notes.txt")).unwrap(),
        "alpha\nBETA\ngamma\n",
        "the fix landed; only verification is missing"
    );
    let attributed_repro = state["evidence"]
        .as_object()
        .unwrap()
        .values()
        .any(|e| e["for_gates"][0] == "reproduce" && e["provenance"]["attributed"].is_object());
    assert!(
        attributed_repro,
        "reproduction bound to a real call:\n{raw}"
    );
    let fixture = std::fs::read_to_string(S2_FIXTURE).unwrap();
    assert!(
        fixture.contains("bug-fix/SKILL.md") || fixture.contains("skill://bug-fix"),
        "OMP read the skill"
    );
    // cedian wrote nothing under .omp/ besides the test's own copy.
    let omp: Vec<_> = walk(&root.join("ws/.omp"));
    assert_eq!(
        omp,
        [root.join("ws/.omp/skills/bug-fix/SKILL.md")],
        "{omp:?}"
    );
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out.sort();
    out
}
