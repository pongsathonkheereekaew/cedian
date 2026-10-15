//! The app's own OMP answers in rpc mode, started the way the app starts it:
//! [`cedian_shell::launch::omp_binary`] (the stamped `target/omp/omp`), then
//! the spawn profile's argv and scrubbed env. No model turn: the only
//! provider is a dead local port, and no prompt is sent. Ignored because it
//! needs `script/cedian-omp-build` first:
//! `cargo test -p cedian_shell --test bundled_omp_rpc -- --ignored --nocapture`
#![allow(
    clippy::disallowed_methods,
    reason = "headless, synchronous process control of OMP: Zed's async spawn helpers do not apply"
)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Command, Stdio};

#[test]
#[ignore = "needs script/cedian-omp-build"]
fn the_bundled_fork_build_answers_get_state_and_set_host_debug() {
    let root = std::env::temp_dir().join(format!("cedian-bundled-omp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let agent = root.join("agent");
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::create_dir_all(root.join("ws")).unwrap();
    std::fs::write(
        agent.join("models.yml"),
        "providers:\n  dead:\n    baseUrl: http://127.0.0.1:9/v1\n    auth: none\n    api: openai-completions\n    models:\n      - id: none\n",
    )
    .unwrap();
    std::fs::write(
        agent.join("config.yml"),
        "modelRoles:\n  default: dead/none\n",
    )
    .unwrap();
    unsafe {
        // SAFETY: the only test in this binary.
        std::env::remove_var("CEDIAN_OMP_BINARY");
        std::env::set_var("HOME", root.join("home"));
        std::env::set_var("PI_CODING_AGENT_DIR", &agent);
    }

    let choice = cedian_shell::launch::omp_binary().expect("the stamped build");
    println!("binary: {}", choice.binary.display());
    let plan = cedian_omp::SpawnProfile {
        binary_path: choice.binary,
        session_dir: root.join("session"),
        sessions: cedian_omp::Sessions::InSessionDir,
        cwd: root.join("ws"),
        policy: cedian_omp::SpawnPolicy::default(),
    }
    .prepare()
    .unwrap();
    println!("argv: {:?}", plan.argv);
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
        r#"{"id":"2","type":"set_host_debug","enabled":true}"#,
        r#"{"id":"3","type":"set_host_debug","enabled":false}"#,
    ] {
        writeln!(stdin, "{line}").unwrap();
    }
    let mut answers = std::collections::BTreeMap::new();
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line.unwrap();
        let frame: serde_json::Value = match serde_json::from_str(&line) {
            Ok(frame) => frame,
            Err(_) => continue,
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
    assert_eq!(answers["1"]["command"], "get_state");
    assert_eq!(answers["1"]["success"], true, "{}", answers["1"]);
    assert_eq!(answers["2"]["success"], true, "{}", answers["2"]);
    assert_eq!(answers["2"]["data"]["enabled"], true);
    assert_eq!(answers["3"]["data"]["enabled"], false);
    let _ = std::fs::remove_dir_all(&root);
}
