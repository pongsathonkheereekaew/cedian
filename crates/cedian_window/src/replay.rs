//! `cedian --replay <stream.jsonl>` plays a recorded cedia run into the agent pane, one line
//! at a time, the way a live run arrives.

use std::{path::Path, time::Duration};

use anyhow::Context as _;
use cedian_protocol::Line;
use gpui::{App, Entity};

use crate::agent_pane::AgentPane;

/// Time between lines, slow enough to watch text stream in.
const LINE_DELAY: Duration = Duration::from_millis(40);

pub fn read(path: &Path) -> anyhow::Result<Vec<Line>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    text.lines()
        .enumerate()
        .map(|(n, line)| {
            Line::parse(line).with_context(|| format!("{} line {}", path.display(), n + 1))
        })
        .collect()
}

pub fn play(lines: Vec<Line>, pane: Entity<AgentPane>, cx: &mut App) {
    cx.spawn(async move |cx| {
        for line in lines {
            cx.background_executor().timer(LINE_DELAY).await;
            pane.update(cx, |pane, cx| pane.apply(&line, cx));
        }
    })
    .detach();
}
