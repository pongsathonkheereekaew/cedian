//! What the project's threads produced: every file a writer session changed, read from the
//! `changed` paths of its sessions' `snapshot` events, across the project's threads. A path is
//! relative to the directory its session ran in (the thread's worktree), so each output keeps
//! that directory.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::{ThreadSummary, list_threads, session_cwd, session_path, threads_dir};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Output {
    /// Where the session that changed it ran.
    pub cwd: PathBuf,
    /// Relative to `cwd`.
    pub path: String,
    /// The thread that changed it last, and its title.
    pub thread: String,
    pub title: String,
    /// When it was changed last.
    pub at: String,
    /// It is still there: a later change, or the worktree's removal, may have taken it away.
    pub exists: bool,
}

/// Each file the project's threads changed, the most recently changed first, once.
pub fn project_outputs(home: &Path, slug: &str) -> anyhow::Result<Vec<Output>> {
    let mut outputs: Vec<Output> = Vec::new();
    for thread in list_threads(&threads_dir(home, slug))? {
        for output in thread_outputs(home, slug, &thread) {
            match outputs
                .iter_mut()
                .find(|o| o.cwd == output.cwd && o.path == output.path)
            {
                Some(seen) if seen.at >= output.at => {}
                Some(seen) => *seen = output,
                None => outputs.push(output),
            }
        }
    }
    outputs.sort_by(|a, b| b.at.cmp(&a.at));
    Ok(outputs)
}

fn thread_outputs(home: &Path, slug: &str, thread: &ThreadSummary) -> Vec<Output> {
    let mut outputs = Vec::new();
    for session in &thread.sessions {
        let path = session_path(home, slug, &session.session_id, session.path.as_deref());
        let Some(cwd) = session_cwd(&path) else {
            continue;
        };
        let changed = match session_changes(&path) {
            Ok(changed) => changed,
            Err(error) => {
                log::warn!("{error:#}");
                continue;
            }
        };
        for (file, at) in changed {
            outputs.push(Output {
                exists: cwd.join(&file).exists(),
                cwd: cwd.clone(),
                path: file,
                thread: thread.id.clone(),
                title: thread.title.clone(),
                at,
            });
        }
    }
    outputs
}

/// The paths the session's snapshots name as changed, each with when it was changed last.
pub fn session_changes(path: &Path) -> anyhow::Result<Vec<(String, String)>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut changes: Vec<(String, String)> = Vec::new();
    let snapshots = text
        .lines()
        .filter(|line| line.contains("\"snapshot\""))
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|event| event.get("kind").and_then(|k| k.as_str()) == Some("snapshot"));
    for event in snapshots {
        let at = event.get("at").and_then(|a| a.as_str()).unwrap_or_default();
        let files = event.get("changed").and_then(|c| c.as_array());
        for file in files.into_iter().flatten().filter_map(|f| f.as_str()) {
            match changes.iter_mut().find(|(f, _)| f == file) {
                Some(seen) => seen.1 = at.to_owned(),
                None => changes.push((file.to_owned(), at.to_owned())),
            }
        }
    }
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../cedian_protocol/cedia/sessions"
    );

    #[test]
    fn a_session_names_each_file_it_changed_once() {
        let path = Path::new(FIXTURE)
            .join("edit-and-check/sessions/e35da0ce-e57e-4cab-94cf-441034dca24d.jsonl");
        let changes = session_changes(&path).expect("read");
        assert_eq!(
            changes,
            [("f.txt".to_owned(), "2026-10-22T09:42:57.225Z".to_owned())]
        );
    }

    #[test]
    fn the_project_lists_its_threads_files_newest_first() {
        let home = tempfile::tempdir().expect("home");
        let work = tempfile::tempdir().expect("work");
        let slug = "fixture-repo";
        let threads = threads_dir(home.path(), slug);
        let sessions = home.path().join("sessions").join(slug);
        std::fs::create_dir_all(&threads).expect("threads");
        std::fs::create_dir_all(&sessions).expect("sessions");
        std::fs::write(work.path().join("a.txt"), "a").expect("a");
        let cwd = work.path().display().to_string();
        let session = |id: &str, files: &[(&str, &str)]| {
            let mut log = serde_json::json!({"kind": "session", "cwd": cwd, "at": "x"}).to_string();
            for (file, at) in files {
                log.push('\n');
                log.push_str(
                    &serde_json::json!({"kind": "snapshot", "tree": "t", "changed": [file], "at": at})
                        .to_string(),
                );
            }
            std::fs::write(sessions.join(format!("{id}.jsonl")), log).expect("session");
        };
        session(
            "s1",
            &[
                ("a.txt", "2026-10-22T01:00:00Z"),
                ("gone.txt", "2026-10-22T02:00:00Z"),
            ],
        );
        session("s2", &[("a.txt", "2026-10-22T03:00:00Z")]);
        let thread = |id: &str, title: &str, session: &str| {
            let open = serde_json::json!({"kind": "open", "at": "2026-10-22T00:00:00Z", "title": title, "task": title});
            let joined = serde_json::json!({"kind": "session", "at": "2026-10-22T00:00:01Z", "sessionId": session, "model": "faux/scripted", "role": "code", "why": "start"});
            std::fs::write(
                threads.join(format!("{id}.jsonl")),
                format!("{open}\n{joined}\n"),
            )
            .expect("thread");
        };
        thread("t1", "first", "s1");
        thread("t2", "second", "s2");
        let outputs = project_outputs(home.path(), slug).expect("outputs");
        let shown: Vec<(&str, &str, bool)> = outputs
            .iter()
            .map(|o| (o.path.as_str(), o.title.as_str(), o.exists))
            .collect();
        assert_eq!(
            shown,
            [("a.txt", "second", true), ("gone.txt", "first", false)]
        );
    }
}
