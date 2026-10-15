//! The bundled agent keeps cedian's own storage (ADR-0078 decision 5): started
//! the way the app starts it, with a temp HOME holding a sentinel `~/.omp`,
//! it answers `get_state` and starts a session, and afterwards nothing under
//! `~/.omp` was created, changed or removed, while its logs and session-owner
//! lease sit under cedian's agent state dir. No model turn: the only provider
//! is a dead local port, and no prompt is sent. Ignored because it needs
//! `script/cedian-omp-build` first:
//! `cargo test -p cedian_shell --test agent_storage -- --ignored --nocapture`
#![allow(
    clippy::disallowed_methods,
    reason = "headless, synchronous process control of OMP: Zed's async spawn helpers do not apply"
)]

use std::collections::BTreeMap;
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

const MODELS: &str = "providers:\n  dead:\n    baseUrl: http://127.0.0.1:9/v1\n    auth: none\n    api: openai-completions\n    models:\n      - id: none\n";
const CONFIG: &str = "modelRoles:\n  default: dead/none\n";

/// Every path under `root` with its mtime (directories included).
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<SystemTime>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            if meta.is_dir() {
                stack.push(path.clone());
            }
            out.insert(path, meta.modified().ok());
        }
    }
    out
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    snapshot(root).into_keys().filter(|p| p.is_file()).collect()
}

#[test]
#[ignore = "needs script/cedian-omp-build"]
fn the_bundled_agent_never_touches_the_persons_omp_dir() {
    let root = std::env::temp_dir().join(format!("cedian-agent-storage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let home = root.join("home");
    let omp = home.join(".omp");
    for dir in [
        omp.join("agent/skills/sentinel"),
        home.join(".config/cedian/agent"),
        root.join("ws"),
    ] {
        std::fs::create_dir_all(dir).unwrap();
    }
    // The person's own OMP setup, which cedian must leave alone.
    std::fs::write(
        omp.join("agent/skills/sentinel/SKILL.md"),
        "---\nname: sentinel\n---\n",
    )
    .unwrap();
    for agent in [omp.join("agent"), home.join(".config/cedian/agent")] {
        std::fs::write(agent.join("models.yml"), MODELS).unwrap();
        std::fs::write(agent.join("config.yml"), CONFIG).unwrap();
    }
    unsafe {
        // SAFETY: the only test in this binary.
        for name in [
            "CEDIAN_OMP_BINARY",
            "CEDIAN_STATE_DIR",
            "PI_CODING_AGENT_DIR",
            "PI_STATE_DIR",
            "PI_RUNTIME_DIR",
            "OMP_PROFILE",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "XDG_CACHE_HOME",
        ] {
            std::env::remove_var(name);
        }
        std::env::set_var("HOME", &home);
    }
    let before = snapshot(&omp);
    let home_before = snapshot(&home);

    let choice = cedian_shell::launch::omp_binary().expect("the stamped build");
    let state = home.join(".local/state/cedian/agent");
    let plan = cedian_omp::SpawnProfile {
        binary_path: choice.binary,
        session_dir: home.join(".local/state/cedian/session"),
        sessions: cedian_omp::Sessions::InSessionDir,
        cwd: root.join("ws"),
        policy: cedian_omp::SpawnPolicy::default(),
    }
    .prepare()
    .unwrap();
    println!("env: {:?}", plan.env);
    let mut child = Command::new(&plan.argv[0])
        .args(&plan.argv[1..])
        .env_clear()
        .envs(plan.env)
        .current_dir(root.join("ws"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for line in [
        r#"{"id":"1","type":"get_state"}"#,
        r#"{"id":"2","type":"new_session"}"#,
        r#"{"id":"3","type":"get_state"}"#,
    ] {
        writeln!(stdin, "{line}").unwrap();
    }
    let mut answers = BTreeMap::new();
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line.unwrap();
        let Ok(frame) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if frame["type"] == "response" {
            println!("response: {}", &line[..line.len().min(300)]);
            answers.insert(frame["id"].as_str().unwrap_or("").to_string(), frame);
            if answers.len() == 3 {
                break;
            }
        }
    }
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();

    let after = snapshot(&omp);
    let cedian_dirs = [
        home.join(".config/cedian"),
        home.join(".local/state/cedian"),
    ];
    let home_after = snapshot(&home);
    let elsewhere: Vec<&PathBuf> = home_before
        .keys()
        .chain(home_after.keys())
        .filter(|p| {
            !cedian_dirs
                .iter()
                .any(|d| p.starts_with(d) || d.starts_with(p))
        })
        .filter(|p| !p.starts_with(&omp))
        .filter(|p| home_before.get(*p) != home_after.get(*p))
        .collect();
    let written = files_under(&home.join(".local/state/cedian"));
    let changed: Vec<&PathBuf> = before
        .keys()
        .chain(after.keys())
        .filter(|p| before.get(*p) != after.get(*p))
        .collect();
    println!("under cedian state: {written:#?}");
    for id in ["1", "2", "3"] {
        assert_eq!(answers[id]["success"], true, "{}", answers[id]);
    }
    assert!(
        changed.is_empty(),
        "the agent changed the person's ~/.omp: {changed:#?}"
    );
    assert!(
        elsewhere.is_empty(),
        "the agent wrote under HOME outside cedian's dirs: {elsewhere:#?}"
    );
    assert!(
        written.iter().any(|p| p.starts_with(state.join("logs"))),
        "no agent log under {}",
        state.display()
    );
    assert!(
        state.join("run/session-owners").is_dir(),
        "no session-owner lease dir under {}",
        state.display()
    );
    let _ = std::fs::remove_dir_all(&root);
}
