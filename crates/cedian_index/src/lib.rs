//! Reads cedia's files under `~/.cedia`: which project a directory belongs to, the project's
//! threads, and a thread's session logs. cedia writes these files; cedian only reads them.

mod home;
mod project;
mod session;
mod thread;

pub use home::cedia_home;
pub use project::{project_key, project_root, project_slug};
pub use session::{session_cwd, session_lines, session_path, session_trees};
pub use thread::{
    SessionRef, ThreadMessage, ThreadQuestion, ThreadSummary, ThreadTurn, fold, list_threads,
    read_thread, threads_dir,
};
