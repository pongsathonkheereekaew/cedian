//! Archiving a thread once nothing is owed on it (Interface, Phase C: auto-archive): it is
//! done, its latest receipt is `verified`, and no hunk of its review waits. A thread that
//! failed, was interrupted, is unverified, asks, or has hunks waiting stays where the person
//! sees it. Only top-level threads: cedia archives its own sub-threads. The daemon checks when
//! a run ends and when a review decision leaves nothing waiting, and archives through
//! `cedia threads archive`, so cedia's files stay the record.

use std::path::Path;

use cedian_index::{ThreadSummary, session_lines, session_path};
use cedian_protocol::{HeadlessResult, Line, ResultSubtype, SessionEvent};
use cedian_review::{HunkState, Review};
use cedian_wire::Status;

/// Whether a thread in this state may be archived without asking anyone.
pub fn nothing_owed(
    summary: &ThreadSummary,
    status: Status,
    verified: bool,
    waiting: usize,
) -> bool {
    status == Status::Done
        && !summary.archived
        && summary.parent.is_none()
        // The project chat is the coordinator's thread, which stays.
        && summary.profile.as_deref() != Some("coordinator")
        && summary.pending.is_empty()
        && verified
        && waiting == 0
}

/// Whether the thread's latest receipt (the `result` event its last session logged) is
/// verified: a success with no unverified claim and no flag.
pub fn verified(home: &Path, slug: &str, summary: &ThreadSummary) -> bool {
    let Some(session) = summary.sessions.last() else {
        return false;
    };
    let path = session_path(home, slug, &session.session_id, session.path.as_deref());
    let lines = match session_lines(&path) {
        Ok(lines) => lines,
        Err(error) => {
            log::warn!("reading {}: {error:#}", path.display());
            return false;
        }
    };
    lines
        .iter()
        .rev()
        .find_map(|text| match Line::parse(text) {
            Ok(Line::Event(SessionEvent::Result { result, .. })) => Some(is_verified(&result)),
            _ => None,
        })
        .unwrap_or(false)
}

fn is_verified(result: &HeadlessResult) -> bool {
    result.subtype == ResultSubtype::Success
        && result
            .evidence
            .as_ref()
            .is_some_and(|e| e.unverified.is_empty() && e.flags.is_empty())
}

/// The hunks of a review still waiting for the person.
pub fn waiting(review: &Review) -> usize {
    review
        .files
        .iter()
        .flat_map(|file| &file.hunks)
        .filter(|hunk| hunk.state == HunkState::Pending)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(parent: Option<&str>, archived: bool) -> ThreadSummary {
        serde_json::from_value(serde_json::json!({
            "id": "t-1", "title": "x", "task": "x", "state": "done", "result": null,
            "parent": parent, "archived": archived, "sessions": [], "messages": [],
            "pending": [], "updated_at": "2026-10-22T03:00:00.000Z"
        }))
        .expect("summary")
    }

    #[test]
    fn only_a_verified_done_thread_with_nothing_waiting_goes() {
        let top = summary(None, false);
        assert!(nothing_owed(&top, Status::Done, true, 0));
        assert!(!nothing_owed(&top, Status::Done, false, 0), "unverified");
        assert!(!nothing_owed(&top, Status::Done, true, 2), "hunks waiting");
        for status in [
            Status::Working,
            Status::Failed,
            Status::Interrupted,
            Status::Asking,
            Status::Stopped,
        ] {
            assert!(!nothing_owed(&top, status, true, 0), "{status:?}");
        }
        assert!(
            !nothing_owed(&summary(Some("t-0"), false), Status::Done, true, 0),
            "sub-thread"
        );
        assert!(
            !nothing_owed(&summary(None, true), Status::Done, true, 0),
            "already archived"
        );
    }
}
