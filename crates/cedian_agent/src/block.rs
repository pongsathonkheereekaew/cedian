//! What the agent pane draws, one block per thing the person reads.

use cedian_protocol::{EffectStatus, Evidence, ResultSubtype};

#[derive(Clone, Debug)]
pub enum Block {
    /// A session began: the thread's first, or one after a handoff.
    Session {
        model: String,
        role: String,
    },
    User {
        text: String,
    },
    /// The model's answer text. `streaming` until the turn's assistant event lands.
    Text {
        text: String,
        streaming: bool,
    },
    Thinking {
        text: String,
        streaming: bool,
    },
    Tool(ToolCall),
    /// The run's receipt.
    Result {
        subtype: ResultSubtype,
        text: String,
        cost_usd: f64,
        turns: u32,
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
