//! Replays cedia's recorded fixtures (at the pin in `cedian_protocol/cedia/REV`) through the
//! protocol types and the transcript, and compares each outline with `tests/replay/<name>.txt`.
//! `CEDIAN_RECORD=1 cargo test -p cedian_agent --test replay` re-records the outlines.

use std::{fs, path::Path};

use cedian_agent::{Block, ToolState, Transcript};
use cedian_protocol::{Line, SessionEvent, ThreadEvent, fixtures_dir};

fn fixture_names() -> Vec<String> {
    let index = fs::read_to_string(fixtures_dir().join("index.json")).expect("index.json");
    let entries: Vec<serde_json::Value> = serde_json::from_str(&index).expect("index is JSON");
    entries
        .iter()
        .map(|entry| entry["name"].as_str().expect("named fixture").to_owned())
        .collect()
}

fn jsonl_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .expect("fixture directory")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort();
    files
}

fn outline(transcript: &Transcript) -> String {
    let mut out = String::new();
    for block in transcript.blocks() {
        let line = match block {
            Block::Session { model, role } => format!("session {role} on {model}"),
            Block::User { text } => format!("user: {text}"),
            Block::Text { text, streaming } => {
                format!(
                    "text{}: {text}",
                    if *streaming { " (streaming)" } else { "" }
                )
            }
            Block::Thinking { text, .. } => format!("thinking: {text}"),
            Block::Tool(call) => {
                let permission = match &call.permission {
                    Some(p) if p.allowed => format!(" allowed by {}", p.rule),
                    Some(p) => format!(" denied by {}", p.rule),
                    None => String::new(),
                };
                let state = match call.state {
                    ToolState::Requested => "requested".to_owned(),
                    ToolState::Running => "running".to_owned(),
                    ToolState::Done(status) => format!("{status}"),
                };
                let output = call
                    .output
                    .as_deref()
                    .unwrap_or("")
                    .lines()
                    .next()
                    .unwrap_or("");
                let counts = call
                    .counts()
                    .map(|(added, removed)| format!(" +{added} -{removed}"))
                    .unwrap_or_default();
                let exit = call
                    .exit_code
                    .map(|code| format!(" exit {code}"))
                    .unwrap_or_default();
                format!(
                    "tool {} [{}]{counts}{permission}: {state}{exit}: {output}",
                    call.name, call.call_id
                )
            }
            Block::Result {
                subtype,
                text,
                turns,
                evidence,
                ..
            } => {
                let mut line = format!(
                    "result {subtype} after {turns} turns{}: {text}",
                    if block.verified() { ", verified" } else { "" }
                );
                for check in evidence.iter().flat_map(|e| &e.checks) {
                    let thread = check.thread.as_deref().unwrap_or("-");
                    line.push_str(&format!(
                        "\n  check `{}` exit {} in {thread}",
                        check.command, check.exit_code
                    ));
                }
                line
            }
        };
        out.push_str(line.trim_end());
        out.push('\n');
    }
    for (thread, sub) in transcript.subthreads() {
        out.push_str(&format!("subthread {thread}:\n"));
        for line in outline(sub).lines() {
            out.push_str(format!("  {line}").trim_end());
            out.push('\n');
        }
    }
    out
}

#[test]
fn every_fixture_replays_to_its_outline() {
    let names = fixture_names();
    assert!(
        !names.is_empty(),
        "no fixtures at {}",
        fixtures_dir().display()
    );
    let record = std::env::var_os("CEDIAN_RECORD").is_some();
    let expected_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/replay");
    for name in names {
        let stream = fs::read_to_string(fixtures_dir().join(&name).join("stream.jsonl"))
            .expect("stream.jsonl");
        let mut transcript = Transcript::default();
        for (n, text) in stream.lines().enumerate() {
            let line =
                Line::parse(text).unwrap_or_else(|e| panic!("{name} stream line {}: {e}", n + 1));
            transcript.apply(&line);
        }
        let actual = outline(&transcript);
        let expected_path = expected_dir.join(format!("{name}.txt"));
        if record {
            fs::create_dir_all(&expected_dir).expect("replay dir");
            fs::write(&expected_path, &actual).expect("write outline");
            continue;
        }
        let expected = fs::read_to_string(&expected_path).unwrap_or_else(|_| {
            panic!(
                "missing {}; run with CEDIAN_RECORD=1",
                expected_path.display()
            )
        });
        assert_eq!(actual, expected, "{name} outline changed");
    }
}

#[test]
fn every_thread_and_session_file_decodes() {
    for name in fixture_names() {
        let dir = fixtures_dir().join(&name);
        for path in jsonl_files(&dir.join("threads")) {
            for (n, text) in fs::read_to_string(&path)
                .expect("thread file")
                .lines()
                .enumerate()
            {
                serde_json::from_str::<ThreadEvent>(text)
                    .unwrap_or_else(|e| panic!("{} line {}: {e}", path.display(), n + 1));
            }
        }
        for path in jsonl_files(&dir.join("sessions")) {
            for (n, text) in fs::read_to_string(&path)
                .expect("session file")
                .lines()
                .enumerate()
            {
                serde_json::from_str::<SessionEvent>(text)
                    .unwrap_or_else(|e| panic!("{} line {}: {e}", path.display(), n + 1));
            }
        }
    }
}

#[test]
fn streamed_text_shows_until_the_assistant_event_replaces_it() {
    let stream =
        fs::read_to_string(fixtures_dir().join("denied-call/stream.jsonl")).expect("stream.jsonl");
    let lines: Vec<Line> = stream
        .lines()
        .map(|text| Line::parse(text).expect("line"))
        .collect();
    let last_assistant = lines
        .iter()
        .rposition(|line| matches!(line, Line::Event(SessionEvent::Assistant { .. })))
        .expect("an assistant event");

    let mut transcript = Transcript::default();
    for line in &lines[..last_assistant] {
        transcript.apply(line);
    }
    let Some(Block::Text { text, streaming }) = transcript.blocks().last() else {
        panic!(
            "expected a streamed text block, got {:?}",
            transcript.blocks().last()
        );
    };
    assert!(*streaming);
    assert_eq!(
        text,
        "Publishing is on the deny list, so I left it for you."
    );
    let before = transcript.blocks().len();

    transcript.apply(&lines[last_assistant]);
    assert_eq!(transcript.blocks().len(), before);
    assert!(matches!(
        transcript.blocks().last(),
        Some(Block::Text {
            streaming: false,
            ..
        })
    ));
}

#[test]
fn a_check_covers_a_file_until_a_later_edit_outdates_it() {
    let stream = fs::read_to_string(fixtures_dir().join("edit-and-check/stream.jsonl"))
        .expect("stream.jsonl");
    let mut transcript = Transcript::default();
    let mut edit_lines = Vec::new();
    for text in stream.lines() {
        // The edit's request and result, to play again after the check.
        if text.starts_with(r#"{"type":"event""#) && text.contains(r#""e1""#) {
            edit_lines.push(text.replace(r#""e1""#, r#""e2""#));
        }
        transcript.apply(&Line::parse(text).expect("line"));
    }
    let covered = transcript.file_checks("f.txt");
    assert_eq!(covered.covering, ["grep -q good f.txt"]);
    assert!(covered.outdated.is_empty());
    assert_eq!(
        transcript.file_checks("other.txt").covering,
        ["grep -q good f.txt"]
    );

    assert!(!edit_lines.is_empty(), "the fixture's edit lines");
    for text in &edit_lines {
        transcript.apply(&Line::parse(text).expect("edit line"));
    }
    let outdated = transcript.file_checks("f.txt");
    assert_eq!(outdated.outdated, ["grep -q good f.txt"]);
    assert!(outdated.covering.is_empty());
}
