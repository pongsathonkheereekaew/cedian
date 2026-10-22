//! One cedia run: its child process, the lines it has streamed, and the threads that read its
//! output.

use std::{
    collections::{HashSet, VecDeque},
    io::{BufRead as _, BufReader},
    path::PathBuf,
    process::Child,
};

use crate::daemon::{ClientId, Shared, lock};

pub type RunId = u64;

/// Lines of stderr kept to explain a run that failed before it had a thread.
const STDERR_TAIL: usize = 40;

pub struct Run {
    pub cwd: PathBuf,
    pub slug: String,
    /// Known from the run's first session event.
    pub thread: Option<String>,
    pub pid: u32,
    pub alive: bool,
    /// Every stdout line so far, so a window that opens the thread mid-run catches up.
    pub lines: Vec<String>,
    /// The thread's sessions before this run; later ones are in `lines`.
    pub before: HashSet<String>,
    /// The client that asked for the run, told when its new thread gets an id.
    pub requester: Option<ClientId>,
    pub stderr: VecDeque<String>,
}

impl Run {
    pub fn push_stderr(&mut self, line: String) {
        if self.stderr.len() == STDERR_TAIL {
            self.stderr.pop_front();
        }
        self.stderr.push_back(line);
    }
}

/// Reads the child's stdout and stderr on their own threads and reports each line, then the
/// exit, to the daemon.
pub fn watch(shared: Shared, id: RunId, mut child: Child) {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    if let Some(stderr) = stderr {
        let shared = shared.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                match line {
                    Ok(line) => lock(&shared).on_stderr(id, line),
                    Err(error) => {
                        log::warn!("run {id} stderr: {error}");
                        break;
                    }
                }
            }
        });
    }
    std::thread::spawn(move || {
        if let Some(stdout) = stdout {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) if line.trim().is_empty() => {}
                    Ok(line) => lock(&shared).on_line(id, line),
                    Err(error) => {
                        log::warn!("run {id} stdout: {error}");
                        break;
                    }
                }
            }
        }
        let status = child.wait();
        lock(&shared).on_exit(id, status);
    });
}
