//! Review of a thread's changes: the hunks between the thread's first snapshot and its last,
//! read from cedia's shadow git, and what each hunk looks like on disk now.

mod hunk;
mod patch;
mod review;
mod shadow;

pub use hunk::{Hunk, HunkState};
pub use patch::{FileDiff, parse_patch};
pub use review::{Decisions, FileReview, Review, reject, review};
pub use shadow::Shadow;
