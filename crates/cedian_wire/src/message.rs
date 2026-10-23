use std::{
    io::{BufRead, Write},
    path::PathBuf,
};

use cedian_index::{MemoryNote, Output};
use cedian_protocol::{
    CommandEntry, ModelList, PermissionList, Rating, Restored, RewindResult, Schedule, UsageReport,
    VersionInfo,
};
use cedian_review::Review;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{BrowserPage, Build, McpServer, Status, ThreadInfo};

/// From the window to the daemon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// The project's threads, answered with `Notice::Threads`, and again whenever one changes.
    Threads { cwd: PathBuf },
    /// A thread's history so far, then its lines as they arrive.
    Open { cwd: PathBuf, thread: String },
    /// No more lines for this thread.
    Close { thread: String },
    /// One cedia run. With no thread it opens a new one, and the client is sent `Started` and
    /// then the run's lines.
    Prompt {
        cwd: PathBuf,
        text: String,
        thread: Option<String>,
        /// For a new thread: run it in a new git worktree of the project.
        #[serde(default)]
        worktree: bool,
        /// For a new thread: the cedia profile its runs take (`coordinator` for the project
        /// chat).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
        /// The model this run's writer takes (`cedia -m`); none keeps cedia's choice.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        /// What the run may do (`cedia --permission`): a mode from `Notice::Choices`; none
        /// keeps cedia's default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permission: Option<String>,
        /// How hard the writer starts thinking (`cedia --effort`): a level the model lists;
        /// none keeps the model's own.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effort: Option<String>,
        #[serde(default, skip_serializing_if = "Helpers::is_empty")]
        helpers: Helpers,
    },
    /// The `/` commands a run in the project can take (`cedia commands`), answered with
    /// `Notice::Commands` when this cedia lists them.
    Commands { cwd: PathBuf },
    /// Where the spend went since `since` (`7d`, `30d` or a day, `YYYY-MM-DD`), in the project
    /// at `cwd` or, with none, every project (`cedia usage --json --since`), answered with
    /// `Notice::Usage` when this cedia offers `usage`.
    Usage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<PathBuf>,
        since: String,
    },
    /// Puts the files of `thread`'s last turn that changed files back (`cedia threads undo`),
    /// or forward again with `redo`; `force` also moves a file changed since. The client is
    /// sent `Restored`, when this cedia offers `threads.restore-json`.
    Restore {
        cwd: PathBuf,
        thread: String,
        #[serde(default)]
        redo: bool,
        #[serde(default)]
        force: bool,
    },
    /// Takes `thread` back to just before its run `n`, from 1, conversation and files
    /// together (`cedia threads rewind`); `force` goes past a refusal that allows it. The
    /// client is sent `Rewound`.
    Rewind {
        cwd: PathBuf,
        thread: String,
        n: u32,
        #[serde(default)]
        force: bool,
    },
    /// Edit and resend: a new thread whose conversation is `thread`'s just before `run`
    /// (`cedia threads fork`), then `text` as its next run. The client is sent `Forked`, then
    /// the run's lines as for an open thread.
    Fork {
        cwd: PathBuf,
        thread: String,
        run: String,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permission: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effort: Option<String>,
        #[serde(default, skip_serializing_if = "Helpers::is_empty")]
        helpers: Helpers,
    },
    /// A message the running thread sees at its next turn.
    Send {
        cwd: PathBuf,
        thread: String,
        text: String,
    },
    /// Ends the thread's run.
    Stop { thread: String },
    /// Takes back messages the thread has not seen yet, through `cedia threads unsend`.
    Unsend {
        cwd: PathBuf,
        thread: String,
        ids: Vec<String>,
    },
    /// The person's 👍 or 👎 on the thread, through `cedia threads rate`.
    Rate {
        cwd: PathBuf,
        thread: String,
        rating: Rating,
    },
    /// Hides the thread from the list, or brings it back, through `cedia threads
    /// archive|unarchive`.
    Archive {
        cwd: PathBuf,
        thread: String,
        archived: bool,
    },
    /// The project's memory (`~/.cedia/projects/<slug>/memory.jsonl`), answered with
    /// `Notice::Memory`.
    Memory { cwd: PathBuf },
    /// The files the project's threads changed, answered with `Notice::Outputs`.
    Outputs { cwd: PathBuf },
    /// The thread's changes since its first snapshot, answered with `Notice::Review`.
    Review { cwd: PathBuf, thread: String },
    /// A decision on one hunk of the thread's review, answered with the review again.
    Decide {
        cwd: PathBuf,
        thread: String,
        hunk: String,
        decision: Decision,
    },
    /// What the Background tasks block lists beside the threads: the project's scheduled runs
    /// and the MCP servers cedia's settings name. Answered with `Notice::Background`, and again
    /// whenever the schedules change.
    Background { cwd: PathBuf },
    /// Pauses or resumes one scheduled run (`cedia schedules pause|resume`); every window
    /// showing the project's background gets `Notice::Background`.
    Schedule {
        cwd: PathBuf,
        id: String,
        enabled: bool,
    },
    /// The project's page in the shared Chromium, which the daemon starts when none runs.
    /// Answered with `Notice::Browser`; from then on the project's runs get the page.
    Browser { cwd: PathBuf },
    /// The `desktop` key of cedia's settings file, answered with `Notice::Settings`.
    Settings,
    /// Sets one key of `desktop`; `null` removes it. Every window gets `Notice::Settings`.
    SetDesktop {
        key: String,
        value: serde_json::Value,
    },
}

/// The roles a run adds around its writer, each cedia's flag: a model ref, `claude-code[:<model>]`
/// or `none`; none keeps cedia's setting (`roles` in its settings).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Helpers {
    /// `--plan`: a model plans before the writer starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// `--review`: a model reviews the change once the done gate passes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<String>,
    /// `--advisor`: a model reads each writer turn and may leave a note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisor: Option<String>,
    /// `--goal`: the run goes on until a judge finds this holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
}

impl Helpers {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// What the person decided about a hunk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum Decision {
    Accept,
    /// Takes back an accept.
    Undo,
    /// Puts the old lines back in the thread's worktree and tells the thread why.
    Reject {
        reason: String,
    },
}

/// From the daemon to the window.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Notice {
    /// The first notice on every connection.
    Welcome {
        daemon: String,
        /// The cedia build the daemon runs, or why it could not be asked.
        cedia: Result<VersionInfo, String>,
        /// The daemon's own build; none from a daemon older than this field.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        build: Option<Build>,
    },
    Threads {
        cwd: PathBuf,
        threads: Vec<ThreadInfo>,
    },
    /// A step before a new thread's run starts, such as creating its worktree.
    Preparing {
        cwd: PathBuf,
        step: String,
    },
    /// A prompt with no thread got one.
    Started {
        cwd: PathBuf,
        thread: String,
    },
    /// One of cedia's stream-json lines, as cedia wrote it. History replays session logs in
    /// the same shape.
    Line {
        thread: String,
        line: String,
    },
    /// The thread's history is all sent; what follows is live.
    Caught {
        thread: String,
    },
    Status {
        thread: String,
        status: Status,
    },
    Review {
        thread: String,
        review: Review,
    },
    /// The project's memory notes, oldest first.
    Memory {
        cwd: PathBuf,
        notes: Vec<MemoryNote>,
    },
    /// The files the project's threads changed, the most recent first.
    Outputs {
        cwd: PathBuf,
        outputs: Vec<Output>,
    },
    Settings {
        path: PathBuf,
        desktop: serde_json::Value,
    },
    /// What a run can be given, when this cedia lists it: the models (`cedia models`) and the
    /// permission modes (`cedia permissions`). Sent after `Welcome`.
    Choices {
        models: Option<ModelList>,
        permissions: Option<PermissionList>,
    },
    /// `thread` was forked from `from` and its run is starting.
    Forked {
        cwd: PathBuf,
        from: String,
        thread: String,
    },
    /// The project's `/` commands: its skills and the person's.
    Commands {
        cwd: PathBuf,
        commands: Vec<CommandEntry>,
    },
    /// The project's scheduled runs (`cedia schedules`), `None` when this cedia has none, and
    /// the MCP servers in cedia's settings.
    Background {
        cwd: PathBuf,
        schedules: Option<Vec<Schedule>>,
        mcp: Vec<McpServer>,
    },
    /// What `Request::Restore` moved, and what it left.
    Restored {
        cwd: PathBuf,
        thread: String,
        result: Restored,
    },
    /// What `Request::Rewind` did, or why it did not.
    Rewound {
        cwd: PathBuf,
        thread: String,
        n: u32,
        result: RewindResult,
    },
    /// The answer to `Request::Usage`, with the request's `cwd` and `since`.
    Usage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<PathBuf>,
        since: String,
        report: Box<UsageReport>,
    },
    /// The answer to `Request::Browser`: the project's page, or why there is none (no
    /// Chromium found, or it did not start).
    Browser {
        cwd: PathBuf,
        page: Result<BrowserPage, String>,
    },
    Error {
        message: String,
    },
}

/// Writes one message and its newline, then flushes.
pub fn write_message<T: Serialize>(out: &mut impl Write, message: &T) -> anyhow::Result<()> {
    let mut line = serde_json::to_vec(message)?;
    line.push(b'\n');
    out.write_all(&line)?;
    out.flush()?;
    Ok(())
}

/// The next message, or `None` at the end of the stream.
pub fn read_message<T: DeserializeOwned>(input: &mut impl BufRead) -> anyhow::Result<Option<T>> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&line)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let request = Request::Prompt {
            cwd: "/repo".into(),
            text: "fix it".into(),
            thread: None,
            worktree: false,
            profile: None,
            model: None,
            permission: None,
            effort: None,
            helpers: Helpers::default(),
        };
        let mut buffer = Vec::new();
        write_message(&mut buffer, &request).expect("write");
        assert_eq!(
            String::from_utf8_lossy(&buffer),
            "{\"op\":\"prompt\",\"cwd\":\"/repo\",\"text\":\"fix it\",\"thread\":null,\"worktree\":false}\n"
        );
        let back: Request = read_message(&mut buffer.as_slice())
            .expect("read")
            .expect("one");
        assert_eq!(back, request);
    }
}
