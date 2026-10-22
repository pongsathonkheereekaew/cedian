//! A thread folded from its append-only file, as cedia folds it (`foldThread` in cedia's
//! `src/core/project/threads.ts`).

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use cedian_protocol::{ThreadEvent, ThreadState};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub title: String,
    pub task: String,
    /// cedia's last state. A run that died leaves `working`.
    pub state: ThreadState,
    pub result: Option<String>,
    /// The thread that spawned this one as a sub-thread.
    pub parent: Option<String>,
    pub archived: bool,
    pub sessions: Vec<SessionRef>,
    pub messages: Vec<ThreadMessage>,
    /// Messages no session has been shown yet.
    pub pending: Vec<String>,
    /// What the thread asked the person, while it waits in `asking` for the answer.
    #[serde(default)]
    pub question: Option<ThreadQuestion>,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadQuestion {
    pub question: String,
    /// Two to four choices, the recommended one first; none for a free answer.
    pub options: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionRef {
    pub session_id: String,
    pub model: String,
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadMessage {
    pub id: String,
    pub from: String,
    pub text: String,
    pub at: String,
}

pub fn threads_dir(home: &Path, slug: &str) -> PathBuf {
    home.join("projects").join(slug).join("threads")
}

pub fn fold(id: &str, events: &[ThreadEvent]) -> ThreadSummary {
    let mut thread = ThreadSummary {
        id: id.to_owned(),
        title: id.to_owned(),
        task: String::new(),
        state: ThreadState::Working,
        result: None,
        parent: None,
        archived: false,
        sessions: Vec::new(),
        messages: Vec::new(),
        pending: Vec::new(),
        question: None,
        updated_at: String::new(),
    };
    let mut delivered = std::collections::HashSet::new();
    let mut opened = false;
    for event in events {
        match event {
            ThreadEvent::Open {
                title,
                task,
                parent,
                at,
            } if !opened => {
                opened = true;
                thread.title = title.clone();
                thread.task = task.clone();
                thread.parent = parent.clone();
                thread.updated_at = at.clone();
            }
            ThreadEvent::Open { at, .. } => thread.updated_at = at.clone(),
            ThreadEvent::Session {
                session_id,
                model,
                path,
                at,
                ..
            } => {
                thread.sessions.push(SessionRef {
                    session_id: session_id.clone(),
                    model: model.clone(),
                    path: path.clone(),
                });
                thread.updated_at = at.clone();
            }
            ThreadEvent::Message { id, from, text, at } => {
                thread.messages.push(ThreadMessage {
                    id: id.clone(),
                    from: from.clone(),
                    text: text.clone(),
                    at: at.clone(),
                });
                thread.updated_at = at.clone();
            }
            ThreadEvent::Delivered { ids, at, .. } => {
                delivered.extend(ids.iter().cloned());
                thread.updated_at = at.clone();
            }
            ThreadEvent::Unsend { ids, at } => {
                thread.messages.retain(|m| !ids.contains(&m.id));
                thread.updated_at = at.clone();
            }
            ThreadEvent::Question {
                question,
                options,
                at,
                ..
            } => {
                thread.question = Some(ThreadQuestion {
                    question: question.clone(),
                    options: options.clone(),
                });
                thread.updated_at = at.clone();
            }
            ThreadEvent::State { state, result, at } => {
                thread.state = *state;
                if *state != ThreadState::Asking {
                    thread.question = None;
                }
                if result.is_some() {
                    thread.result = result.clone();
                }
                thread.updated_at = at.clone();
            }
            ThreadEvent::Archived { archived, at } => {
                thread.archived = *archived;
                thread.updated_at = at.clone();
            }
            ThreadEvent::Handoff { at, .. } | ThreadEvent::Rate { at, .. } => {
                thread.updated_at = at.clone();
            }
        }
    }
    thread.pending = thread
        .messages
        .iter()
        .filter(|m| !delivered.contains(&m.id))
        .map(|m| m.id.clone())
        .collect();
    thread
}

/// One thread. A line that does not decode (the last one may be mid-write) is skipped.
pub fn read_thread(dir: &Path, id: &str) -> anyhow::Result<ThreadSummary> {
    let path = dir.join(format!("{id}.jsonl"));
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut events = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<ThreadEvent>(line) {
            Ok(event) => events.push(event),
            Err(error) => log::warn!("{} line {}: {error}", path.display(), n + 1),
        }
    }
    Ok(fold(id, &events))
}

/// The project's threads, most recently changed first. No directory means no threads yet.
pub fn list_threads(dir: &Path) -> anyhow::Result<Vec<ThreadSummary>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("listing {}", dir.display())),
    };
    let mut threads = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let Some(id) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".jsonl"))
        else {
            continue;
        };
        match read_thread(dir, id) {
            Ok(thread) => threads.push(thread),
            Err(error) => log::warn!("{error:#}"),
        }
    }
    threads.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(threads)
}
