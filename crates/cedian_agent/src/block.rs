//! What the agent pane draws, one block per thing the person reads.

use cedian_protocol::{EffectStatus, Evidence, ImageRef, ResultSubtype};

#[derive(Clone, Debug)]
pub enum Block {
    /// A session began: the thread's first, or one after a handoff.
    Session {
        model: String,
        role: String,
    },
    User {
        text: String,
        /// The images shown to the writer with the text (`--image`), stored by cedia in
        /// `~/.cedia/blobs/<sha256>`.
        images: Vec<ImageRef>,
    },
    /// The model's answer text. `streaming` until the turn's assistant event lands.
    Text {
        text: String,
        streaming: bool,
    },
    Thinking {
        text: String,
        streaming: bool,
        /// How long the model thought, from the assistant event once it lands.
        ms: Option<f64>,
    },
    Tool(ToolCall),
    /// The run's receipt.
    Result {
        subtype: ResultSubtype,
        text: String,
        cost_usd: f64,
        turns: u32,
        /// How long the run took, from its timing.
        wall_ms: Option<f64>,
        evidence: Option<Evidence>,
    },
}

impl Block {
    /// True when a result's evidence leaves nothing unverified or flagged.
    pub fn verified(&self) -> bool {
        match self {
            Block::Result {
                subtype: ResultSubtype::Success,
                evidence: Some(evidence),
                ..
            } => evidence.unverified.is_empty() && evidence.flags.is_empty(),
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub input: serde_json::Map<String, serde_json::Value>,
    pub permission: Option<Permission>,
    pub state: ToolState,
    pub output: Option<String>,
    pub is_error: bool,
    pub exit_code: Option<f64>,
    /// When the tool started, from its intent event, until the result gives `ms`.
    pub started_at: Option<String>,
    /// From the start to the result.
    pub ms: Option<i64>,
}

impl ToolCall {
    /// It ran and failed, was refused, or never finished.
    pub fn failed(&self) -> bool {
        self.is_error
            || matches!(self.state, ToolState::Done(status) if status != EffectStatus::Completed)
            || self.permission.as_ref().is_some_and(|p| !p.allowed)
    }

    /// What an `edit` or `write` call changes, read from its input: the old lines and the
    /// new. `None` for any other tool.
    pub fn diff(&self) -> Option<Vec<DiffLine>> {
        let text =
            |value: Option<&serde_json::Value>| value.and_then(|v| v.as_str()).map(str::to_owned);
        let mut lines = Vec::new();
        let mut hunk = |old: &str, new: &str| {
            lines.extend(old.lines().map(|l| DiffLine::Removed(l.to_owned())));
            lines.extend(new.lines().map(|l| DiffLine::Added(l.to_owned())));
        };
        match self.name.as_str() {
            "write" => hunk("", &text(self.input.get("content"))?),
            "edit" => match self.input.get("edits") {
                Some(serde_json::Value::Array(edits)) => {
                    for edit in edits {
                        let old = text(edit.get("old_text")).unwrap_or_default();
                        let new = text(edit.get("new_text")).unwrap_or_default();
                        hunk(&old, &new);
                    }
                }
                // The shape models send by mistake, which cedia takes too.
                _ => hunk(
                    &text(self.input.get("old_text"))?,
                    &text(self.input.get("new_text"))?,
                ),
            },
            _ => return None,
        }
        Some(lines)
    }

    /// Lines added and removed, as `+n −n` shows them.
    pub fn counts(&self) -> Option<(usize, usize)> {
        let diff = self.diff()?;
        let added = diff
            .iter()
            .filter(|l| matches!(l, DiffLine::Added(_)))
            .count();
        Some((added, diff.len() - added))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiffLine {
    Removed(String),
    Added(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Permission {
    pub allowed: bool,
    pub rule: String,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolState {
    /// The model asked for it; no decision yet.
    Requested,
    Running,
    Done(EffectStatus),
}
