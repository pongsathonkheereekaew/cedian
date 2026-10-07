//! `cedian shell` (ADR-0021 / P4): one long-lived, foreground headless
//! process per workspace. Holds ONE OMP runtime across turns, so a turn can
//! be steered or aborted while it streams. Holds `.cedian/shell.lock`, so
//! mutating one-shot commands refuse while it runs. Dies with its terminal.
//!
//! Verbs: `prompt <msg>`, `steer <msg>`, `abort`, `help`, `quit`; any other
//! line runs the one-shot command of the same name in-process (`review`,
//! `accept <path> <hunk>`, `workflow status`, …).

use crate::shell_lock::ShellLock;
use cedian_omp::{OmpRuntime, RuntimeControl};
use cedian_workspace::HostTools;
use std::{
    io::BufRead as _,
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    thread::JoinHandle,
    time::Duration,
};

const HELP: &str = "prompt <msg> | steer <msg> | abort | review | accept <path> <hunk> | \
reject <path> <hunk> | accept-all | <any cedian verb> | help | quit";

type TurnResult = (OmpRuntime, Result<(), String>);

struct Running {
    turn: JoinHandle<TurnResult>,
    control: RuntimeControl,
}

/// Split a shell line into words; `"double quotes"` keep spaces.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The rest of the line after the verb, verbatim (prompt/steer text).
fn rest(line: &str) -> &str {
    let line = line.trim_start();
    line.split_once(char::is_whitespace)
        .map(|(_, r)| r.trim())
        .unwrap_or("")
}

pub fn run(session_dir: &Path, workdir: &Path) -> Result<(), String> {
    let _lock = ShellLock::acquire(workdir)?;
    let host = HostTools::shared(workdir);
    let mut rt = crate::spawn(session_dir, workdir, &host)?;
    rt.open_session("shell").map_err(|e| e.to_string())?;
    println!(
        "cedian shell — {} (pid {}). {HELP}",
        workdir.display(),
        std::process::id()
    );

    let (lines_tx, lines) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if lines_tx.send(line).is_err() {
                break;
            }
        }
    });

    let mut idle: Option<OmpRuntime> = Some(rt);
    let mut running: Option<Running> = None;
    loop {
        if running.as_ref().is_some_and(|r| r.turn.is_finished()) {
            idle = Some(finish(running.take())?);
        }
        let line = match lines.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break, // EOF
        };
        let args = words(&line);
        let Some(verb) = args.first().map(String::as_str) else {
            continue;
        };
        match (verb, &running) {
            ("quit" | "exit", _) => break,
            ("help", _) => println!("{HELP}"),
            ("steer", Some(r)) => report(r.control.steer(rest(&line)).map(|()| "steered")),
            ("abort", Some(r)) => report(r.control.abort().map(|()| "abort sent")),
            ("steer" | "abort", None) => println!("no turn running"),
            (_, Some(_)) => println!("turn running — only steer, abort, help, quit"),
            ("prompt", None) => {
                let message = rest(&line).to_string();
                if message.is_empty() {
                    println!("usage: prompt <message>");
                    continue;
                }
                let Some(rt) = idle.take() else {
                    return Err("runtime lost".to_string());
                };
                running = Some(start_turn(rt, &host, workdir.to_path_buf(), message));
            }
            (_, None) => {
                if let Err(e) = crate::dispatch(args, true) {
                    println!("error: {e}");
                }
            }
        }
    }

    // Leaving: abort a running turn, then shut the runtime down cleanly.
    if let Some(r) = &running {
        let _ = r.control.abort();
    }
    if running.is_some() {
        idle = Some(finish(running.take())?);
    }
    if let Some(rt) = idle {
        rt.shutdown().map_err(|e| e.to_string())?;
    }
    println!("cedian shell closed");
    Ok(())
}

fn start_turn(
    mut rt: OmpRuntime,
    host: &Arc<HostTools>,
    workdir: PathBuf,
    message: String,
) -> Running {
    let control = rt.control();
    let host = Arc::clone(host);
    let turn = std::thread::spawn(move || {
        let result = crate::run_turn(
            &mut rt,
            &host,
            &workdir,
            &message,
            true,
            crate::session::TurnKind::Prompt,
            &message,
        );
        (rt, result)
    });
    Running { turn, control }
}

/// Join a finished (or aborted) turn and hand the runtime back.
fn finish(running: Option<Running>) -> Result<OmpRuntime, String> {
    let Running { turn, control } = running.ok_or("no turn")?;
    drop(control); // shutdown needs the only client handle
    let (rt, result) = turn
        .join()
        .map_err(|_| "turn thread panicked".to_string())?;
    match result {
        Ok(()) => println!("(turn done)"),
        Err(e) => println!("(turn failed: {e})"),
    }
    Ok(rt)
}

fn report(result: Result<&str, cedian_omp::OmpError>) {
    match result {
        Ok(msg) => println!("({msg})"),
        Err(e) => println!("error: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_rest() {
        assert_eq!(
            words(r#"accept /a b.txt 0"#),
            ["accept", "/a", "b.txt", "0"]
        );
        assert_eq!(
            words(r#"workflow run bug_fix "fix login""#),
            ["workflow", "run", "bug_fix", "fix login"]
        );
        assert_eq!(rest("prompt  change \"x\" to y "), "change \"x\" to y");
        assert_eq!(rest("abort"), "");
    }
}
