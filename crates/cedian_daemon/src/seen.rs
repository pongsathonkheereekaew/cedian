//! Which thread changes the person has seen, kept in the daemon's state directory: for each
//! thread, the `updated_at` it had when a window last showed it. A thread that finished,
//! failed or asked after that needs the person, and counts in the badges until opened.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use cedian_wire::Status;

pub struct Seen {
    path: PathBuf,
    /// `<project key>/<thread id>` to the `updated_at` last seen.
    marks: BTreeMap<String, String>,
}

impl Seen {
    pub fn new(state_dir: &Path) -> Self {
        let path = state_dir.join("seen.json");
        let marks = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                log::warn!("{}: {error}", path.display());
                BTreeMap::new()
            }),
            Err(_) => BTreeMap::new(),
        };
        Self { path, marks }
    }

    /// True when the thread changed since it was last seen and its state is one the person
    /// should look at. A thread never seen counts only if it needs an answer or failed, so
    /// the threads from before cedian do not all light up at once.
    pub fn needs_you(&self, slug: &str, thread: &str, updated_at: &str, status: Status) -> bool {
        let wants = matches!(
            status,
            Status::Done | Status::Failed | Status::Interrupted | Status::Asking
        );
        match self.marks.get(&key(slug, thread)) {
            Some(seen) => wants && seen.as_str() < updated_at,
            None => matches!(
                status,
                Status::Asking | Status::Failed | Status::Interrupted
            ),
        }
    }

    /// Records that a window showed the thread as of `updated_at`; true when that is news.
    pub fn mark(&mut self, slug: &str, thread: &str, updated_at: &str) -> bool {
        let key = key(slug, thread);
        if self
            .marks
            .get(&key)
            .is_some_and(|seen| seen.as_str() >= updated_at)
        {
            return false;
        }
        self.marks.insert(key, updated_at.to_owned());
        if let Err(error) = self.save() {
            log::warn!("{error:#}");
        }
        true
    }

    fn save(&self) -> anyhow::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(&self.marks)?)
            .with_context(|| format!("writing {}", temp.display()))?;
        std::fs::rename(&temp, &self.path)
            .with_context(|| format!("replacing {}", self.path.display()))
    }
}

fn key(slug: &str, thread: &str) -> String {
    format!("{slug}/{thread}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_needs_you_until_it_is_seen_and_again_when_it_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut seen = Seen::new(dir.path());
        let at = "2026-10-22T03:00:00.000Z";
        assert!(
            !seen.needs_you("p", "t-1", at, Status::Done),
            "unseen and old"
        );
        assert!(seen.needs_you("p", "t-1", at, Status::Asking));
        assert!(seen.mark("p", "t-1", at));
        assert!(!seen.needs_you("p", "t-1", at, Status::Asking));
        let later = "2026-10-22T03:05:00.000Z";
        assert!(seen.needs_you("p", "t-1", later, Status::Done));
        assert!(!seen.needs_you("p", "t-1", later, Status::Working));
        assert!(!seen.needs_you("p", "t-1", later, Status::Stopped));
        assert!(!seen.mark("p", "t-1", at), "an older mark is not news");

        // It survives a restart.
        let again = Seen::new(dir.path());
        assert!(!again.needs_you("p", "t-1", at, Status::Failed));
    }
}
