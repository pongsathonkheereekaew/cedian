//! Reads cedia's files under `~/.cedia`: which project a directory belongs to, the project's
//! threads, and a thread's session logs. cedia writes these files; cedian only reads them.

mod home;
mod project;
mod session;
mod thread;

pub use home::cedia_home;
pub use project::{project_key, project_root, project_slug};
pub use session::{session_lines, session_path};
pub use thread::{
    SessionRef, ThreadMessage, ThreadSummary, fold, list_threads, read_thread, threads_dir,
};
