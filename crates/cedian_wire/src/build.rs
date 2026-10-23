use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};

/// Which build of the daemon answers: its binary, and that file's modification time when the
/// daemon started. A window compares it with the daemon beside its own binary, so a window
/// from a new app does not drive a daemon left running from an old one (or from a build
/// replaced in place since).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    pub exe: PathBuf,
    /// Seconds since the Unix epoch.
    pub modified: u64,
}

impl Build {
    /// The build of the binary at `exe` as it is on disk now.
    pub fn of(exe: &Path) -> Option<Self> {
        let exe = exe.canonicalize().ok()?;
        let modified = std::fs::metadata(&exe)
            .ok()?
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs();
        Some(Self { exe, modified })
    }

    /// This process's own build.
    pub fn current() -> Option<Self> {
        Self::of(&std::env::current_exe().ok()?)
    }
}
