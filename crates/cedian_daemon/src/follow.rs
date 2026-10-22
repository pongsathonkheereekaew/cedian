//! Follows a run that a previous daemon started: there is no pipe to its stdout, so its lines
//! come from the session logs it writes, and its end from the pid going away.

use std::{
    collections::HashMap,
    io::{Read as _, Seek as _, SeekFrom},
    path::{Path, PathBuf},
    time::Duration,
};

use crate::{
    daemon::{Shared, lock},
    run::RunId,
};

const POLL: Duration = Duration::from_millis(500);

/// How far into each of the run's session logs has been read.
#[derive(Default)]
pub struct Offsets(HashMap<PathBuf, u64>);

impl Offsets {
    /// The complete lines appended to `path` since the last call, as stream-json lines.
    pub fn read_new(&mut self, path: &Path) -> Vec<String> {
        let offset = self.0.entry(path.to_path_buf()).or_default();
        let Ok(mut file) = std::fs::File::open(path) else {
            return Vec::new();
        };
        let mut text = String::new();
        if file.seek(SeekFrom::Start(*offset)).is_err() || file.read_to_string(&mut text).is_err() {
            return Vec::new();
        }
        // A line still being written waits for the next poll.
        let complete = text.rfind('\n').map_or(0, |end| end + 1);
        *offset += complete as u64;
        text[..complete]
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(
                |line| match serde_json::from_str::<serde_json::Value>(line) {
                    Ok(event) => {
                        Some(serde_json::json!({"type": "event", "event": event}).to_string())
                    }
                    Err(error) => {
                        log::warn!("{}: {error}", path.display());
                        None
                    }
                },
            )
            .collect()
    }
}

pub fn follow(shared: Shared, id: RunId) {
    std::thread::spawn(move || {
        while lock(&shared).poll_followed(id) {
            std::thread::sleep(POLL);
        }
    });
}
