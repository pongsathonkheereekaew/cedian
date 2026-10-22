//! Writes a long stream-json run for scroll and memory budgets: one recorded fixture's turn,
//! repeated until the transcript has at least the asked number of blocks.
//!
//! cargo run -p cedian_agent --example long_thread -- 10000 > /tmp/long.jsonl

use std::io::{BufWriter, Write as _};

use cedian_agent::Transcript;
use cedian_protocol::{Line, SessionEvent, fixtures_dir};

/// Events the agent pane draws; the rest (requests, reminders, streaming) only add bytes.
fn drawn(line: &Line) -> bool {
    matches!(
        line,
        Line::Event(
            SessionEvent::User { .. }
                | SessionEvent::Assistant { .. }
                | SessionEvent::Permission { .. }
                | SessionEvent::ToolResult { .. }
        )
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target: usize = std::env::args()
        .nth(1)
        .as_deref()
        .unwrap_or("10000")
        .parse()?;
    let text = std::fs::read_to_string(fixtures_dir().join("edit-and-check/stream.jsonl"))?;
    let lines: Vec<(&str, Line)> = text
        .lines()
        .map(|raw| Ok((raw, Line::parse(raw)?)))
        .collect::<Result<_, cedian_protocol::LineError>>()?;

    let (first, rest) = lines.split_first().ok_or("empty fixture")?;
    let (last, body) = rest.split_last().ok_or("fixture has one line")?;
    let body: Vec<_> = body.iter().filter(|(_, line)| drawn(line)).collect();

    let mut transcript = Transcript::default();
    let mut out = BufWriter::new(std::io::stdout().lock());
    writeln!(out, "{}", first.0)?;
    transcript.apply(&first.1);
    while transcript.blocks().len() < target {
        for (raw, line) in &body {
            writeln!(out, "{raw}")?;
            transcript.apply(line);
        }
    }
    writeln!(out, "{}", last.0)?;
    eprintln!("{} blocks", transcript.blocks().len() + 1);
    Ok(())
}
