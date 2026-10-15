//! Workloads shaped like a long cedian session, fed through the production
//! `Thread::apply` so the benchmarks see the same events the panel holds.

use cedian_agent::Thread;
use cedian_omp::{DeltaKind, RouterEvent};

const WORDS: &[&str] = &[
    "the",
    "panel",
    "renders",
    "every",
    "tool",
    "card",
    "after",
    "workspace",
    "buffer",
    "review",
    "agent",
    "session",
    "history",
    "scroll",
    "frame",
    "layout",
    "message",
    "stream",
    "thread",
    "cedian",
    "omp",
    "edit",
    "file",
    "test",
    "passes",
    "fails",
    "because",
    "of",
    "a",
    "stale",
];

fn sentence(seed: usize, words: usize) -> String {
    let mut text = String::new();
    for index in 0..words {
        if index > 0 {
            text.push(' ');
        }
        text.push_str(WORDS[(seed * 31 + index * 7) % WORDS.len()]);
    }
    text.push('.');
    text
}

fn paragraph(seed: usize, sentences: usize) -> String {
    (0..sentences)
        .map(|index| sentence(seed + index, 12 + (seed + index) % 9))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Text deltas of one assistant reply, a few words each, like OMP streams them.
pub fn reply_deltas(seed: usize) -> Vec<String> {
    let reply = format!(
        "{}\n\n{}\n\n{}",
        paragraph(seed, 4),
        paragraph(seed + 1, 3),
        paragraph(seed + 2, 2)
    );
    reply
        .split_inclusive(' ')
        .collect::<Vec<_>>()
        .chunks(3)
        .map(|chunk| chunk.concat())
        .collect()
}

/// Feed one finished turn: prompt, thinking, a streamed reply, two tool calls.
pub fn push_turn(thread: &mut Thread, turn: usize) {
    thread.push_user(&sentence(turn, 16));
    let message_id = format!("message-{turn}");
    thread.apply(&RouterEvent::MessageDelta {
        message_id: message_id.clone(),
        kind: DeltaKind::Thinking,
        delta: paragraph(turn + 3, 2),
    });
    for delta in reply_deltas(turn) {
        thread.apply(&RouterEvent::MessageDelta {
            message_id: message_id.clone(),
            kind: DeltaKind::Text,
            delta,
        });
    }
    thread.apply(&RouterEvent::MessageEnd { message_id });
    for tool in 0..2 {
        let tool_call_id = format!("call-{turn}-{tool}");
        thread.apply(&RouterEvent::ToolStart {
            tool_call_id: tool_call_id.clone(),
            tool_name: if tool == 0 { "read" } else { "edit" }.to_string(),
            args_preview: format!("crates/cedian_panel/src/panel.rs:{}", turn * 10 + tool),
            paths: Vec::new(),
        });
        thread.apply(&RouterEvent::ToolEnd {
            tool_call_id,
            tool_name: if tool == 0 { "read" } else { "edit" }.to_string(),
            result_summary: sentence(turn + tool, 6),
            is_error: false,
            before: Vec::new(),
        });
    }
    thread.apply(&RouterEvent::AgentEnd {
        yielded: false,
        is_terminal: true,
    });
}

pub fn thread(turns: usize) -> Thread {
    let mut thread = Thread::new();
    for turn in 0..turns {
        push_turn(&mut thread, turn);
    }
    thread
}

/// Older messages as `sessions::message_line` flattens them, one per message.
pub fn history_lines(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| match index % 3 {
            0 => format!("user: {}", sentence(index, 14)),
            1 => format!("assistant: {}", paragraph(index, 3)),
            _ => format!(
                "bash: $ cargo test -p cedian_panel → {}",
                sentence(index, 5)
            ),
        })
        .collect()
}
