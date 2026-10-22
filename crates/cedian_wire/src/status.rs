use cedian_index::ThreadSummary;
use cedian_protocol::ThreadState;
use serde::{Deserialize, Serialize};

/// What a thread's tab shows: cedia's state, corrected by whether a run is alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The project's coordinator opened it and its first run has not started yet.
    Queued,
    /// A run of this thread is in progress.
    Working,
    Done,
    Failed,
    /// Stopped by the person.
    Stopped,
    /// cedia's last word was `working` but no run is alive: it crashed or the Mac restarted.
    /// The thread can go on with `--thread`.
    Interrupted,
    /// The run ended on a question for the person; their answer goes on with the thread.
    Asking,
}

impl Status {
    pub fn of(state: ThreadState, running: bool) -> Self {
        match (running, state) {
            (true, _) => Self::Working,
            (false, ThreadState::Queued) => Self::Queued,
            (false, ThreadState::Working) => Self::Interrupted,
            (false, ThreadState::Done) => Self::Done,
            (false, ThreadState::Failed) => Self::Failed,
            (false, ThreadState::Stopped) => Self::Stopped,
            (false, ThreadState::Asking) => Self::Asking,
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
    /// It finished, failed, was interrupted or asks something since the person last saw it:
    /// it counts in the badges until a window opens it.
    #[serde(default)]
    pub needs_you: bool,
}
