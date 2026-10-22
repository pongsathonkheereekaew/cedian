//! A project open in a tab (Interface, Layout: tabs): its directory, its threads as the daemon
//! last listed them, the thread it shows, and a shell per directory it has shown. The daemon
//! can send all of it again, so a closed tab loses nothing.

use std::{collections::HashMap, path::PathBuf};

use cedian_wire::{Status, ThreadInfo};
use gpui::{Entity, SharedString};

use crate::{shell_pane::ShellPane, tab_strip::ProjectTab, thread_list};

pub struct Project {
    pub cwd: PathBuf,
    pub threads: Vec<ThreadInfo>,
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

    /// The top-level threads still in use: archived ones and sub-threads stay out of the list.
    pub fn set_threads(&mut self, threads: Vec<ThreadInfo>) {
        self.threads = threads
            .into_iter()
            .filter(|t| !t.thread.archived && t.thread.parent.is_none())
            .collect();
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
