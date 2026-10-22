//! The runs alive when the daemon last looked, kept in its state directory. cedia runs in its
//! own process group, so a run outlives a daemon that crashes or is restarted; the next daemon
//! reads this file and follows each run that is still going.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub pid: u32,
    pub thread: String,
    pub cwd: PathBuf,
    pub slug: String,
    /// The thread's sessions before the run, as `Run::before`.
    pub before: Vec<String>,
}

pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    pub fn new(state_dir: &Path) -> Self {
        Self {
            path: state_dir.join("runs.json"),
        }
    }

    pub fn load(&self) -> Vec<RunRecord> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            Err(error) => {
                log::warn!("reading {}: {error}", self.path.display());
                return Vec::new();
            }
        };
        serde_json::from_str(&text).unwrap_or_else(|error| {
            log::warn!("{}: {error}", self.path.display());
            Vec::new()
        })
    }

    /// Replaces the file in one rename, so a crash mid-write leaves the old list.
    pub fn save(&self, runs: &[RunRecord]) -> anyhow::Result<()> {
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec(runs)?)
            .with_context(|| format!("writing {}", temporary.display()))?;
        std::fs::rename(&temporary, &self.path)
            .with_context(|| format!("replacing {}", self.path.display()))
    }
}

/// Whether `pid` is still a cedia run: alive, and its command line is a stream-json run, so a
/// pid the system reused for something else is not mistaken for it.
pub fn still_running(pid: u32) -> bool {
    let Ok(raw) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 only checks that the process exists.
    if unsafe { libc::kill(raw, 0) } != 0 {
        return false;
    }
    std::process::Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains("stream-json"))
}
