//! Notices threads that change outside a run of this daemon (`cedia threads send` in a
//! terminal, another cedia, an archive) by polling the threads directory of each project a
//! window shows, and sends the new list when it moved. Each poll also starts the threads the
//! project's coordinator opened and that wait `queued`, and wakes the scheduled runs that are
//! due.

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash as _, Hasher as _},
    path::Path,
    time::Duration,
};

use crate::daemon::{Daemon, Shared, lock};

const POLL: Duration = Duration::from_secs(1);

/// Each project's threads directory as last seen: its files' names, sizes and times.
#[derive(Default)]
pub struct Fingerprints(HashMap<String, u64>);

impl Fingerprints {
    /// Records `fingerprint` for `slug`; true when it differs from what was there before.
    pub fn changed(&mut self, slug: &str, fingerprint: u64) -> bool {
        self.0.insert(slug.to_string(), fingerprint) != Some(fingerprint)
    }

    /// Forgets the projects no window shows any more.
    pub fn retain(&mut self, shown: &[String]) {
        self.0.retain(|slug, _| shown.contains(slug));
    }
}

/// The directory's state in one number. A missing directory has its own.
pub fn fingerprint(dir: &Path) -> u64 {
    let mut files: Vec<_> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let meta = entry.metadata().ok()?;
                Some((entry.file_name(), meta.len(), meta.modified().ok()))
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    let mut hasher = DefaultHasher::new();
    files.hash(&mut hasher);
    hasher.finish()
}

pub fn watch_threads(shared: Shared) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(POLL);
            let dirs = lock(&shared).shown_thread_dirs();
            // The directories are read without the lock; a run's own changes refresh anyway.
            let prints: Vec<(String, u64)> = dirs
                .into_iter()
                .map(|(slug, dir)| {
                    let print = fingerprint(&dir);
                    (slug, print)
                })
                .collect();
            lock(&shared).threads_seen(prints);
            Daemon::start_queued(&shared);
            Daemon::wake_schedules(&shared);
        }
    });
}
