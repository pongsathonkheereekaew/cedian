//! A session log as stream-json lines, so the window folds a thread's history with the same
//! code as a live run.

use std::path::{Path, PathBuf};

use anyhow::Context as _;

/// Where a session's log lives. A thread names the path it wrote; when that path is gone (the
/// home moved) the log is looked for under this home.
pub fn session_path(home: &Path, slug: &str, session_id: &str, named: Option<&str>) -> PathBuf {
    match named.map(PathBuf::from) {
        Some(path) if path.exists() => path,
        _ => home
            .join("sessions")
            .join(slug)
            .join(format!("{session_id}.jsonl")),
    }
}

/// Each event of the log, wrapped as the `{"type":"event","event":…}` line a run streams. A
/// last line cut short by a write in progress is left out.
pub fn session_lines(path: &Path) -> anyhow::Result<Vec<String>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut lines = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(event) => {
                lines.push(serde_json::json!({"type": "event", "event": event}).to_string())
            }
            Err(error) => log::warn!("{} line {}: {error}", path.display(), n + 1),
        }
    }
    Ok(lines)
}
