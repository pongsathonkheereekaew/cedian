//! A project open in a tab (Interface, Layout: tabs): its directory, its threads as the daemon
//! last listed them, the thread it shows, and a shell per directory it has shown. The daemon
//! can send all of it again, so a closed tab loses nothing.

use std::{collections::HashMap, path::PathBuf};

use cedian_layout::Workspace;
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
    /// The blocks and their named layouts, as `desktop.layout` keeps them.
    pub workspace: Workspace,
    /// The workspace before each layout change, newest last, for `⌘Z`.
    pub undo: Vec<Workspace>,
    /// The project chat's thread the chat block reads, once one is open.
    pub chat_open: Option<String>,
    /// A prompt went out from the project chat with no thread yet.
    pub awaiting_chat: bool,
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
            workspace: Workspace::default(),
            undo: Vec::new(),
            chat_open: None,
            awaiting_chat: false,
        }
    }

    pub fn name(&self) -> SharedString {
        name_of(&self.cwd)
    }

    /// The thread the tab shows, as the last list had it.
    /// The open thread, which may be archived: a thread that archives itself once nothing is
    /// owed on it stays open where the person reads it.
    pub fn info(&self) -> Option<&ThreadInfo> {
        let id = self.current.as_ref()?;
        self.threads
            .iter()
            .chain(&self.archived)
            .find(|t| &t.thread.id == id)
    }

    /// The project chat's thread: the newest top-level thread on cedia's `coordinator` profile.
    pub fn chat_thread(&self) -> Option<&ThreadInfo> {
        self.threads
            .iter()
            .filter(|t| is_chat(t))
            .max_by(|a, b| a.thread.updated_at.cmp(&b.thread.updated_at))
    }

    /// The threads the project chat opened, newest first.
    pub fn opened_by_chat(&self) -> Vec<&ThreadInfo> {
        let Some(chat) = self.chat_thread() else {
            return Vec::new();
        };
        let mut opened: Vec<&ThreadInfo> = self
            .threads
            .iter()
            .chain(&self.archived)
            .filter(|t| t.thread.by.as_deref() == Some(chat.thread.id.as_str()))
            .collect();
        opened.sort_by(|a, b| b.thread.updated_at.cmp(&a.thread.updated_at));
        opened
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

/// A thread of the project chat, which Threads shows as its own row and not in a group.
pub fn is_chat(info: &ThreadInfo) -> bool {
    info.thread.profile.as_deref() == Some(CHAT_PROFILE)
}

/// cedia's built-in profile for the project's coordinator.
pub const CHAT_PROFILE: &str = "coordinator";

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
        // An open thread that archived itself is still the one shown.
        project.current = Some("c".into());
        assert_eq!(project.info().map(|t| t.thread.id.as_str()), Some("c"));
    }

    #[test]
    fn the_chat_is_the_newest_coordinator_thread_and_lists_what_it_opened() {
        let mut project = Project::new(PathBuf::from("/repo"));
        let mut chat = info("c", false, None, "2026-10-22T05:00:00.000Z");
        chat.thread.profile = Some(CHAT_PROFILE.into());
        let mut opened = info("o", false, None, "2026-10-22T06:00:00.000Z");
        opened.thread.by = Some("c".into());
        project.set_threads(vec![
            chat,
            opened,
            info("x", false, None, "2026-10-22T07:00:00.000Z"),
        ]);
        assert_eq!(
            project.chat_thread().map(|t| t.thread.id.as_str()),
            Some("c")
        );
        let ids: Vec<&str> = project
            .opened_by_chat()
            .iter()
            .map(|t| t.thread.id.as_str())
            .collect();
        assert_eq!(ids, ["o"]);
    }
}
