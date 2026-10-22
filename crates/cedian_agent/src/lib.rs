//! The agent pane's model: cedia's stream folded into the blocks a thread shows.

mod block;
mod file_checks;
mod transcript;

pub use block::{Block, DiffLine, Permission, ToolCall, ToolState};
pub use file_checks::FileChecks;
pub use transcript::Transcript;
