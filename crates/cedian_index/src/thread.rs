//! A thread folded from its append-only file, as cedia folds it (`foldThread` in cedia's
//! `src/core/project/threads.ts`).

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use cedian_protocol::{Rating, ThreadEvent, ThreadState, TodoState};
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
    /// The thread that opened this one with `open_thread` (the project's coordinator), for a
    /// client to run: it waits `queued` until its first run.
    #[serde(default)]
    pub by: Option<String>,
    /// The profile its runs take unless another is named.
    #[serde(default)]
    pub profile: Option<String>,
    /// The writer's latest list of steps, whole: the thread's progress.
    #[serde(default)]
    pub todo: Vec<Step>,
    pub archived: bool,
    pub sessions: Vec<SessionRef>,
    pub messages: Vec<ThreadMessage>,
    /// Messages no session has been shown yet.
    pub pending: Vec<String>,
    /// What the thread asked the person, while it waits in `asking` for the answer.
    #[serde(default)]
    pub question: Option<ThreadQuestion>,
    /// The person's latest 👍 or 👎 on the thread (`cedia threads rate`).
    #[serde(default)]
    pub rating: Option<Rating>,
    /// Sessions a rewind dropped (`cedia rewind`): their logs stay, but they are no longer the
    /// thread's conversation, so they are not in `sessions`.
    #[serde(default)]
    pub rewound: Vec<String>,
    /// The latest run that changed files: the shadow-git trees before its first write and
    /// after its last, what `cedia threads diff|undo|redo` call the thread's turn.
    #[serde(default)]
    pub last_turn: Option<ThreadTurn>,
    pub updated_at: String,
}

/// One step of a thread's todo list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub text: String,
    pub state: TodoState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadTurn {
    pub cwd: String,
    pub before: String,
    pub after: String,
    pub sessions: Vec<String>,
    pub at: String,
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
        by: None,
        profile: None,
        todo: Vec::new(),
        archived: false,
        sessions: Vec::new(),
        messages: Vec::new(),
        pending: Vec::new(),
        question: None,
        rating: None,
        rewound: Vec::new(),
        last_turn: None,
        updated_at: String::new(),
    };
    let mut delivered = std::collections::HashSet::new();
    let mut unsent = std::collections::HashSet::new();
    let mut opened = false;
    let (live, dropped) = rewound(events);
    for event in &dropped {
        if let ThreadEvent::Session { session_id, .. } = event
            && !thread.rewound.contains(session_id)
        {
            thread.rewound.push(session_id.clone());
        }
    }
    for event in live {
        match event {
            ThreadEvent::Open {
                title,
                task,
                parent,
                by,
                profile,
                at,
                ..
            } if !opened => {
                opened = true;
                thread.title = title.clone();
                thread.task = task.clone();
                thread.parent = parent.clone();
                thread.by = by.clone();
                thread.profile = profile.clone();
                if by.is_some() {
                    thread.state = ThreadState::Queued;
                }
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
            ThreadEvent::Message {
                id, from, text, at, ..
            } => {
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
            ThreadEvent::Unsend { ids, at, .. } => {
                unsent.extend(ids.iter().cloned());
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
            ThreadEvent::State {
                state, result, at, ..
            } => {
                thread.state = *state;
                if *state != ThreadState::Asking {
                    thread.question = None;
                }
                if result.is_some() {
                    thread.result = result.clone();
                }
                thread.updated_at = at.clone();
            }
            ThreadEvent::Archived { archived, at, .. } => {
                thread.archived = *archived;
                thread.updated_at = at.clone();
            }
            ThreadEvent::Rate { rating, at, .. } => {
                thread.rating = Some(*rating);
                thread.updated_at = at.clone();
            }
            ThreadEvent::Turn {
                cwd,
                before,
                after,
                sessions,
                at,
                ..
            } => {
                thread.last_turn = Some(ThreadTurn {
                    cwd: cwd.clone(),
                    before: before.clone(),
                    after: after.clone(),
                    sessions: sessions.clone(),
                    at: at.clone(),
                });
                thread.updated_at = at.clone();
            }
            ThreadEvent::Todo { items, at, .. } => {
                thread.todo = items
                    .iter()
                    .map(|item| Step {
                        text: item.text.clone(),
                        state: item.state,
                    })
                    .collect();
                thread.updated_at = at.clone();
            }
            ThreadEvent::Run { at, .. } => {
                if thread.state == ThreadState::Queued {
                    thread.state = ThreadState::Working;
                }
                thread.updated_at = at.clone();
            }
            ThreadEvent::Handoff { at, .. } | ThreadEvent::Rewind { at, .. } => {
                thread.updated_at = at.clone()
            }
        }
    }
    // The last line's time, as cedia has it, whether or not the fold reads that line.
    if let Some(at) = events.last().map(at_of) {
        thread.updated_at = at.to_owned();
    }
    // A message taken back stays when a session had already been shown it: the model saw it.
    thread
        .messages
        .retain(|m| !unsent.contains(&m.id) || delivered.contains(&m.id));
    thread.pending = thread
        .messages
        .iter()
        .filter(|m| !delivered.contains(&m.id))
        .map(|m| m.id.clone())
        .collect();
    thread
}

/// The events a fold reads after each rewind, and the ones it dropped, as cedia's `rewound`:
/// a rewind drops the events from the run it goes back to up to itself, except messages,
/// unsends, ratings and archiving, which are not the conversation's, and so does anything a
/// dropped run writes later. A rewind to a run that is not there changes nothing.
fn rewound(events: &[ThreadEvent]) -> (Vec<&ThreadEvent>, Vec<&ThreadEvent>) {
    let keep = |e: &ThreadEvent| {
        matches!(
            e,
            ThreadEvent::Message { .. }
                | ThreadEvent::Unsend { .. }
                | ThreadEvent::Rate { .. }
                | ThreadEvent::Archived { .. }
        )
    };
    let mut live: Vec<&ThreadEvent> = Vec::new();
    let mut dropped = Vec::new();
    let mut dropped_runs = std::collections::HashSet::new();
    for event in events {
        let ThreadEvent::Rewind { run, .. } = event else {
            if run_of(event).is_some_and(|r| dropped_runs.contains(r)) && !keep(event) {
                dropped.push(event);
            } else {
                live.push(event);
            }
            continue;
        };
        let Some(from) = live
            .iter()
            .position(|e| matches!(e, ThreadEvent::Run { id, .. } if id == run))
        else {
            continue;
        };
        let tail = live.split_off(from);
        for e in tail {
            if let ThreadEvent::Run { id, .. } = e {
                dropped_runs.insert(id.as_str());
            }
            if keep(e) {
                live.push(e);
            } else {
                dropped.push(e);
            }
        }
        live.push(event);
    }
    (live, dropped)
}

/// The run that wrote the event, when cedia tagged it.
fn run_of(event: &ThreadEvent) -> Option<&str> {
    match event {
        ThreadEvent::Open { run, .. }
        | ThreadEvent::Session { run, .. }
        | ThreadEvent::Handoff { run, .. }
        | ThreadEvent::Message { run, .. }
        | ThreadEvent::Delivered { run, .. }
        | ThreadEvent::Unsend { run, .. }
        | ThreadEvent::Rate { run, .. }
        | ThreadEvent::Question { run, .. }
        | ThreadEvent::State { run, .. }
        | ThreadEvent::Turn { run, .. }
        | ThreadEvent::Todo { run, .. }
        | ThreadEvent::Run { run, .. }
        | ThreadEvent::Archived { run, .. } => run.as_deref(),
        ThreadEvent::Rewind { .. } => None,
    }
}

fn at_of(event: &ThreadEvent) -> &str {
    match event {
        ThreadEvent::Open { at, .. }
        | ThreadEvent::Session { at, .. }
        | ThreadEvent::Handoff { at, .. }
        | ThreadEvent::Message { at, .. }
        | ThreadEvent::Delivered { at, .. }
        | ThreadEvent::Unsend { at, .. }
        | ThreadEvent::Rate { at, .. }
        | ThreadEvent::Question { at, .. }
        | ThreadEvent::State { at, .. }
        | ThreadEvent::Turn { at, .. }
        | ThreadEvent::Todo { at, .. }
        | ThreadEvent::Run { at, .. }
        | ThreadEvent::Rewind { at, .. }
        | ThreadEvent::Archived { at, .. } => at,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn events(lines: &[&str]) -> Vec<ThreadEvent> {
        lines
            .iter()
            .map(|l| serde_json::from_str(l).expect("event"))
            .collect()
    }

    #[test]
    fn a_rewind_drops_the_runs_it_goes_back_over_but_not_the_messages() {
        let events = events(&[
            r#"{"kind":"open","title":"t","task":"t","at":"1"}"#,
            r#"{"kind":"run","id":"r1","prompt":"a","cwd":"/r","at":"2"}"#,
            r#"{"kind":"session","sessionId":"s1","model":"m","role":"code","why":"start","at":"3","run":"r1"}"#,
            r#"{"kind":"state","state":"done","result":"one","at":"4","run":"r1"}"#,
            r#"{"kind":"run","id":"r2","prompt":"b","cwd":"/r","at":"5"}"#,
            r#"{"kind":"session","sessionId":"s2","model":"m","role":"code","why":"resume","at":"6","run":"r2"}"#,
            r#"{"kind":"message","id":"m1","from":"user","text":"hi","at":"7","run":"r2"}"#,
            r#"{"kind":"rewind","run":"r2","at":"8"}"#,
            // A dropped run that was still going writes its end after the rewind.
            r#"{"kind":"state","state":"failed","result":"two","at":"9","run":"r2"}"#,
        ]);
        let thread = fold("t-1", &events);
        assert_eq!(thread.state, ThreadState::Done);
        assert_eq!(thread.result.as_deref(), Some("one"));
        assert_eq!(
            thread
                .sessions
                .iter()
                .map(|s| s.session_id.as_str())
                .collect::<Vec<_>>(),
            ["s1"]
        );
        assert_eq!(thread.rewound, ["s2"]);
        assert_eq!(thread.messages.len(), 1);
        assert_eq!(thread.pending, ["m1"]);
        assert_eq!(thread.updated_at, "9");
        // A rewind to a run an earlier rewind dropped changes nothing.
        let mut again = events.clone();
        again.push(
            serde_json::from_str(r#"{"kind":"rewind","run":"r2","at":"10"}"#).expect("event"),
        );
        assert_eq!(fold("t-1", &again).sessions, thread.sessions);
    }

    #[test]
    fn an_opened_thread_waits_queued_until_its_first_run_and_keeps_its_todo() {
        let opened = [
            r#"{"kind":"open","title":"t","task":"t","by":"t-0","profile":"code","at":"1"}"#,
            r#"{"kind":"run","id":"r1","prompt":"t","cwd":"/r","at":"2"}"#,
            r#"{"kind":"todo","items":[{"text":"read","state":"done"},{"text":"fix","state":"doing"}],"at":"3"}"#,
        ];
        let queued = fold("t-1", &events(&opened[..1]));
        assert_eq!(queued.state, ThreadState::Queued);
        assert_eq!(queued.by.as_deref(), Some("t-0"));
        assert_eq!(queued.profile.as_deref(), Some("code"));
        let running = fold("t-1", &events(&opened));
        assert_eq!(running.state, ThreadState::Working);
        assert_eq!(
            running.todo,
            [
                Step {
                    text: "read".into(),
                    state: TodoState::Done
                },
                Step {
                    text: "fix".into(),
                    state: TodoState::Doing
                },
            ]
        );
        // A thread nobody opened for someone else starts working, as before.
        let own = fold(
            "t-2",
            &events(&[r#"{"kind":"open","title":"t","task":"t","at":"1"}"#]),
        );
        assert_eq!(own.state, ThreadState::Working);
    }

    #[test]
    fn a_message_taken_back_after_a_session_saw_it_stays() {
        let events = events(&[
            r#"{"kind":"open","title":"t","task":"t","at":"1"}"#,
            r#"{"kind":"message","id":"m1","from":"user","text":"a","at":"2"}"#,
            r#"{"kind":"message","id":"m2","from":"user","text":"b","at":"3"}"#,
            r#"{"kind":"delivered","ids":["m1"],"sessionId":"s1","at":"4"}"#,
            r#"{"kind":"unsend","ids":["m1","m2"],"at":"5"}"#,
        ]);
        let thread = fold("t-1", &events);
        assert_eq!(
            thread
                .messages
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            ["m1"]
        );
        assert!(thread.pending.is_empty());
    }
}
