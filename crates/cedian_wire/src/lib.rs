//! What the window and the daemon say to each other over the daemon's Unix socket: one JSON
//! object per line, requests one way and notices the other.

mod browser;
mod build;
mod launch_agent;
mod mcp;
mod message;
mod socket;
mod status;

pub use browser::BrowserPage;
pub use build::Build;
pub use cedian_review::{FileReview, Hunk, HunkState, Review};
pub use launch_agent::{LAUNCH_AGENT_LABEL, launch_agent_plist};
pub use mcp::{McpServer, McpTransport};
pub use message::{Decision, Helpers, Notice, Request, read_message, write_message};
pub use socket::{socket_path, state_dir};
pub use status::{Status, ThreadInfo};
