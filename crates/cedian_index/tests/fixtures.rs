//! Reads the threads and sessions cedia recorded with its fixtures.

use cedian_index::{list_threads, read_thread, session_lines, session_path};
use cedian_protocol::{Line, ThreadState, fixtures_dir};

#[test]
fn edit_and_check_thread_folds() {
    let dir = fixtures_dir().join("edit-and-check");
    let threads = list_threads(&dir.join("threads")).expect("threads");
    assert_eq!(threads.len(), 1);
    let thread = &threads[0];
    assert_eq!(thread.title, "make f.txt say good");
    assert_eq!(thread.state, ThreadState::Done);
    assert_eq!(thread.sessions.len(), 1);
    assert!(
        thread
            .result
            .as_deref()
            .is_some_and(|r| r.contains("passes"))
    );
}

#[test]
fn a_turn_names_its_trees_and_every_session_it_took() {
    let dir = fixtures_dir().join("handoff").join("threads");
    let thread = read_thread(&dir, "t-00000001").expect("thread");
    let turn = thread.last_turn.expect("the handoff run changed files");
    assert_eq!(turn.cwd, "/fixture/repo");
    assert_ne!(turn.before, turn.after);
    assert_eq!(turn.sessions.len(), 2);
    assert_eq!(
        turn.sessions,
        thread
            .sessions
            .iter()
            .map(|s| s.session_id.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn subthread_has_a_parent() {
    let dir = fixtures_dir().join("subthread").join("threads");
    let threads = list_threads(&dir).expect("threads");
    assert!(threads.iter().any(|t| t.parent.is_some()), "{threads:#?}");
}

#[test]
fn session_logs_read_as_stream_lines() {
    let dir = fixtures_dir().join("handoff");
    for thread in list_threads(&dir.join("threads")).expect("threads") {
        let thread = read_thread(&dir.join("threads"), &thread.id).expect("thread");
        for session in &thread.sessions {
            // The recorded paths point at the recording machine; the fixture keeps the logs
            // under sessions/ by id.
            let path = session_path(&dir, "unused", &session.session_id, session.path.as_deref());
            let path = if path.exists() {
                path
            } else {
                dir.join("sessions")
                    .join(format!("{}.jsonl", session.session_id))
            };
            let lines = session_lines(&path).expect("session");
            assert!(!lines.is_empty());
            for line in lines {
                assert!(matches!(Line::parse(&line), Ok(Line::Event(_))), "{line}");
            }
        }
    }
}

#[test]
fn a_missing_directory_has_no_threads() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(
        list_threads(&dir.path().join("nope"))
            .expect("empty")
            .is_empty()
    );
}

#[test]
fn a_thread_rebuilt_from_its_logs_ends_with_its_receipt() {
    let dir = fixtures_dir().join("edit-and-check");
    let thread = &list_threads(&dir.join("threads")).expect("threads")[0];
    let mut transcript = cedian_agent::Transcript::default();
    for session in &thread.sessions {
        let path = dir
            .join("sessions")
            .join(format!("{}.jsonl", session.session_id));
        for line in session_lines(&path).expect("session") {
            transcript.apply(&Line::parse(&line).expect("line"));
        }
    }
    assert!(
        matches!(transcript.blocks().last(), Some(block @ cedian_agent::Block::Result { .. }) if block.verified()),
        "{:#?}",
        transcript.blocks().last()
    );
}

#[test]
fn a_session_names_its_snapshot_trees_in_order() {
    let dir = fixtures_dir().join("edit-and-check").join("sessions");
    let log = std::fs::read_dir(&dir)
        .expect("sessions")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|e| e == "jsonl"))
        .expect("a session");
    assert_eq!(
        cedian_index::session_trees(&log).expect("trees"),
        [
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904",
            "5f443e10ae5374c7c54f6caa2d73aa39e2872d6f",
            "99f8c32e405deb9aaa1ecb9d26171f9c8936cfcf"
        ]
    );
}

#[test]
fn a_thread_that_asked_holds_its_question() {
    let dir = fixtures_dir().join("ask").join("threads");
    let thread = list_threads(&dir).expect("threads").remove(0);
    assert_eq!(thread.state, ThreadState::Asking);
    let question = thread.question.expect("question");
    assert!(
        question
            .question
            .starts_with("Should f.txt say good or great?")
    );
    assert_eq!(question.options, ["good", "great"]);
}

#[test]
fn the_latest_rating_wins() {
    let line = |rating: &str, at: &str| {
        serde_json::from_str::<cedian_protocol::ThreadEvent>(&format!(
            r#"{{"kind":"rate","rating":"{rating}","at":"{at}"}}"#
        ))
        .expect("rate event")
    };
    let thread = cedian_index::fold(
        "t-1",
        &[
            line("down", "2026-10-22T03:00:00.000Z"),
            line("up", "2026-10-22T03:00:01.000Z"),
        ],
    );
    assert_eq!(thread.rating, Some(cedian_protocol::Rating::Up));
}
