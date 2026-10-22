//! A project open in a tab (Interface, Layout: tabs): its directory, its threads as the daemon
//! last listed them, the thread it shows, and a shell per directory it has shown. The daemon
//! can send all of it again, so a closed tab loses nothing.

use std::{collections::HashMap, path::PathBuf};

use cedian_wire::{Status, ThreadInfo};
use gpui::{Entity, SharedString};

use crate::{
    shell_pane::ShellPane,
    sound::{self, Ending},
    tab_strip::ProjectTab,
    thread_list,
};

pub struct Project {
    pub cwd: PathBuf,
    pub threads: Vec<ThreadInfo>,
    /// The top-level threads archived, last changed first, which the sidebar can bring back.
    pub archived: Vec<ThreadInfo>,
    /// The thread the tab shows, if it has one yet.
    pub current: Option<String>,
    pub status: Option<Status>,
    /// A prompt went out with no thread, and the daemon has not named the thread yet.
    pub awaiting_thread: bool,
    /// What the daemon is doing before the new thread's run starts.
    pub preparing: Option<String>,
    /// Opens the saved or most recent thread when the first list arrives.
    pub first_list: bool,
    /// A shell per directory: the project's, and each worktree a thread opened shows.
    pub shells: HashMap<PathBuf, Entity<ShellPane>>,
}

impl Project {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            threads: Vec::new(),
            archived: Vec::new(),
            current: None,
            status: None,
            awaiting_thread: false,
            preparing: None,
            first_list: true,
            shells: HashMap::new(),
        }
    }

    pub fn name(&self) -> SharedString {
        name_of(&self.cwd)
    }

    /// The thread the tab shows, as the last list had it.
    pub fn info(&self) -> Option<&ThreadInfo> {
        let id = self.current.as_ref()?;
        self.threads.iter().find(|t| &t.thread.id == id)
    }

    /// The top-level threads still in use, and apart from them the archived ones; sub-threads
    /// stay out of both. Returns how each run that was working in the last list ended, for the
    /// sound.
    pub fn set_threads(&mut self, threads: Vec<ThreadInfo>) -> Vec<Ending> {
        let (mut archived, threads): (Vec<ThreadInfo>, Vec<ThreadInfo>) = threads
            .into_iter()
            .filter(|t| t.thread.parent.is_none())
            .partition(|t| t.thread.archived);
        // RFC 3339 times from one clock sort as text.
        archived.sort_by(|a, b| b.thread.updated_at.cmp(&a.thread.updated_at));
        self.archived = archived;
        let endings = threads
            .iter()
            .filter_map(|t| {
                let before = self
                    .threads
                    .iter()
                    .find(|old| old.thread.id == t.thread.id)
                    .map(|old| old.status);
                sound::ending(before, t.status)
            })
            .collect();
        self.threads = threads;
        endings
    }

    /// Where the terminal runs: the open thread's directory (its worktree) when it is on this
    /// machine, else the project.
    pub fn shell_dir(&self) -> PathBuf {
        self.info()
            .and_then(|t| t.cwd.clone())
            .filter(|dir| dir.is_dir())
            .unwrap_or_else(|| self.cwd.clone())
    }

    pub fn waiting(&self) -> usize {
        thread_list::badge_count(&self.threads)
    }

    pub fn working(&self) -> usize {
        self.threads
            .iter()
            .filter(|t| t.status == Status::Working)
            .count()
    }

    pub fn tab(&self) -> ProjectTab {
        ProjectTab {
            name: self.name(),
            working: self.working(),
            waiting: self.waiting(),
            failed: self
                .threads
                .iter()
                .any(|t| t.needs_you && t.status == Status::Failed),
        }
    }
}

pub fn name_of(cwd: &std::path::Path) -> SharedString {
    cwd.file_name()
        .map_or_else(|| "cedian".to_owned(), |n| n.to_string_lossy().into_owned())
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(id: &str, archived: bool, parent: Option<&str>, at: &str) -> ThreadInfo {
        ThreadInfo {
            thread: serde_json::from_value(serde_json::json!({
                "id": id, "title": id, "task": id, "state": "done", "result": null,
                "parent": parent, "archived": archived, "sessions": [], "messages": [],
                "pending": [], "updated_at": at
            }))
            .expect("summary"),
            status: Status::Done,
            cwd: None,
            needs_you: false,
        }
    }

    #[test]
    fn archived_threads_are_kept_apart_newest_first() {
        let mut project = Project::new(PathBuf::from("/repo"));
        project.set_threads(vec![
            info("a", false, None, "2026-10-22T01:00:00.000Z"),
            info("b", true, None, "2026-10-22T02:00:00.000Z"),
            info("c", true, None, "2026-10-22T03:00:00.000Z"),
            info("d", true, Some("a"), "2026-10-22T04:00:00.000Z"),
        ]);
        let ids =
            |list: &[ThreadInfo]| list.iter().map(|t| t.thread.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&project.threads), ["a"]);
        assert_eq!(ids(&project.archived), ["c", "b"]);
    }
}
