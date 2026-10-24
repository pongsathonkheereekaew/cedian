//! The window: the project tabs over the front project's blocks (Interface, Blocks). It keeps
//! only what the daemon can send again.

mod background;
mod blocks;
mod chat;
mod editing;
mod knowledge;
mod restore;
mod run_choices;
mod ship;
mod tear_off;
mod thread_edit;

pub(crate) use thread_edit::{CancelRename, ConfirmRename};
#[cfg(test)]
mod window_tests;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use cedian_layout::{BlockId, Kind, Rect, Side};
use cedian_protocol::{Line, TodoState};
use cedian_wire::{HunkState, Notice, Request, Status, ThreadInfo};
use editor::Editor;
use futures::StreamExt as _;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, EntityInputHandler as _, Focusable as _,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, Window, WindowControlArea, WindowHandle,
    deferred, div, prelude::FluentBuilder as _, px, svg,
};
use settings::Settings as _;
use theme::ActiveTheme as _;

use crate::type_scale::TypeScale as _;
use crate::{
    agent_pane::{AgentEvent, AgentPane, Header, StopRun},
    browser_pane::{BrowserEvent, BrowserPane},
    composer::{Composer, ComposerEvent, Queued},
    content_search::{ContentEvent, ContentSearch, SearchContents},
    daemon_client::DaemonClient,
    desktop::Desktop,
    dock, export,
    file_finder::{FileFinder, FindFile, FinderEvent},
    file_tree::{FileTree, FileTreeEvent},
    git_pane::{GitEvent, GitPane},
    layout::Layout,
    line_breaks,
    memory_pane::MemoryPane,
    outputs_pane::{OutputsEvent, OutputsPane},
    project::Project,
    review_pane::{ReviewEvent, ReviewPane},
    settings_view::{SettingsEvent, SettingsView},
    settings_window::SettingsWindow,
    shell_pane::{AttachSelection, ShellEvent, ShellPane},
    sidebar_foot::{self, StatusLine},
    sound::{self, Sound},
    tab_strip::{
        self, ActivateTab, CloseTab, NewTab, NextTab, PreviousTab, ProjectTab, TabClick, TabDrag,
    },
    thread_list::{self, Group},
    thread_search::{Act, FocusSearch, Found, SearchEvent, ThreadSearch},
    tokens,
    usage_pane::{UsageEvent, UsagePane},
    view_mode::ViewMode,
    workplace::Workplace,
};

/// The conversation's 700 px column with the composer's own margins.
const COMPOSER_WIDTH: f32 = 732.;
/// Under this window width the title bar keeps only what acts (Interface, Blocks: nothing
/// leaks).
const COMPACT_WINDOW: f32 = 1000.;

enum Connection {
    /// Playing a recorded run; there is no daemon.
    Replay,
    Connecting,
    Connected {
        cedia: SharedString,
    },
    Failed(SharedString),
}

pub struct RootView {
    client: Option<Arc<DaemonClient>>,
    connection: Connection,
    /// The project tabs, never empty; the one in `active` fills the window.
    projects: Vec<Project>,
    active: usize,
    /// A new thread runs in its own git worktree.
    use_worktree: bool,
    error: Option<SharedString>,
    agent: Entity<AgentPane>,
    composer: Entity<Composer>,
    /// The project chat's conversation and composer, in the Project chat block.
    chat: Entity<AgentPane>,
    chat_composer: Entity<Composer>,
    editor: Entity<Editor>,
    /// What the Editor shows, when Files, Git or a search put something in it.
    editor_shows: Option<SharedString>,
    /// The file the Editor holds to edit; none for a diff or a file it only names.
    editor_file: Option<crate::editor_file::EditorFile>,
    /// The Editor holds edits its file does not have.
    editor_dirty: bool,
    /// The text an agent wrote to the file while the person had edits: Override or Discard.
    editor_conflict: Option<String>,
    /// Unsaved text of files the Editor left for another, put back when they come back.
    editor_drafts: HashMap<PathBuf, String>,
    /// Checks the file on disk each second while the Editor holds one.
    editor_watch: Option<Task<()>>,
    files: Entity<FileTree>,
    git: Entity<GitPane>,
    outputs: Entity<OutputsPane>,
    memory: Entity<MemoryPane>,
    skills: Entity<crate::skills_pane::SkillsPane>,
    workplace: Entity<Workplace>,
    /// The models and permission modes cedia lists, and the picks for the next runs.
    choices: run_choices::RunChoices,
    search: Entity<ThreadSearch>,
    review: Entity<ReviewPane>,
    usage: Entity<UsagePane>,
    browser: Entity<BrowserPane>,
    /// Hunks of the open thread's review still waiting, for the Review block's meta.
    waiting_hunks: usize,
    /// The block keys act on, the one zoomed to fill the area, and where a dragged block
    /// would land.
    focused: Option<BlockId>,
    zoomed: Option<BlockId>,
    drop_target: Option<(BlockId, Side)>,
    /// The block whose ⋯ menu is open.
    block_menu: Option<BlockId>,
    /// The + Tool menu, and the timers that open and close it on hover.
    tool_menu: Option<blocks::ToolTarget>,
    tool_menu_opening: Option<Task<()>>,
    tool_menu_closing: Option<Task<()>>,
    layout_menu: bool,
    overview: bool,
    /// The area the blocks fill, measured at each frame.
    area: Rect,
    /// Holds the keys when no block's own input has them, so the block keys still reach the
    /// window after the focused input leaves the screen (a zoom, a park).
    root_focus: gpui::FocusHandle,
    /// The focused block changed: the next frame gives its input the keys.
    refocus: bool,
    /// The window is too small for the layout, so some leaves are drawn folded into others.
    folded: bool,
    settings: Entity<SettingsView>,
    settings_window: Option<WindowHandle<SettingsWindow>>,
    /// What the next window opens with; saved shortly after it changes.
    layout: Layout,
    saving_layout: Option<Task<()>>,
    /// A press on the tab strip that moves the window once the mouse moves.
    should_move: bool,
    /// The thread archived last, while the sidebar offers to bring it back.
    archived: Option<Archived>,
    /// Whether the sidebar lists the tab's archived threads.
    show_archived: bool,
    /// Whether a run that ends plays a sound (`desktop.sound`).
    sound: Sound,
    /// The count last put on the Dock icon.
    dock_badge: Option<usize>,
    /// cedia can fork a thread at a run (`threads.fork`), which edit and resend take.
    can_fork: bool,
    /// cedia undoes a turn and rewinds a thread for a client (`threads.restore-json`).
    can_restore: bool,
    /// cedia renames and deletes threads (`threads.rename`, `threads.delete`).
    can_rename: bool,
    can_delete: bool,
    /// The thread list's row whose title is being edited.
    renaming: Option<thread_edit::Renaming>,
    /// The row asking whether to delete its thread.
    deleting: Option<String>,
    /// The card over the composer saying what an undo or a rewind did, or asking first.
    restore: Option<restore::RestoreCard>,
    /// cedia commits and opens pull requests (`commit`, `pr`).
    can_commit: bool,
    can_pr: bool,
    /// The card over the composer that commits the thread's files or opens its pull request,
    /// and the field its commit message is edited in.
    ship: Option<ship::ShipCard>,
    ship_message: Option<Entity<Editor>>,
    /// Threads whose last turn was undone, with that turn's tree after it: the receipt offers
    /// redo while the thread's last turn is still that one.
    undone: std::collections::HashMap<String, String>,
    /// The sent message the composer holds to send again: its thread and the run it started.
    editing: Option<(String, String)>,
    /// `⌘P`, while open.
    finder: Option<Entity<FileFinder>>,
    /// `⌘⇧F`, while open.
    contents: Option<Entity<ContentSearch>>,
    /// The blocks moving after a split, close, zoom or park.
    motion: Option<crate::motion::Motion>,
    /// A window a block was dragged out into: its tree is its own and is not saved.
    torn: bool,
    /// The block being dragged and where the mouse last was, to open it in a new window when
    /// it is let go outside this one.
    dragging: Option<(BlockId, gpui::Point<gpui::Pixels>)>,
}

impl RootView {
    pub fn replay(agent: Entity<AgentPane>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self::new(vec![PathBuf::new()], 0, agent, window, cx);
        view.connection = Connection::Replay;
        view
    }

    /// Connects to the daemon in the background and shows the threads of the projects in
    /// `tabs`, `active` in front.
    pub fn live(
        tabs: Vec<PathBuf>,
        active: usize,
        layout: Layout,
        view_mode: ViewMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let agent = cx.new(|_| AgentPane::new("new thread").with_view_mode(view_mode));
        let mut view = Self::new(tabs, active, agent, window, cx);
        view.layout = layout;
        let mut projects = std::mem::take(&mut view.projects);
        for project in &mut projects {
            view.load_blocks(project);
        }
        view.projects = projects;
        view.focused = view.ws().find(Kind::Agent);
        view.set_search_actions(cx);
        view.remember_tabs();
        view.layout.bounds = Some(saved_bounds(window));
        cx.observe_window_bounds(window, |view, window, cx| {
            view.layout.bounds = Some(saved_bounds(window));
            view.save_layout(cx);
        })
        .detach();
        view.composer
            .update(cx, |composer, cx| composer.focus(window, cx));
        cx.spawn(async move |this, cx| {
            let connected = cx
                .background_executor()
                .spawn(async { DaemonClient::connect() })
                .await;
            let mut notices = match connected {
                Ok((client, notices)) => {
                    let client = Arc::new(client);
                    if this
                        .update(cx, |view, _| view.client = Some(client))
                        .is_err()
                    {
                        return;
                    }
                    notices
                }
                Err(error) => {
                    log::error!("{error:#}");
                    this.update(cx, |view, cx| {
                        view.connection = Connection::Failed(format!("{error:#}").into());
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            while let Some(notice) = notices.next().await {
                if this
                    .update(cx, |view, cx| view.on_notice(notice, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        view
    }

    fn new(
        tabs: Vec<PathBuf>,
        active: usize,
        agent: Entity<AgentPane>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| Editor::multi_line(window, cx));
        cx.subscribe(&editor, |view, _, event, cx| {
            if matches!(event, editor::EditorEvent::BufferEdited) {
                view.editor_edited(cx);
            }
        })
        .detach();
        let tabs = if tabs.is_empty() {
            vec![PathBuf::new()]
        } else {
            tabs
        };
        let active = active.min(tabs.len() - 1);
        let workplace = cx.new(|_| Workplace::new());
        let search = cx.new(|cx| ThreadSearch::new(window, cx));
        cx.subscribe_in(&search, window, |view, _, event, window, cx| match event {
            SearchEvent::Open { cwd, thread } => {
                view.open_thread(cwd, thread.clone(), cx);
                view.composer
                    .update(cx, |composer, cx| composer.focus(window, cx));
            }
            SearchEvent::Act(act) => {
                match *act {
                    Act::Layout(n) => view.switch_layout(n, cx),
                    Act::Add(kind) => {
                        view.open_kind(kind, blocks::ToolTarget { against: None }, cx)
                    }
                }
                view.refocus = true;
            }
            SearchEvent::Dismissed => view
                .composer
                .update(cx, |composer, cx| composer.focus(window, cx)),
        })
        .detach();
        // The sidebar shows the search's matches in place of its groups while it has text.
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        // Thread details reads the transcript.
        cx.observe(&agent, |view, _, cx| {
            if view.showing(Kind::Session) {
                cx.notify();
            }
        })
        .detach();
        let composer = cx.new(|cx| Composer::new(window, cx));
        cx.subscribe(&composer, |view, _, event, cx| match event {
            ComposerEvent::Submit(text, images) => view.submit(text.clone(), images.clone(), cx),
            ComposerEvent::CancelEdit => view.editing = None,
            ComposerEvent::WantCommands => view.request(Request::Commands {
                cwd: view.project().cwd.clone(),
            }),
            ComposerEvent::Unsend(id) => {
                if let Some(thread) = view.project().current.clone() {
                    view.request(Request::Unsend {
                        cwd: view.project().cwd.clone(),
                        thread,
                        ids: vec![id.clone()],
                    });
                }
            }
        })
        .detach();
        cx.subscribe_in(
            &agent,
            window,
            |view, agent, event, window, cx| match event {
                AgentEvent::Edit(text) => view.edit_sent(text, window, cx),
                AgentEvent::Rewind(text) => view.ask_rewind(text, cx),
                AgentEvent::Restore { redo } => view.restore_turn(*redo, false, cx),
                AgentEvent::Rate(rating) => {
                    if let Some(thread) = view.project().current.clone() {
                        view.request(Request::Rate {
                            cwd: view.project().cwd.clone(),
                            thread,
                            rating: *rating,
                        });
                    }
                }
                AgentEvent::ViewMode(mode) => {
                    let mode = *mode;
                    agent.update(cx, |agent, cx| agent.set_view_mode(mode, cx));
                    view.request(Request::SetDesktop {
                        key: "view_mode".into(),
                        value: mode.key().into(),
                    });
                }
                AgentEvent::Commit => view.start_commit(window, cx),
                AgentEvent::OpenPr => view.start_pr(cx),
                AgentEvent::Review => {
                    view.open_kind(Kind::Review, blocks::ToolTarget { against: None }, cx)
                }
            },
        )
        .detach();
        let chat = cx.new(|_| AgentPane::new("แชทโปรเจกต์"));
        let chat_composer = cx.new(|cx| Composer::new(window, cx));
        cx.subscribe(&chat_composer, |view, _, event, cx| match event {
            ComposerEvent::Submit(text, images) => {
                view.submit_chat(text.clone(), images.clone(), cx)
            }
            ComposerEvent::CancelEdit => {}
            ComposerEvent::WantCommands => view.request(Request::Commands {
                cwd: view.project().cwd.clone(),
            }),
            ComposerEvent::Unsend(id) => {
                if let Some(thread) = view.project().chat_open.clone() {
                    view.request(Request::Unsend {
                        cwd: view.project().cwd.clone(),
                        thread,
                        ids: vec![id.clone()],
                    });
                }
            }
        })
        .detach();
        let files = cx.new(|_| FileTree::default());
        cx.subscribe_in(&files, window, |view, _, event, window, cx| match event {
            FileTreeEvent::Open(path) => view.show_file(path.clone(), window, cx),
        })
        .detach();
        // `@` in the composers lists the same files.
        cx.observe(&files, |view, files, cx| {
            let (root, count) = {
                let tree = files.read(cx);
                (tree.root().to_owned(), tree.files().len())
            };
            if view.composer.read(cx).has_files(&root, count) {
                return;
            }
            let list = files.read(cx).files().to_vec();
            for composer in [&view.composer, &view.chat_composer] {
                composer.update(cx, |composer, _| composer.set_files(&root, &list));
            }
        })
        .detach();
        let git = cx.new(|_| GitPane::default());
        cx.subscribe_in(&git, window, |view, _, event, window, cx| match event {
            GitEvent::Show { name, text } => view.show_text(name.into(), text.clone(), window, cx),
        })
        .detach();
        let outputs = cx.new(|_| OutputsPane::default());
        cx.subscribe_in(&outputs, window, |view, _, event, window, cx| match event {
            OutputsEvent::Open(path) => view.show_file(path.clone(), window, cx),
        })
        .detach();
        let memory = cx.new(|_| MemoryPane::default());
        let skills = cx.new(|_| crate::skills_pane::SkillsPane::default());
        cx.subscribe_in(&skills, window, |view, _, event, window, cx| match event {
            crate::skills_pane::SkillsEvent::Use(name) => view
                .composer
                .update(cx, |composer, cx| composer.use_command(name, window, cx)),
        })
        .detach();
        let settings = cx.new(|_| SettingsView::default());
        cx.subscribe(&settings, |view, _, event, _| match event {
            SettingsEvent::Set { key, value } => view.request(Request::SetDesktop {
                key: (*key).to_owned(),
                value: value.clone(),
            }),
            SettingsEvent::Send(request) => view.request((**request).clone()),
        })
        .detach();
        let review = cx.new(|cx| {
            let mut pane = ReviewPane::new(cx);
            pane.set_agent(&agent, cx);
            pane
        });
        cx.subscribe(&review, |view, _, event, cx| match event {
            ReviewEvent::Decide { hunk, decision } => {
                if let Some(thread) = view.project().current.clone() {
                    view.request(Request::Decide {
                        cwd: view.project().cwd.clone(),
                        thread,
                        hunk: hunk.clone(),
                        decision: decision.clone(),
                    });
                }
            }
            ReviewEvent::Refresh => view.refresh_review(),
            ReviewEvent::Rerun { command, path } => view.submit(
                format!("รัน check `{command}` ใหม่: {path} ถูกแก้หลังจาก check ครั้งล่าสุด"),
                Vec::new(),
                cx,
            ),
        })
        .detach();
        let browser = cx.new(|cx| BrowserPane::new(window, cx));
        cx.subscribe_in(&browser, window, |view, _, event, window, cx| match event {
            BrowserEvent::Wanted => view.request(Request::Browser {
                cwd: view.project().cwd.clone(),
            }),
            BrowserEvent::Captured(path) => view.composer.update(cx, |composer, cx| {
                composer.add_files(std::slice::from_ref(path), window, cx)
            }),
        })
        .detach();
        let usage = cx.new(|_| UsagePane::new());
        cx.subscribe(&usage, |view, _, event, cx| match event {
            UsageEvent::Changed => {
                view.ask_usage(cx);
                cx.notify();
            }
        })
        .detach();
        Self {
            client: None,
            connection: Connection::Connecting,
            projects: tabs.into_iter().map(Project::new).collect(),
            active,
            use_worktree: true,
            error: None,
            agent,
            composer,
            chat,
            chat_composer,
            editor,
            editor_shows: None,
            editor_file: None,
            editor_dirty: false,
            editor_conflict: None,
            editor_drafts: HashMap::new(),
            editor_watch: None,
            files,
            git,
            outputs,
            memory,
            skills,
            workplace,
            choices: run_choices::RunChoices::default(),
            search,
            review,
            usage,
            browser,
            waiting_hunks: 0,
            focused: None,
            zoomed: None,
            drop_target: None,
            block_menu: None,
            tool_menu: None,
            tool_menu_opening: None,
            tool_menu_closing: None,
            layout_menu: false,
            overview: false,
            area: Rect::new(0., 0., 1200., 800.),
            root_focus: cx.focus_handle(),
            refocus: false,
            folded: false,
            settings,
            settings_window: None,
            layout: Layout::default(),
            saving_layout: None,
            should_move: false,
            archived: None,
            show_archived: false,
            sound: Sound::Off,
            dock_badge: None,
            can_fork: false,
            can_restore: false,
            can_rename: false,
            can_delete: false,
            renaming: None,
            deleting: None,
            restore: None,
            can_commit: false,
            can_pr: false,
            ship: None,
            ship_message: None,
            undone: std::collections::HashMap::new(),
            editing: None,
            finder: None,
            contents: None,
            motion: None,
            torn: false,
            dragging: None,
        }
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.request(Request::Settings);
        self.settings_window =
            SettingsWindow::show(self.settings.clone(), self.settings_window.take(), cx);
    }

    /// Asks the daemon for the usage the Usage block shows, once per range, filter and
    /// project.
    pub(super) fn ask_usage(&mut self, cx: &mut Context<Self>) {
        let cwd = self.project().cwd.clone();
        let today = chrono::Local::now().date_naive();
        let Some((cwd, since)) = self.usage.update(cx, |pane, _| pane.take_want(&cwd, today))
        else {
            return;
        };
        // cedia names a project by its key; the tabs give the names.
        let names = self
            .projects
            .iter()
            .filter(|p| !p.cwd.as_os_str().is_empty())
            .map(|p| {
                let name = p
                    .cwd
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.cwd.display().to_string());
                (cedian_index::project_key(&p.cwd), name)
            })
            .collect();
        self.usage.update(cx, |pane, _| pane.set_names(names));
        self.request(Request::Usage { cwd, since });
    }

    /// Whether the open thread is working and its last tool call is the browser, for the
    /// Browser block's line that the agent is using the page.
    pub(super) fn agent_on_browser(&self, cx: &gpui::App) -> bool {
        self.project().status == Some(Status::Working)
            && self
                .agent
                .read(cx)
                .transcript()
                .blocks()
                .iter()
                .rev()
                .find_map(|block| match block {
                    cedian_agent::Block::Tool(call) => Some(call.name == "browser"),
                    _ => None,
                })
                .unwrap_or(false)
    }

    fn request(&self, request: Request) {
        if let Some(client) = &self.client {
            client.request(&request);
        }
    }

    fn on_notice(&mut self, notice: Notice, cx: &mut Context<Self>) {
        match notice {
            Notice::Welcome { cedia, .. } => {
                // A Browser block drawn before the daemon answered asked no one: ask again.
                self.browser.update(cx, |pane, cx| pane.forget(cx));
                self.can_fork = cedia.as_ref().is_ok_and(|v| {
                    v.capabilities
                        .contains(&cedian_protocol::Capability::ThreadsFork)
                });
                self.can_restore = cedia.as_ref().is_ok_and(|v| {
                    v.capabilities
                        .contains(&cedian_protocol::Capability::ThreadsRestoreJson)
                });
                self.can_rename = cedia.as_ref().is_ok_and(|v| {
                    v.capabilities
                        .contains(&cedian_protocol::Capability::ThreadsRename)
                });
                self.can_delete = cedia.as_ref().is_ok_and(|v| {
                    v.capabilities
                        .contains(&cedian_protocol::Capability::ThreadsDelete)
                });
                self.can_commit = cedia.as_ref().is_ok_and(|v| {
                    v.capabilities
                        .contains(&cedian_protocol::Capability::Commit)
                });
                self.can_pr = cedia
                    .as_ref()
                    .is_ok_and(|v| v.capabilities.contains(&cedian_protocol::Capability::Pr));
                let can_mcp = cedia.as_ref().is_ok_and(|v| {
                    v.capabilities
                        .contains(&cedian_protocol::Capability::McpManage)
                });
                self.settings.update(cx, |view, cx| {
                    view.mcp.capable = can_mcp;
                    cx.notify();
                });
                let can_attach = cedia.as_ref().is_ok_and(|v| {
                    v.capabilities
                        .contains(&cedian_protocol::Capability::Images)
                });
                for composer in [&self.composer, &self.chat_composer] {
                    composer.update(cx, |composer, cx| composer.set_can_attach(can_attach, cx));
                }
                let cedia = match cedia {
                    // A cedia whose schema is not the pinned one may send shapes cedian skips.
                    Ok(version) if version.protocol != cedian_protocol::PROTOCOL_HASH => {
                        log::warn!(
                            "cedia speaks protocol {}, cedian was built on {}",
                            version.protocol,
                            cedian_protocol::PROTOCOL_HASH
                        );
                        format!("cedia {} · protocol ไม่ตรงกับที่ pin", version.version)
                    }
                    Ok(version) => format!("cedia {}", version.version),
                    Err(error) => {
                        self.error = Some(error.into());
                        "cedia not found".to_owned()
                    }
                };
                self.connection = Connection::Connected {
                    cedia: cedia.into(),
                };
                for project in &self.projects {
                    self.request(Request::Threads {
                        cwd: project.cwd.clone(),
                    });
                    self.request(Request::Background {
                        cwd: project.cwd.clone(),
                    });
                }
                self.request(Request::Settings);
            }
            Notice::Choices {
                models,
                permissions,
            } => {
                self.choices.set(models, permissions);
                cx.notify();
            }
            Notice::Forked { cwd, from, thread } => {
                self.choices.forked(&from, &thread);
                if cwd == self.project().cwd && self.project().current.as_ref() == Some(&from) {
                    self.select(thread, cx);
                }
            }
            Notice::Commands { cwd, commands } if cwd == self.project().cwd => {
                self.skills
                    .update(cx, |pane, cx| pane.set(commands.clone(), cx));
                self.composer.update(cx, |composer, cx| {
                    composer.set_commands(commands.clone(), cx)
                });
                self.chat_composer
                    .update(cx, |composer, cx| composer.set_commands(commands, cx));
            }
            Notice::Commands { .. } => {}
            Notice::Restored {
                cwd,
                thread,
                result,
            } if cwd == self.project().cwd => self.on_restored(thread, result, cx),
            Notice::Rewound {
                cwd,
                thread,
                n,
                result,
            } if cwd == self.project().cwd => self.on_rewound(thread, n, result, cx),
            Notice::Restored { .. } | Notice::Rewound { .. } => {}
            Notice::CommitDraft { thread, draft } => self.on_commit_draft(thread, draft, cx),
            Notice::Committed { thread, result } => self.on_committed(thread, result, cx),
            Notice::PrPlan { thread, plan } => self.on_pr_plan(thread, plan, cx),
            Notice::PrOpened { thread, result } => self.on_pr_opened(thread, result, cx),
            Notice::Background {
                cwd,
                schedules,
                mcp,
            } => {
                if let Some(project) = self.projects.iter_mut().find(|p| p.cwd == cwd) {
                    project.background = Some(crate::project::Background { schedules, mcp });
                    cx.notify();
                }
            }
            Notice::Settings { path, desktop } => {
                let settings = Desktop::from_value(&desktop);
                settings.apply(cx);
                self.sound = settings.sound;
                self.agent.update(cx, |agent, cx| {
                    agent.set_view_mode(settings.view_mode, cx);
                    agent.set_density(settings.density, cx);
                });
                self.settings
                    .update(cx, |view, cx| view.set(path, desktop, cx));
            }
            Notice::Threads { cwd, threads } => {
                let Some(n) = self.projects.iter().position(|p| p.cwd == cwd) else {
                    return;
                };
                let project = &mut self.projects[n];
                // Threads in other tabs get no state notices, only lists: their runs ring here.
                for ending in project.set_threads(threads) {
                    self.sound.play(ending);
                }
                if std::mem::take(&mut project.first_list)
                    && project.current.is_none()
                    && !project.awaiting_thread
                {
                    let saved = self.layout.open.get(&cwd.display().to_string());
                    let thread = project
                        .threads
                        .iter()
                        .find(|t| Some(&t.thread.id) == saved)
                        .or(project.threads.iter().find(|t| !crate::project::is_chat(t)))
                        .map(|t| (t.thread.id.clone(), t.status));
                    match thread {
                        Some((id, _)) if n == self.active => self.select(id, cx),
                        // A tab behind opens its thread when it comes to the front.
                        Some((id, status)) => {
                            project.current = Some(id);
                            project.status = Some(status);
                        }
                        None => {}
                    }
                }
                if n == self.active {
                    self.open_chat_thread(cx);
                    self.refresh_knowledge();
                }
                self.retitle(cx);
            }
            Notice::Preparing { cwd, step } => {
                if let Some(project) = self
                    .projects
                    .iter_mut()
                    .find(|p| p.cwd == cwd && p.awaiting_thread)
                {
                    project.preparing = Some(step);
                    self.retitle(cx);
                }
            }
            Notice::Started { cwd, thread }
                if self
                    .projects
                    .iter()
                    .any(|p| p.cwd == cwd && p.awaiting_chat) =>
            {
                // The project chat's first message started its thread.
                if let Some(project) = self
                    .projects
                    .iter_mut()
                    .find(|p| p.cwd == cwd && p.awaiting_chat)
                {
                    project.awaiting_chat = false;
                    project.chat_open = Some(thread);
                }
            }
            Notice::Started { cwd, thread } => {
                let Some(n) = self
                    .projects
                    .iter()
                    .position(|p| p.cwd == cwd && p.awaiting_thread)
                else {
                    return;
                };
                let project = &mut self.projects[n];
                project.awaiting_thread = false;
                project.preparing = None;
                project.current = Some(thread.clone());
                project.status = Some(Status::Working);
                self.choices.started(&thread);
                if n == self.active {
                    self.retitle(cx);
                    self.review_current(cx);
                } else {
                    // Started from a tab now behind: it reads the lines when it is in front.
                    self.request(Request::Close {
                        thread: thread.clone(),
                    });
                    self.layout.open.insert(cwd.display().to_string(), thread);
                    self.save_layout(cx);
                    self.retitle(cx);
                }
            }
            Notice::Line { thread, line } if self.project().current.as_ref() == Some(&thread) => {
                match Line::parse(&line) {
                    Ok(line) => self.agent.update(cx, |agent, cx| agent.apply(&line, cx)),
                    Err(error) => log::warn!("{thread}: {error}"),
                }
            }
            Notice::Line { thread, line } if self.project().chat_open.as_ref() == Some(&thread) => {
                match Line::parse(&line) {
                    Ok(line) => self.chat.update(cx, |chat, cx| chat.apply(&line, cx)),
                    Err(error) => log::warn!("{thread}: {error}"),
                }
            }
            Notice::Line { .. } | Notice::Caught { .. } => {}
            Notice::Status { thread, status } => {
                for project in &mut self.projects {
                    if let Some(info) = project.threads.iter_mut().find(|t| t.thread.id == thread) {
                        if let Some(ending) = sound::ending(Some(info.status), status) {
                            self.sound.play(ending);
                        }
                        info.status = status;
                    }
                    if project.current.as_ref() == Some(&thread) {
                        project.status = Some(status);
                    }
                }
                if self.project().current.as_ref() == Some(&thread) {
                    self.retitle(cx);
                    self.refresh_review();
                    // A run that ended may have added or removed files.
                    if status != Status::Working {
                        self.files.update(cx, |files, cx| files.refresh(cx));
                        self.git.update(cx, |git, cx| git.refresh(cx));
                    }
                }
            }
            Notice::Review { thread, review }
                if self.project().current.as_ref() == Some(&thread) =>
            {
                self.waiting_hunks = review
                    .files
                    .iter()
                    .flat_map(|f| &f.hunks)
                    .filter(|h| h.state == HunkState::Pending)
                    .count();
                self.review
                    .update(cx, |pane, cx| pane.set_review(review, cx));
            }
            Notice::Review { .. } => {}
            Notice::Usage { cwd, since, report } => self
                .usage
                .update(cx, |pane, cx| pane.set_report(cwd, since, *report, cx)),
            Notice::Mcp { list, refused } => self
                .settings
                .update(cx, |view, cx| view.set_mcp(list, refused, cx)),
            Notice::McpChecked { check } => self
                .settings
                .update(cx, |view, cx| view.set_mcp_check(check, cx)),
            Notice::Browser { cwd, page } => self
                .browser
                .update(cx, |pane, cx| pane.set_page(cwd, page, cx)),
            Notice::Outputs { cwd, outputs } if cwd == self.project().cwd => {
                self.outputs.update(cx, |pane, cx| pane.set(outputs, cx));
            }
            Notice::Memory { cwd, notes } if cwd == self.project().cwd => {
                self.memory.update(cx, |pane, cx| pane.set(notes, cx));
            }
            Notice::Outputs { .. } | Notice::Memory { .. } => {}
            Notice::Error { message } => {
                // A new thread that could not start: the composer is free again.
                for project in &mut self.projects {
                    project.awaiting_thread = false;
                    project.preparing = None;
                }
                self.retitle(cx);
                self.error = Some(message.into());
            }
        }
        cx.notify();
    }

    fn project(&self) -> &Project {
        &self.projects[self.active]
    }

    fn project_mut(&mut self) -> &mut Project {
        &mut self.projects[self.active]
    }

    fn retitle(&mut self, cx: &mut Context<Self>) {
        let project = self.project();
        let info = project.info();
        let title = match &project.current {
            Some(id) => info
                .map_or(id.as_str(), |t| t.thread.title.as_str())
                .to_owned(),
            None if project.awaiting_thread => match &project.preparing {
                Some(step) => format!("new thread · {step}"),
                None => "new thread · starting".to_owned(),
            },
            None => "new thread".to_owned(),
        };
        let branch = info
            .and_then(|t| t.cwd.as_deref())
            .and_then(crate::agent_pane::branch_of);
        // The search finds threads of every tab, this one's first, archived ones too.
        let found = std::iter::once(project)
            .chain(self.projects.iter().filter(|p| p.cwd != project.cwd))
            .flat_map(|p| {
                let elsewhere = (p.cwd != project.cwd).then(|| p.name());
                p.threads
                    .iter()
                    .chain(&p.archived)
                    .map(move |t| Found::of(t, &p.cwd, elsewhere.clone()))
            })
            .collect();
        let dir = project.shell_dir();
        self.files
            .update(cx, |files, cx| files.set_root(dir.clone(), cx));
        self.git.update(cx, |git, cx| git.set_dir(dir.clone(), cx));
        let header = Header {
            title: title.clone().into(),
            status: project.status,
            rating: info.and_then(|t| t.thread.rating),
        };
        self.agent
            .update(cx, |agent, cx| agent.set_header(header, cx));
        let queue = info.map(queued).unwrap_or_default();
        self.search.update(cx, |search, cx| search.set(found, cx));
        self.workplace
            .update(cx, |workplace, cx| workplace.set(branch, dir, cx));
        self.show_offers(cx);
        let working = self.project().status == Some(Status::Working);
        self.composer.update(cx, |composer, cx| {
            composer.set_queue(queue, cx);
            composer.set_working(working, cx);
        });
    }

    /// What the open thread's messages and latest receipt offer. A fork or a rewind goes back
    /// to a run's start and an undo moves its files; a run still going would race each.
    fn show_offers(&mut self, cx: &mut Context<Self>) {
        let offers = self
            .project()
            .info()
            .filter(|t| t.status != Status::Working)
            .map(|t| crate::agent_pane::RunOffers {
                prompts: t.thread.runs.iter().map(|r| r.prompt.clone()).collect(),
                edit: self.can_fork,
                rewind: self.can_restore,
                undone: t
                    .thread
                    .last_turn
                    .as_ref()
                    .filter(|_| self.can_restore)
                    .map(|turn| self.undone.get(&t.thread.id) == Some(&turn.after)),
                commit: self.can_commit,
                pr: self.can_pr,
            })
            .unwrap_or_default();
        self.agent
            .update(cx, |agent, cx| agent.set_offers(offers, cx));
    }

    /// The person's message `text` goes in the composer to edit; sending it forks the thread
    /// at the run it started (the latest whose prompt it is).
    fn edit_sent(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(info) = self.project().info() else {
            return;
        };
        let Some(run) = info
            .thread
            .runs
            .iter()
            .rev()
            .find(|r| !r.prompt.is_empty() && text.starts_with(r.prompt.as_str()))
        else {
            return;
        };
        let (editing, prompt) = ((info.thread.id.clone(), run.id.clone()), run.prompt.clone());
        self.editing = Some(editing);
        self.composer
            .update(cx, |composer, cx| composer.edit_sent(&prompt, window, cx));
    }

    fn select(&mut self, thread: String, cx: &mut Context<Self>) {
        if self.project().current.as_ref() == Some(&thread) {
            return;
        }
        if let Some(old) = self.project_mut().current.take() {
            self.request(Request::Close { thread: old });
        }
        if self.editing.take().is_some() {
            self.composer
                .update(cx, |composer, cx| composer.forget_edit(cx));
        }
        let project = self.project_mut();
        project.awaiting_thread = false;
        project.preparing = None;
        project.status = None;
        project.current = Some(thread);
        self.show_current(cx);
    }

    /// Puts the front tab's thread in the agent pane, read again from the daemon.
    fn show_current(&mut self, cx: &mut Context<Self>) {
        let project = self.project();
        let status = project.info().map(|t| t.status).or(project.status);
        let cwd = project.cwd.clone();
        match project.current.clone() {
            Some(thread) => {
                self.agent
                    .update(cx, |agent, cx| agent.reset(thread.clone(), cx));
                self.request(Request::Open { cwd, thread });
            }
            None => self
                .agent
                .update(cx, |agent, cx| agent.reset("new thread", cx)),
        }
        self.project_mut().status = status;
        self.waiting_hunks = 0;
        self.retitle(cx);
        self.review_current(cx);
    }

    /// A thread the omnibox found, in whichever tab holds it.
    fn open_thread(&mut self, cwd: &Path, thread: String, cx: &mut Context<Self>) {
        if let Some(n) = self.projects.iter().position(|p| p.cwd == cwd) {
            self.activate(n, cx);
            self.select(thread, cx);
        }
    }

    /// Brings tab `n` to the front; the tab it leaves stops reading its thread's lines.
    fn activate(&mut self, n: usize, cx: &mut Context<Self>) {
        if n == self.active || n >= self.projects.len() {
            return;
        }
        if let Some(old) = self.project().current.clone() {
            self.request(Request::Close { thread: old });
        }
        if let Some(chat) = self.project_mut().chat_open.take() {
            self.request(Request::Close { thread: chat });
        }
        self.active = n;
        // A notice was about the tab left behind.
        self.error = None;
        self.zoomed = None;
        self.block_menu = None;
        self.focused = self.ws().find(Kind::Agent);
        self.show_current(cx);
        self.open_chat_thread(cx);
        self.reset_knowledge(cx);
        self.set_search_actions(cx);
        self.remember_tabs();
        self.save_layout(cx);
        cx.notify();
    }

    /// Opens `dir` in a tab of its own, or brings its tab to the front.
    fn open_tab(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        let dir = match dir.canonicalize() {
            Ok(dir) if dir.is_dir() => dir,
            Ok(_) | Err(_) => {
                self.error = Some(format!("{} is not a folder", dir.display()).into());
                cx.notify();
                return;
            }
        };
        let n = match self.projects.iter().position(|p| p.cwd == dir) {
            Some(n) => n,
            None => {
                self.request(Request::Threads { cwd: dir.clone() });
                self.request(Request::Background { cwd: dir.clone() });
                let mut project = Project::new(dir);
                self.load_blocks(&mut project);
                self.projects.push(project);
                self.projects.len() - 1
            }
        };
        self.activate(n, cx);
        self.remember_tabs();
        self.save_layout(cx);
    }

    /// Writes the open thread as Markdown to a file the person picks, then shows it in the
    /// file manager. The text comes from the transcript the pane already holds.
    fn export_thread(&mut self, cx: &mut Context<Self>) {
        let Some(info) = self.project().info() else {
            return;
        };
        let text = export::markdown(
            &info.thread.title,
            &info.thread.id,
            self.agent.read(cx).transcript().blocks(),
        );
        let name = export::file_name(&info.thread.title);
        let home = std::env::home_dir().unwrap_or_else(|| self.project().cwd.clone());
        let downloads = home.join("Downloads");
        let dir = if downloads.is_dir() { downloads } else { home };
        let picked = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            let saved = match picked.await {
                Ok(Ok(Some(path))) => std::fs::write(&path, text)
                    .map(|()| path)
                    .map_err(anyhow::Error::from),
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => Err(error),
            };
            this.update(cx, |view, cx| {
                match saved {
                    Ok(path) => cx.reveal_path(&path),
                    Err(error) => {
                        log::warn!("exporting the thread: {error:#}");
                        view.error = Some(format!("ส่งออก thread ไม่ได้: {error:#}").into());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `⌘T` and `+`: the system's folder picker, and a tab for the folder picked.
    fn pick_project(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("เปิดโปรเจกต์".into()),
        });
        cx.spawn(async move |this, cx| {
            let dir = match picked.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    log::warn!("opening the folder picker: {error:#}");
                    this.update(cx, |view, cx| {
                        view.error = Some(format!("เปิดหน้าต่างเลือกโฟลเดอร์ไม่ได้: {error:#}").into());
                        cx.notify();
                    })
                    .ok();
                    None
                }
            };
            if let Some(dir) = dir {
                this.update(cx, |view, cx| view.open_tab(dir, cx)).ok();
            }
        })
        .detach();
    }

    /// Closes tab `n`, and its shells with it; the last tab stays.
    fn close_tab(&mut self, n: usize, cx: &mut Context<Self>) {
        if self.projects.len() < 2 || n >= self.projects.len() {
            return;
        }
        if n == self.active
            && let Some(old) = self.project().current.clone()
        {
            self.request(Request::Close { thread: old });
        }
        self.projects.remove(n);
        if n < self.active {
            self.active -= 1;
        } else if n == self.active {
            self.active = n.min(self.projects.len() - 1);
            self.show_current(cx);
        }
        self.remember_tabs();
        self.save_layout(cx);
        cx.notify();
    }

    fn click_tab(&mut self, click: TabClick, cx: &mut Context<Self>) {
        match click {
            TabClick::Activate(n) => self.activate(n, cx),
            TabClick::Close(n) => self.close_tab(n, cx),
            TabClick::New => self.pick_project(cx),
        }
    }

    /// A tab dragged from `from` and dropped on the tab at `to`.
    fn move_tab(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from == to || from >= self.projects.len() || to >= self.projects.len() {
            return;
        }
        tab_strip::moved(&mut self.projects, from, to);
        self.active = tab_strip::follow(self.active, from, to);
        self.remember_tabs();
        self.save_layout(cx);
        cx.notify();
    }

    /// Puts the tabs in the layout the next window opens with.
    fn remember_tabs(&mut self) {
        self.layout.projects = self
            .projects
            .iter()
            .map(|p| p.cwd.display().to_string())
            .collect();
        self.layout.project = Some(self.project().cwd.display().to_string());
    }

    fn new_thread(&mut self, cx: &mut Context<Self>) {
        if let Some(old) = self.project_mut().current.take() {
            self.request(Request::Close { thread: old });
        }
        let project = self.project_mut();
        project.awaiting_thread = false;
        project.preparing = None;
        project.status = None;
        self.agent
            .update(cx, |agent, cx| agent.reset("new thread", cx));
        self.review_current(cx);
    }

    /// Writes the layout through the daemon half a second after the last change, so dragging
    /// the window writes the file once.
    fn save_layout(&mut self, cx: &mut Context<Self>) {
        if matches!(self.connection, Connection::Replay) || self.torn {
            return;
        }
        self.saving_layout = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            this.update(cx, |view, _| {
                view.request(Request::SetDesktop {
                    key: "layout".into(),
                    value: view.layout.to_value(),
                });
            })
            .ok();
        }));
    }

    /// Starts the shell for the terminal's directory the first time it is shown there. Each
    /// directory keeps its shell, so going back to a thread finds its shell as it was left.
    fn ensure_shell(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dir = self.project().shell_dir();
        if self.project().shells.contains_key(&dir) {
            return;
        }
        let start = (!dir.as_os_str().is_empty()).then(|| dir.clone());
        let shell = cx.new(|cx| ShellPane::new(start, window, cx));
        cx.subscribe_in(&shell, window, |view, _, event, window, cx| match event {
            ShellEvent::Attach(text) => view
                .composer
                .update(cx, |composer, cx| composer.attach(text, window, cx)),
        })
        .detach();
        self.project_mut().shells.insert(dir, shell);
    }

    fn attach_editor_selection(
        &mut self,
        _: &AttachSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.editor.update(cx, |editor, cx| {
            let range = editor.selected_text_range(false, window, cx)?.range;
            (!range.is_empty())
                .then(|| editor.text_for_range(range, &mut None, window, cx))
                .flatten()
        });
        if let Some(text) = text.filter(|t| !t.trim().is_empty()) {
            self.composer
                .update(cx, |composer, cx| composer.attach(&text, window, cx));
        }
    }

    /// Opens `⌘P` on the open thread's directory, or closes it when it is open.
    fn find_file(&mut self, _: &FindFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.finder.is_some() {
            self.close_finder(window, cx);
            return;
        }
        self.contents = None;
        let (root, files) = {
            let tree = self.files.read(cx);
            (tree.root().to_owned(), tree.files().to_vec())
        };
        if files.is_empty() {
            self.files.update(cx, |tree, cx| tree.refresh(cx));
        }
        let finder = cx.new(|cx| FileFinder::new(root, &files, window, cx));
        cx.subscribe_in(&finder, window, |view, _, event, window, cx| match event {
            FinderEvent::Open(path) => {
                let path = path.clone();
                view.close_finder(window, cx);
                view.show_file(path, window, cx);
            }
            FinderEvent::Closed => view.close_finder(window, cx),
        })
        .detach();
        window.focus(&finder.focus_handle(cx), cx);
        self.finder = Some(finder);
        cx.notify();
    }

    /// Opens `⌘⇧F` on the open thread's directory, or closes it when it is open.
    fn search_contents(&mut self, _: &SearchContents, window: &mut Window, cx: &mut Context<Self>) {
        if self.contents.is_some() {
            self.close_finder(window, cx);
            return;
        }
        self.finder = None;
        let (root, files) = {
            let tree = self.files.read(cx);
            (tree.root().to_owned(), tree.files().to_vec())
        };
        let search = cx.new(|cx| ContentSearch::new(root, files, window, cx));
        cx.subscribe_in(&search, window, |view, _, event, window, cx| match event {
            ContentEvent::Open { path, line } => {
                let (path, line) = (path.clone(), *line);
                view.close_finder(window, cx);
                view.show_file(path, window, cx);
                view.editor.update(cx, |editor, cx| {
                    let row = line.saturating_sub(1);
                    editor.go_to_singleton_buffer_point(text::Point::new(row, 0), window, cx);
                });
            }
            ContentEvent::Closed => view.close_finder(window, cx),
        })
        .detach();
        window.focus(&search.focus_handle(cx), cx);
        self.contents = Some(search);
        cx.notify();
    }

    /// Closes `⌘P` or `⌘⇧F`, whichever is open.
    fn close_finder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.finder.take().is_some() | self.contents.take().is_some();
        if open {
            self.focus_input(window, cx);
            cx.notify();
        }
    }

    /// The thread's question, with a button per option; any other answer goes in the composer.
    fn render_question(
        &self,
        question: cedian_index::ThreadQuestion,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let tokens = tokens::Tokens::of(cx);
        let mut options = div().flex().flex_wrap().gap_2();
        for (n, option) in question.options.into_iter().enumerate() {
            let answer = option.clone();
            let recommended = n == 0;
            options = options.child(
                div()
                    .id(("answer", n))
                    .h(px(28.))
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded(px(7.))
                    .border_1()
                    .cursor_pointer()
                    .text_ui()
                    .map(|button| {
                        if recommended {
                            button
                                .bg(tokens.primary)
                                .border_color(tokens.primary)
                                .text_color(tokens.on_primary)
                        } else {
                            button
                                .bg(colors.element_background)
                                .border_color(colors.border)
                                .text_color(colors.text)
                                .hover(|b| b.bg(colors.element_hover))
                        }
                    })
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.submit(answer.clone(), Vec::new(), cx)
                    }))
                    .child(if recommended {
                        format!("{option} · แนะนำ")
                    } else {
                        option
                    }),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .mx_3()
            .my_2()
            .p_3()
            .rounded(px(12.))
            .border_1()
            .border_color(colors.text_accent)
            .bg(colors.element_background)
            .text_color(colors.text)
            .child(
                div()
                    .text_label()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.text_accent)
                    .child("ถามคุณ"),
            )
            .child(
                div()
                    .text_message()
                    .child(line_breaks::wrapped(&question.question)),
            )
            .child(options)
            .child(
                div()
                    .text_meta()
                    .text_color(colors.text_muted)
                    .child("หรือพิมพ์คำตอบอื่นด้านล่าง"),
            )
            .into_any_element()
    }

    /// Points the review at the open thread.
    fn review_current(&mut self, cx: &mut Context<Self>) {
        let cwd = self.project().cwd.display().to_string();
        let changed = match &self.projects[self.active].current {
            Some(thread) => self.layout.open.insert(cwd, thread.clone()).as_ref() != Some(thread),
            None => false,
        };
        if changed {
            self.save_layout(cx);
        }
        let thread = self.project().current.clone();
        self.review
            .update(cx, |pane, cx| pane.set_thread(thread, cx));
        self.refresh_review();
    }

    fn refresh_review(&self) {
        // Asked for even while the review is closed: the rail's badge counts its hunks.
        if let Some(thread) = &self.project().current {
            self.request(Request::Review {
                cwd: self.project().cwd.clone(),
                thread: thread.clone(),
            });
        }
    }

    /// Archives a thread (`cedia threads archive`): it leaves the list, and the sidebar offers
    /// to bring it back for a few seconds. The thread open in the pane gives way to a new one.
    fn archive(&mut self, thread: &str, cx: &mut Context<Self>) {
        let project = self.project();
        let cwd = project.cwd.clone();
        let title = project
            .threads
            .iter()
            .find(|t| t.thread.id == thread)
            .map_or_else(|| thread.to_owned(), |t| t.thread.title.clone());
        self.request(Request::Archive {
            cwd: cwd.clone(),
            thread: thread.to_owned(),
            archived: true,
        });
        if self.project().current.as_deref() == Some(thread) {
            self.new_thread(cx);
        }
        let expiry = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(UNDO_FOR).await;
            this.update(cx, |view, cx| {
                view.archived = None;
                cx.notify();
            })
            .ok();
        });
        self.archived = Some(Archived {
            cwd,
            thread: thread.to_owned(),
            title,
            _expiry: expiry,
        });
        cx.notify();
    }

    fn unarchive(&mut self, cx: &mut Context<Self>) {
        if let Some(archived) = self.archived.take() {
            self.bring_back(archived.cwd, archived.thread, cx);
        }
    }

    /// Takes a thread out of the archive, so it is listed with the rest again.
    fn bring_back(&mut self, cwd: PathBuf, thread: String, cx: &mut Context<Self>) {
        if self
            .archived
            .as_ref()
            .is_some_and(|archived| archived.thread == thread)
        {
            self.archived = None;
        }
        self.request(Request::Archive {
            cwd,
            thread,
            archived: false,
        });
        cx.notify();
    }

    fn is_pinned(&self, thread: &str) -> bool {
        self.layout
            .is_pinned(&self.project().cwd.display().to_string(), thread)
    }

    fn toggle_pin(&mut self, thread: &str, cx: &mut Context<Self>) {
        let cwd = self.project().cwd.display().to_string();
        self.layout.toggle_pin(&cwd, thread);
        self.save_layout(cx);
        cx.notify();
    }

    fn submit(&mut self, text: String, images: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.client.is_none() {
            self.error = Some("not connected to the daemon".into());
            cx.notify();
            return;
        }
        self.error = None;
        if let Some((thread, run)) = self.editing.take()
            && self.project().current.as_ref() == Some(&thread)
        {
            let writer = self.writer_model(cx);
            let (model, permission, effort) =
                self.choices.for_run(Some(&thread), writer.as_deref());
            let helpers = self.choices.helpers_for_run(Some(&thread), &text);
            self.request(Request::Fork {
                cwd: self.project().cwd.clone(),
                thread,
                run,
                text,
                model,
                permission,
                effort,
                helpers,
            });
            cx.notify();
            return;
        }
        if self.project().current.is_none() {
            if self.project().awaiting_thread {
                self.error = Some("the new thread has not started yet".into());
                cx.notify();
                return;
            }
            self.project_mut().awaiting_thread = true;
        }
        let writer = self.writer_model(cx);
        let current = self.project().current.clone();
        let (model, permission, effort) =
            self.choices.for_run(current.as_deref(), writer.as_deref());
        let helpers = self.choices.helpers_for_run(current.as_deref(), &text);
        let project = self.project();
        self.request(Request::Prompt {
            cwd: project.cwd.clone(),
            text,
            thread: project.current.clone(),
            worktree: project.current.is_none() && self.use_worktree,
            profile: None,
            model,
            permission,
            effort,
            helpers,
            images,
        });
        self.retitle(cx);
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(thread) = self.project().current.clone() {
            self.request(Request::Stop { thread });
        }
        cx.notify();
    }

    /// The Threads block: the search, the project chat's row, the groups, the archived ones
    /// and the foot. A narrow block keeps each row's dot and title.
    fn render_threads(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let chat_row = self.render_chat_row(cx);
        let archived = self.render_archived(cx);
        let foot = self.render_foot(cx);
        let searching = self.search.read(cx).searching(cx);
        let colors = cx.theme().colors().clone();
        let status_colors = cx.theme().status().clone();
        let now = chrono::Utc::now();
        let mut list = div()
            .id("threads")
            .flex()
            .flex_col()
            .flex_grow(1.)
            .px_2()
            .pb_2()
            .overflow_y_scroll();
        for group in Group::ALL {
            let members: Vec<&ThreadInfo> = self
                .project()
                .threads
                .iter()
                .filter(|t| blocks::in_groups(t))
                .filter(|t| Group::of(t, self.is_pinned(&t.thread.id)) == group)
                .collect();
            if members.is_empty() {
                continue;
            }
            let counted = members.iter().filter(|t| t.needs_you).count();
            let failed = members
                .iter()
                .any(|t| t.needs_you && t.status == Status::Failed);
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .pt_3()
                    .pb_1()
                    .text_label()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.text_placeholder)
                    .child(group.label())
                    .when(counted > 0, |header| {
                        header.child(
                            div()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(if failed {
                                    status_colors.error
                                } else {
                                    colors.text
                                })
                                .child(counted.to_string()),
                        )
                    }),
            );
            for info in members {
                let id = info.thread.id.clone();
                let pin_id = id.clone();
                let archive_id = id.clone();
                let rename_id = id.clone();
                let delete_id = id.clone();
                let archivable = info.status != Status::Working;
                let rename_field = self.render_rename_field(&id, cx);
                let delete_confirm = self.render_delete_confirm(&id, cx);
                let editing = rename_field.is_some() || delete_confirm.is_some();
                let can_rename = self.can_rename;
                let can_delete = self.can_delete && archivable;
                let pinned = self.is_pinned(&id);
                let selected = self.project().current.as_ref() == Some(&id);
                // One line: the dot says the state; the meta says what a word must.
                let done = info
                    .thread
                    .todo
                    .iter()
                    .filter(|step| step.state == TodoState::Done)
                    .count();
                let progress = (!info.thread.todo.is_empty())
                    .then(|| format!("{done}/{}", info.thread.todo.len()));
                let (meta, meta_color) = match info.status {
                    // A todo list gives a thread at work its progress, `2/4`.
                    Status::Queued | Status::Working => (
                        progress.unwrap_or_else(|| {
                            if info.status == Status::Queued {
                                "รอ".to_owned()
                            } else {
                                String::new()
                            }
                        }),
                        colors.text_muted,
                    ),
                    Status::Asking => ("ถาม".to_owned(), colors.text_accent),
                    Status::Failed => ("ล้มเหลว".to_owned(), status_colors.error),
                    Status::Interrupted => ("ขาดตอน".to_owned(), status_colors.warning),
                    Status::Done | Status::Stopped => (
                        thread_list::since(&info.thread.updated_at, now),
                        colors.text_placeholder,
                    ),
                };
                list = list.child(
                    div()
                        .id(SharedString::from(id.clone()))
                        .flex()
                        .items_center()
                        .gap_2()
                        .relative()
                        .h(px(32.))
                        .flex_none()
                        .px_2()
                        .rounded(px(8.))
                        .cursor_pointer()
                        .group("thread-row")
                        .when(selected, |row| row.bg(colors.element_selected))
                        .hover(|row| row.bg(colors.element_hover))
                        .on_click(cx.listener(move |view, _, _, cx| view.select(id.clone(), cx)))
                        .child(thread_list::dot(info.status, cx))
                        .children(rename_field)
                        .children(delete_confirm)
                        .when(!editing, |row| {
                            row.child(
                                crate::tooltip::cut("title", info.thread.title.clone())
                                    .flex_grow(1.)
                                    .text_ui()
                                    .when(info.needs_you, |title| {
                                        title.font_weight(gpui::FontWeight::SEMIBOLD)
                                    }),
                            )
                            .when(!meta.is_empty() && !narrow, |row| {
                                row.child(
                                    div()
                                        .flex_none()
                                        .font(tokens::numeric_font())
                                        .text_label()
                                        .text_color(meta_color)
                                        .child(meta),
                                )
                            })
                            // Pin, rename, archive and delete, over the meta while the pointer is on the row.
                            .child(
                                div()
                                    .absolute()
                                    .right(px(4.))
                                    .top_0()
                                    .h_full()
                                    .pl_1()
                                    .flex()
                                    .items_center()
                                    .gap_0p5()
                                    .bg(colors.background)
                                    .rounded(px(6.))
                                    .opacity(0.)
                                    .group_hover("thread-row", |b| b.opacity(1.))
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("pin-{pin_id}")))
                                            .p_1()
                                            .rounded(px(4.))
                                            .hover(|pin| pin.bg(colors.element_selected))
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                cx.stop_propagation();
                                                view.toggle_pin(&pin_id, cx)
                                            }))
                                            .child(
                                                svg()
                                                    .path(if pinned {
                                                        "icons/unpin.svg"
                                                    } else {
                                                        "icons/pin.svg"
                                                    })
                                                    .size(px(12.))
                                                    .text_color(colors.text_muted),
                                            ),
                                    )
                                    .when(can_rename, |actions| {
                                        actions.child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "rename-{rename_id}"
                                                )))
                                                .debug_selector({
                                                    let id = rename_id.clone();
                                                    move || format!("thread-rename-{id}")
                                                })
                                                .p_1()
                                                .rounded(px(4.))
                                                .hover(|b| b.bg(colors.element_selected))
                                                .on_click(cx.listener(
                                                    move |view, _, window, cx| {
                                                        cx.stop_propagation();
                                                        view.start_rename(&rename_id, window, cx)
                                                    },
                                                ))
                                                .child(
                                                    svg()
                                                        .path("icons/pencil.svg")
                                                        .size(px(12.))
                                                        .text_color(colors.text_muted),
                                                ),
                                        )
                                    })
                                    // A working thread is not archived from here: stop it first.
                                    .when(archivable, |actions| {
                                        actions.child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "archive-{archive_id}"
                                                )))
                                                .p_1()
                                                .rounded(px(4.))
                                                .hover(|b| b.bg(colors.element_selected))
                                                .on_click(cx.listener(move |view, _, _, cx| {
                                                    cx.stop_propagation();
                                                    view.archive(&archive_id, cx)
                                                }))
                                                .child(
                                                    svg()
                                                        .path("icons/archive.svg")
                                                        .size(px(12.))
                                                        .text_color(colors.text_muted),
                                                ),
                                        )
                                    })
                                    // Deleting asks in the row first; a working thread is stopped first.
                                    .when(can_delete, |actions| {
                                        actions.child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "delete-{delete_id}"
                                                )))
                                                .debug_selector({
                                                    let id = delete_id.clone();
                                                    move || format!("thread-delete-{id}")
                                                })
                                                .p_1()
                                                .rounded(px(4.))
                                                .hover(|b| b.bg(colors.element_selected))
                                                .on_click(cx.listener(move |view, _, _, cx| {
                                                    cx.stop_propagation();
                                                    view.ask_delete(&delete_id, cx)
                                                }))
                                                .child(
                                                    svg()
                                                        .path("icons/trash.svg")
                                                        .size(px(12.))
                                                        .text_color(colors.text_muted),
                                                ),
                                        )
                                    }),
                            )
                            .when(info.needs_you, |row| {
                                row.child(div().flex_none().size(px(6.)).rounded_full().bg(
                                    if info.status == Status::Failed {
                                        status_colors.error
                                    } else {
                                        colors.text
                                    },
                                ))
                            })
                        }),
                );
            }
        }
        div()
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .text_color(colors.text)
            .child(div().flex_none().child(self.search.clone()))
            .map(|sidebar| {
                if searching {
                    sidebar.child(div().flex_grow(1.))
                } else {
                    sidebar.child(chat_row).child(list)
                }
            })
            .child(archived)
            .child(foot)
            .when_some(self.archived.as_ref(), |sidebar, archived| {
                sidebar.child(
                    div()
                        .flex_none()
                        .mx_2()
                        .mb_2()
                        .px_3()
                        .py_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .rounded(px(8.))
                        .bg(colors.element_background)
                        .text_meta()
                        .child(
                            div()
                                .flex_grow(1.)
                                .min_w_0()
                                .truncate()
                                .child(format!("เก็บ “{}” แล้ว", archived.title)),
                        )
                        .child(
                            div()
                                .id("unarchive")
                                .flex_none()
                                .cursor_pointer()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .on_click(cx.listener(|view, _, _, cx| view.unarchive(cx)))
                                .child("เลิกทำ"),
                        ),
                )
            })
            .into_any_element()
    }
}

impl RootView {
    /// The sidebar's last line: the daemon, the runs and Settings.
    fn render_foot(&self, cx: &mut Context<Self>) -> gpui::Div {
        // A narrow thread list leaves cedia's version to Settings.
        let full = self
            .ws()
            .find(Kind::Threads)
            .and_then(|id| self.block_rect(id))
            .is_none_or(|r| r.w >= blocks::FOOT_FULL);
        let (daemon, connected, cedia): (SharedString, bool, Option<SharedString>) =
            match &self.connection {
                Connection::Replay => ("replay".into(), false, None),
                Connection::Connecting => ("กำลังต่อ daemon".into(), false, None),
                Connection::Connected { cedia } => ("daemon".into(), true, Some(cedia.clone())),
                Connection::Failed(_) => ("ต่อ daemon ไม่ได้".into(), false, None),
            };
        sidebar_foot::foot(
            StatusLine {
                daemon,
                connected,
                cedia: cedia.filter(|_| full),
                working: self.projects.iter().map(Project::working).sum(),
                waiting: self.projects.iter().map(Project::waiting).sum(),
            },
            div()
                .id("open-settings")
                .on_click(cx.listener(|view, _, _, cx| view.open_settings(cx)))
                .child("Settings"),
            cx,
        )
    }

    /// The foot of the sidebar: how many threads are archived, and when opened, each with a way
    /// to bring it back.
    fn render_archived(&self, cx: &mut Context<Self>) -> gpui::Div {
        let colors = cx.theme().colors().clone();
        let archived = &self.project().archived;
        let now = chrono::Utc::now();
        let open = self.show_archived && !archived.is_empty();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .px_2()
            .pb_2()
            .when(!archived.is_empty(), |foot| {
                foot.border_t_1().border_color(colors.border)
            })
            .when(open, |foot| {
                foot.child(
                    div()
                        .id("archived-threads")
                        .max_h(px(240.))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .children(archived.iter().map(|info| {
                            let cwd = self.project().cwd.clone();
                            let id = info.thread.id.clone();
                            div()
                                .id(SharedString::from(format!("archived-{id}")))
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py(px(5.))
                                .rounded(px(8.))
                                .group("archived-row")
                                .hover(|row| row.bg(colors.element_hover))
                                .child(
                                    div()
                                        .flex_grow(1.)
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            crate::tooltip::cut("title", info.thread.title.clone())
                                                .text_ui()
                                                .text_color(colors.text_muted),
                                        )
                                        .child(
                                            div()
                                                .text_meta()
                                                .text_color(colors.text_placeholder)
                                                .child(thread_list::since(
                                                    &info.thread.updated_at,
                                                    now,
                                                )),
                                        ),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("bring-back-{id}")))
                                        .flex_none()
                                        .px_1()
                                        .rounded(px(4.))
                                        .cursor_pointer()
                                        .text_meta()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .opacity(0.)
                                        .group_hover("archived-row", |b| b.opacity(1.))
                                        .hover(|b| b.bg(colors.element_selected))
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.bring_back(cwd.clone(), id.clone(), cx)
                                        }))
                                        .child("เอากลับ"),
                                )
                        })),
                )
            })
            .when(!archived.is_empty(), |foot| {
                foot.child(
                    div()
                        .id("show-archived")
                        .flex()
                        .items_center()
                        .gap_2()
                        .mt_1()
                        .px_2()
                        .h(px(28.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .text_meta()
                        .text_color(colors.text_muted)
                        .hover(|b| b.bg(colors.element_hover))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.show_archived = !view.show_archived;
                            cx.notify();
                        }))
                        .child(
                            svg()
                                .path("icons/archive.svg")
                                .size(px(12.))
                                .text_color(colors.text_muted),
                        )
                        .child(
                            div()
                                .flex_grow(1.)
                                .font(tokens::numeric_font())
                                .child(archived.len().to_string()),
                        )
                        .child(if open { "ซ่อน" } else { "ดู" }),
                )
            })
    }
}

/// The thread archived last, which the sidebar offers to bring back for a while.
struct Archived {
    cwd: PathBuf,
    thread: String,
    title: String,
    /// Clears the offer after a while.
    _expiry: Task<()>,
}

/// How long the sidebar offers to bring an archived thread back.
const UNDO_FOR: std::time::Duration = std::time::Duration::from_secs(10);

/// The person's messages the thread has not seen yet, oldest first.
fn queued(info: &ThreadInfo) -> Vec<Queued> {
    info.thread
        .messages
        .iter()
        .filter(|m| m.from == "user" && info.thread.pending.contains(&m.id))
        .map(|m| Queued {
            id: m.id.clone(),
            text: m.text.clone().into(),
        })
        .collect()
}

/// The window's bounds as the next window should be opened with them. A window opens with its
/// content at the size it is given, while the bounds a window reports include the title bar,
/// so the size saved is the content's: saving the reported one would grow the window by its
/// title bar at every relaunch.
fn saved_bounds(window: &Window) -> gpui::WindowBounds {
    let content = window.viewport_size();
    match window.window_bounds() {
        gpui::WindowBounds::Windowed(b) => {
            gpui::WindowBounds::Windowed(gpui::Bounds::new(b.origin, content))
        }
        other => other,
    }
}

impl RootView {
    /// The Agent block: the open thread's conversation, the notice, its question and the
    /// composer under the conversation's column. A narrow block drops the chips.
    fn render_agent(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let live = !matches!(self.connection, Connection::Replay);
        let question = self
            .project()
            .info()
            .filter(|t| t.status == Status::Asking)
            .and_then(|t| t.thread.question.clone())
            .map(|question| self.render_question(question, cx));
        let error_color = cx.theme().status().error;
        let failure = match &self.connection {
            Connection::Failed(error) => Some(error.clone()),
            _ => None,
        };
        let notice = failure.or_else(|| self.error.clone());
        let window = self
            .writer_model(cx)
            .and_then(|model| self.choices.context_window(&model));
        let chips = self
            .agent
            .read(cx)
            .chips(&self.agent.downgrade(), window, cx);
        let working = self.project().status == Some(Status::Working);
        let colors = cx.theme().colors().clone();
        div()
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .child(div().flex_grow(1.).min_h_0().child(self.agent.clone()))
            .when_some(notice, |column, notice| {
                column.child(
                    div()
                        .px_4()
                        .py_1()
                        .text_meta()
                        .text_color(error_color)
                        .child(notice),
                )
            })
            .children(question)
            .children(self.render_restore(cx))
            .children(self.render_ship(cx))
            .when(live, |column| {
                let worktree = self.project().current.is_none().then(|| {
                    let word = if self.use_worktree {
                        "worktree ใหม่: เปิด"
                    } else {
                        "worktree ใหม่: ปิด"
                    };
                    div()
                        .id("worktree")
                        .min_w_0()
                        .truncate()
                        .cursor_pointer()
                        .text_meta()
                        .text_color(colors.text_muted)
                        .hover(|w| w.text_color(colors.text))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.use_worktree = !view.use_worktree;
                            cx.notify();
                        }))
                        .child(word)
                });
                let stop = working.then(|| {
                    div()
                        .id("stop-run")
                        .flex_none()
                        .h(px(24.))
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_1()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(colors.border)
                        .cursor_pointer()
                        .text_meta()
                        .text_color(colors.text_muted)
                        .hover(|b| b.bg(colors.element_hover).text_color(colors.text))
                        .on_click(cx.listener(|view, _, _, cx| view.stop(cx)))
                        .child(div().size(px(8.)).rounded(px(2.)).bg(colors.text_muted))
                        .child("หยุด")
                        .child(
                            div()
                                .text_label()
                                .text_color(colors.text_placeholder)
                                .child(crate::agent_pane::STOP_KEY),
                        )
                });
                // Under the conversation's column, as wide as it with its margins.
                column.child(
                    div().w_full().flex_none().flex().justify_center().child(
                        div()
                            .w_full()
                            .min_w_0()
                            .max_w(px(COMPOSER_WIDTH))
                            .child(self.composer.clone())
                            .child(
                                // The composer's bottom row: where the thread runs, then the
                                // context and the view mode.
                                div()
                                    .debug_selector(|| "composer-row".into())
                                    .flex_none()
                                    .min_w_0()
                                    .px_4()
                                    .pb_3()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .overflow_hidden()
                                    .child(self.workplace.clone())
                                    .children(worktree)
                                    .child(self.render_run_choices(cx))
                                    // The chips keep their width: the model's name and
                                    // the workplace give way first.
                                    .child(
                                        div()
                                            .flex_grow(1.)
                                            .flex_shrink_0()
                                            .when(!narrow, |row| row.child(chips)),
                                    )
                                    .children(stop),
                            ),
                    ),
                )
            })
            .into_any_element()
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Text sizes are in rems, so the interface size setting scales all of it, as in Zed.
        window.set_rem_size(theme_settings::ThemeSettings::get_global(cx).ui_font_size(cx));
        // The window's title carries the front tab's badge; the Dock and the status bar count
        // every tab's threads that need the person.
        let project = self.project();
        let here = project.waiting();
        window.set_window_title(&if here > 0 {
            format!("{} ({here})", project.name())
        } else {
            project.name().to_string()
        });
        let waiting: usize = self.projects.iter().map(Project::waiting).sum();
        if self.dock_badge != Some(waiting) {
            self.dock_badge = Some(waiting);
            dock::set_badge(waiting);
        }
        let live = !matches!(self.connection, Connection::Replay);
        let colors = cx.theme().colors();
        let background = colors.background;
        let text = colors.text;
        let body = if live {
            self.render_blocks(window, cx)
        } else {
            // A replay is one conversation, alone.
            div()
                .flex_grow(1.)
                .min_h_0()
                .w_full()
                .p(px(blocks::BLOCK_GAP))
                .flex()
                .child(
                    div()
                        .size_full()
                        .rounded(px(blocks::BLOCK_RADIUS))
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.editor_background)
                        .overflow_hidden()
                        .child(self.agent.clone()),
                )
                .into_any_element()
        };
        let overview = live.then(|| self.render_overview(cx)).flatten();
        let tabs: Vec<ProjectTab> = self.projects.iter().map(Project::tab).collect();
        let compact = f32::from(window.viewport_size().width) < COMPACT_WINDOW;
        let before = if live {
            self.render_threads_button(cx)
        } else {
            div().into_any_element()
        };
        let after = if live {
            self.render_title_tools(compact, cx)
        } else {
            div().into_any_element()
        };
        let strip =
            tab_strip::strip(
                &tabs,
                self.active,
                cx,
                |click| Box::new(cx.listener(move |view, _, _, cx| view.click_tab(click, cx))),
                |to| {
                    Box::new(cx.listener(move |view, drag: &TabDrag, _, cx| {
                        view.move_tab(drag.from, to, cx)
                    }))
                },
                tab_strip::Ends {
                    before,
                    after,
                    compact,
                },
            )
            .id("tab-strip")
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, _| view.should_move = true),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, _, _| view.should_move = false),
            )
            .on_mouse_down_out(cx.listener(|view, _, _, _| view.should_move = false))
            .on_mouse_move(cx.listener(|view, _, window, _| {
                if std::mem::take(&mut view.should_move) {
                    window.start_window_move();
                }
            }))
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            });
        if std::mem::take(&mut self.refocus) {
            self.focus_input(window, cx);
        }
        let root = div()
            .track_focus(&self.root_focus)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .on_action(
                cx.listener(|view, _: &zed_actions::OpenSettings, _, cx| view.open_settings(cx)),
            )
            .on_action(cx.listener(|view, _: &StopRun, _, cx| view.stop(cx)))
            .on_action(cx.listener(Self::find_file))
            .on_action(cx.listener(Self::search_contents));
        let root = if live {
            self.block_actions(root, cx)
                .on_action(cx.listener(|view, _: &NewTab, _, cx| view.pick_project(cx)))
                .on_action(cx.listener(|view, _: &CloseTab, _, cx| view.close_tab(view.active, cx)))
                .on_action(cx.listener(|view, _: &NextTab, _, cx| {
                    let n = tab_strip::step(view.active, view.projects.len(), 1);
                    view.activate(n, cx)
                }))
                .on_action(cx.listener(|view, _: &PreviousTab, _, cx| {
                    let n = tab_strip::step(view.active, view.projects.len(), -1);
                    view.activate(n, cx)
                }))
                .on_action(cx.listener(|view, tab: &ActivateTab, _, cx| {
                    if let Some(n) = tab_strip::nth(tab.0, view.projects.len()) {
                        view.activate(n, cx)
                    }
                }))
                .on_action(cx.listener(|view, _: &FocusSearch, window, cx| {
                    // A hidden thread list comes back for the search.
                    if view.ws().find(Kind::Threads).is_none() {
                        view.open_kind(Kind::Threads, blocks::ToolTarget { against: None }, cx);
                    }
                    view.search
                        .update(cx, |search, cx| search.focus(window, cx))
                }))
        } else {
            root
        };
        root.font(
            theme_settings::ThemeSettings::get_global(cx)
                .ui_font
                .clone(),
        )
        .bg(background)
        .text_color(text)
        .child(strip)
        .child(body)
        .children(overview)
        .children(
            self.finder
                .as_ref()
                .map(|f| f.clone().into_any_element())
                .or(self.contents.as_ref().map(|c| c.clone().into_any_element()))
                .map(|finder| {
                    // Over everything, a little below the tabs, as Zed's file finder.
                    deferred(
                        div()
                            .absolute()
                            .top(px(64.))
                            .left_0()
                            .right_0()
                            .px_4()
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .occlude()
                                    .on_mouse_down_out(cx.listener(|view, _, window, cx| {
                                        view.close_finder(window, cx)
                                    }))
                                    .child(finder),
                            ),
                    )
                    .with_priority(2)
                }),
        )
    }
}
