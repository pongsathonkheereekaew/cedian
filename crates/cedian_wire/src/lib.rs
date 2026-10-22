//! What the window and the daemon say to each other over the daemon's Unix socket: one JSON
//! object per line, requests one way and notices the other.

mod message;
mod socket;
mod status;

pub use cedian_review::{FileReview, Hunk, HunkState, Review};
pub use message::{Decision, Notice, Request, read_message, write_message};
pub use socket::{socket_path, state_dir};
pub use status::{Status, ThreadInfo};
