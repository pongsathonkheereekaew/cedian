use std::{
    io::{BufRead, Write},
    path::PathBuf,
};

use cedian_protocol::VersionInfo;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{Status, ThreadInfo};

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
    },
    /// A message the running thread sees at its next turn.
    Send {
        cwd: PathBuf,
        thread: String,
        text: String,
    },
    /// Ends the thread's run.
    Stop { thread: String },
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
    },
    Threads {
        cwd: PathBuf,
        threads: Vec<ThreadInfo>,
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
        };
        let mut buffer = Vec::new();
        write_message(&mut buffer, &request).expect("write");
        assert_eq!(
            String::from_utf8_lossy(&buffer),
            "{\"op\":\"prompt\",\"cwd\":\"/repo\",\"text\":\"fix it\",\"thread\":null}\n"
        );
        let back: Request = read_message(&mut buffer.as_slice())
            .expect("read")
            .expect("one");
        assert_eq!(back, request);
    }
}
