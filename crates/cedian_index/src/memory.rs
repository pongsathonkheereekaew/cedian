//! A project's memory folded from its append-only log, as cedia folds it (`MemoryStore.notes`
//! in cedia's `src/core/project/memory.ts`): short facts every new session of the project gets
//! in its first turn, kept by the person (`cedia memory add`) or a writer session (the
//! `remember` tool). Forgetting a note is an event in the same log.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemoryNote {
    pub id: String,
    pub text: String,
    /// `user` for the person's notes, else the session that kept it.
    pub from: String,
    pub at: String,
}

pub fn memory_path(home: &Path, slug: &str) -> PathBuf {
    home.join("projects").join(slug).join("memory.jsonl")
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MemoryEvent {
    Note {
        id: String,
        text: String,
        from: String,
        at: String,
    },
    Forget {
        id: String,
    },
}

/// The notes still kept, oldest first. A project with no log has none; a line cut by a crash,
/// edited by hand or written by another version is skipped, as cedia skips it.
pub fn read_memory(path: &Path) -> anyhow::Result<Vec<MemoryNote>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", path.display()));
        }
    };
    Ok(fold_memory(&text))
}

fn fold_memory(text: &str) -> Vec<MemoryNote> {
    let events: Vec<MemoryEvent> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let gone: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            MemoryEvent::Forget { id } => Some(id.as_str()),
            MemoryEvent::Note { .. } => None,
        })
        .collect();
    events
        .iter()
        .filter_map(|e| match e {
            MemoryEvent::Note { id, text, from, at } if !gone.contains(&id.as_str()) => {
                Some(MemoryNote {
                    id: id.clone(),
                    text: text.clone(),
                    from: from.clone(),
                    at: at.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forgotten_and_broken_lines_leave_the_notes() {
        let log = concat!(
            r#"{"kind":"note","id":"a1","text":"bun test runs the suite","from":"user","at":"2026-10-21T10:00:00Z"}"#,
            "\n",
            r#"{"kind":"note","id":"b2","text":"old","from":"s-1","at":"2026-10-21T11:00:00Z"}"#,
            "\n",
            r#"{"kind":"forget","id":"b2","at":"2026-10-21T12:00:00Z"}"#,
            "\n",
            r#"{"kind":"note","id":"c3","text":"cut sho"#,
            "\n",
            r#"{"kind":"note","id":"d4","text":"the daemon owns state","from":"s-2","at":"2026-10-22T09:00:00Z"}"#,
            "\n",
        );
        let notes = fold_memory(log);
        let ids: Vec<&str> = notes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["a1", "d4"]);
        assert_eq!(notes[0].from, "user");
    }

    #[test]
    fn a_project_with_no_log_has_no_notes() {
        let path = std::env::temp_dir().join("cedian-no-memory/memory.jsonl");
        assert_eq!(read_memory(&path).expect("read"), []);
    }
}
