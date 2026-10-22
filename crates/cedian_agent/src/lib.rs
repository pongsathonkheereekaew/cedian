//! The agent pane's model: cedia's stream folded into the blocks a thread shows.

mod block;
mod transcript;

pub use block::{Block, DiffLine, Permission, ToolCall, ToolState};
pub use transcript::Transcript;
