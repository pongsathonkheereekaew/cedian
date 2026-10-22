use cedian_index::ThreadSummary;
use cedian_protocol::ThreadState;
use serde::{Deserialize, Serialize};

/// What a thread's tab shows: cedia's state, corrected by whether a run is alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// A run of this thread is in progress.
    Working,
    Done,
    Failed,
    /// Stopped by the person.
    Stopped,
    /// cedia's last word was `working` but no run is alive: it crashed or the Mac restarted.
    /// The thread can go on with `--thread`.
    Interrupted,
}

impl Status {
    pub fn of(state: ThreadState, running: bool) -> Self {
        match (running, state) {
            (true, _) => Self::Working,
            (false, ThreadState::Working) => Self::Interrupted,
            (false, ThreadState::Done) => Self::Done,
            (false, ThreadState::Failed) => Self::Failed,
            (false, ThreadState::Stopped) => Self::Stopped,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadInfo {
    #[serde(flatten)]
    pub thread: ThreadSummary,
    pub status: Status,
    /// Where the thread's latest session ran: its worktree, when it has one.
    #[serde(default)]
    pub cwd: Option<std::path::PathBuf>,
}
