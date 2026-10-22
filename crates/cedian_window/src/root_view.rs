//! The window: the project tab and the toolbar, the project's threads, the open thread with
//! its composer, the tool panel and its rail, and the status bar. It keeps only what the daemon
//! can send again.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use cedian_protocol::Line;
use cedian_wire::{HunkState, Notice, Request, Status, ThreadInfo};
use editor::Editor;
use futures::StreamExt as _;
use gpui::{
    AnyElement, AppContext as _, Context, DragMoveEvent, Entity, EntityInputHandler as _,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, Window, WindowControlArea, WindowHandle,
    div, prelude::FluentBuilder as _, px,
};
use settings::Settings as _;
use theme::ActiveTheme as _;

use crate::{
    agent_pane::{AgentEvent, AgentPane, Header, StopRun},
    composer::{Composer, ComposerEvent, Queued},
    daemon_client::DaemonClient,
    desktop::Desktop,
    dock,
    layout::Layout,
    line_breaks,
    project::{self, Project},
    review_pane::{ReviewEvent, ReviewPane},
    settings_view::{SettingsEvent, SettingsView},
    settings_window::SettingsWindow,
    shell_pane::{AttachSelection, ShellEvent, ShellPane},
    status_bar::{self, StatusLine},
    tab_strip::{
        self, ActivateTab, CloseTab, NewTab, NextTab, PreviousTab, ProjectTab, TabClick, TabDrag,
    },
    thread_details::{self, Details},
    thread_list::{self, Group},
    tool_rail::{
        self, DividerDrag, HideTools, ToggleDetails, ToggleEditor, ToggleReview, ToggleTerminal,
        Tool,
    },
    toolbar::{FocusOmnibox, Found, Toolbar, ToolbarEvent},
    view_mode::ViewMode,
};

gpui::actions!(sidebar, [ToggleSidebar]);

/// The sidebar's width, open and folded to its rail of dots.
const SIDEBAR_WIDTH: f32 = 264.;
const SIDEBAR_FOLDED: f32 = 56.;

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
    editor: Entity<Editor>,
    toolbar: Entity<Toolbar>,
    review: Entity<ReviewPane>,
    /// The tools open in the panel, in rail order.
    tools: Vec<Tool>,
    /// Hunks of the open thread's review still waiting, for the rail's badge.
    waiting_hunks: usize,
    /// The review fills the main area in place of the agent pane.
    review_wide: bool,
    settings: Entity<SettingsView>,
    settings_window: Option<WindowHandle<SettingsWindow>>,
    /// What the next window opens with; saved shortly after it changes.
    layout: Layout,
    saving_layout: Option<Task<()>>,
    /// A press on the tab strip that moves the window once the mouse moves.
    should_move: bool,
    /// The count last put on the Dock icon.
    dock_badge: Option<usize>,
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
        view.tools = layout.tools.clone();
        view.layout = layout;
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
        let tabs = if tabs.is_empty() {
            vec![PathBuf::new()]
        } else {
            tabs
        };
        let active = active.min(tabs.len() - 1);
        let project = project::name_of(&tabs[active]);
        let toolbar = cx.new(|cx| Toolbar::new(project, window, cx));
        cx.subscribe_in(&toolbar, window, |view, _, event, window, cx| match event {
            ToolbarEvent::Open { cwd, thread } => {
                view.open_thread(cwd, thread.clone(), cx);
                view.composer
                    .update(cx, |composer, cx| composer.focus(window, cx));
            }
            ToolbarEvent::Start(task) => {
                view.new_thread(cx);
                view.submit(task.clone(), cx);
            }
            ToolbarEvent::NewThread => {
                view.new_thread(cx);
                view.composer
                    .update(cx, |composer, cx| composer.focus(window, cx));
            }
            ToolbarEvent::Dismissed => view
                .composer
                .update(cx, |composer, cx| composer.focus(window, cx)),
        })
        .detach();
        // Thread details reads the transcript.
        cx.observe(&agent, |view, _, cx| {
            if view.tools.contains(&Tool::Details) {
                cx.notify();
            }
        })
        .detach();
        let composer = cx.new(|cx| Composer::new(window, cx));
        cx.subscribe(&composer, |view, _, event, cx| match event {
            ComposerEvent::Submit(text) => view.submit(text.clone(), cx),
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
        cx.subscribe(&agent, |view, agent, event, cx| match event {
            AgentEvent::Stop => view.stop(cx),
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
            AgentEvent::Review => view.show_review(true, cx),
        })
        .detach();
        let settings = cx.new(|_| SettingsView::default());
        cx.subscribe(&settings, |view, _, event, _| match event {
            SettingsEvent::Set { key, value } => view.request(Request::SetDesktop {
                key: (*key).to_owned(),
                value: value.clone(),
            }),
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
            ReviewEvent::Widen(wide) => view.widen_review(*wide, cx),
            ReviewEvent::Rerun { command, path } => view.submit(
                format!("รัน check `{command}` ใหม่: {path} ถูกแก้หลังจาก check ครั้งล่าสุด"),
                cx,
            ),
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
            editor,
            toolbar,
            review,
            tools: Layout::default().tools,
            waiting_hunks: 0,
            review_wide: false,
            settings,
            settings_window: None,
            layout: Layout::default(),
            saving_layout: None,
            should_move: false,
            dock_badge: None,
        }
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.request(Request::Settings);
        self.settings_window =
            SettingsWindow::show(self.settings.clone(), self.settings_window.take(), cx);
    }

    fn request(&self, request: Request) {
        if let Some(client) = &self.client {
            client.request(&request);
        }
    }

    fn on_notice(&mut self, notice: Notice, cx: &mut Context<Self>) {
        match notice {
            Notice::Welcome { cedia, .. } => {
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
                }
                self.request(Request::Settings);
            }
            Notice::Settings { path, desktop } => {
                let settings = Desktop::from_value(&desktop);
                settings.apply(cx);
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
                project.set_threads(threads);
                if std::mem::take(&mut project.first_list)
                    && project.current.is_none()
                    && !project.awaiting_thread
                {
                    let saved = self.layout.open.get(&cwd.display().to_string());
                    let thread = project
                        .threads
                        .iter()
                        .find(|t| Some(&t.thread.id) == saved)
                        .or(project.threads.first())
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
            Notice::Line { .. } | Notice::Caught { .. } => {}
            Notice::Status { thread, status } => {
                for project in &mut self.projects {
                    if let Some(info) = project.threads.iter_mut().find(|t| t.thread.id == thread) {
                        info.status = status;
                    }
                    if project.current.as_ref() == Some(&thread) {
                        project.status = Some(status);
                    }
                }
                if self.project().current.as_ref() == Some(&thread) {
                    self.retitle(cx);
                    self.refresh_review();
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
        // The omnibox finds threads of every tab, this one's first.
        let found = std::iter::once(project)
            .chain(self.projects.iter().filter(|p| p.cwd != project.cwd))
            .flat_map(|p| {
                let elsewhere = (p.cwd != project.cwd).then(|| p.name());
                p.threads
                    .iter()
                    .map(move |t| Found::of(t, &p.cwd, elsewhere.clone()))
            })
            .collect();
        let dir = project.shell_dir();
        let name = project.name();
        let header = Header {
            title: title.into(),
            status: project.status,
            cwd: info.and_then(|t| t.cwd.clone()),
            rating: info.and_then(|t| t.thread.rating),
        };
        self.agent
            .update(cx, |agent, cx| agent.set_header(header, cx));
        let queue = info.map(queued).unwrap_or_default();
        self.toolbar
            .update(cx, |toolbar, cx| toolbar.set(name, found, branch, dir, cx));
        self.composer
            .update(cx, |composer, cx| composer.set_queue(queue, cx));
    }

    fn select(&mut self, thread: String, cx: &mut Context<Self>) {
        if self.project().current.as_ref() == Some(&thread) {
            return;
        }
        if let Some(old) = self.project_mut().current.take() {
            self.request(Request::Close { thread: old });
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
        self.active = n;
        // A notice was about the tab left behind.
        self.error = None;
        self.show_current(cx);
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
                self.projects.push(Project::new(dir));
                self.projects.len() - 1
            }
        };
        self.activate(n, cx);
        self.remember_tabs();
        self.save_layout(cx);
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
        if matches!(self.connection, Connection::Replay) {
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

    fn widen_review(&mut self, wide: bool, cx: &mut Context<Self>) {
        self.review_wide = wide;
        self.review.update(cx, |pane, cx| pane.set_wide(wide, cx));
        if !wide && !self.tools.contains(&Tool::Review) {
            tool_rail::toggle(&mut self.tools, Tool::Review);
            self.layout.tools = self.tools.clone();
            self.save_layout(cx);
        }
        self.refresh_review();
        cx.notify();
    }

    fn show_review(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.tools.contains(&Tool::Review) != on {
            self.toggle_tool(Tool::Review, cx);
        }
    }

    fn toggle_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        if tool == Tool::Review && self.review_wide {
            // The review is in the main area; its button brings it back to the panel.
            self.widen_review(false, cx);
            return;
        }
        tool_rail::toggle(&mut self.tools, tool);
        self.layout.tools = self.tools.clone();
        self.save_layout(cx);
        self.refresh_review();
        cx.notify();
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

    fn hide_tools(&mut self, cx: &mut Context<Self>) {
        self.tools.clear();
        self.layout.tools.clear();
        self.save_layout(cx);
        cx.notify();
    }

    /// The thread's question, with a button per option; any other answer goes in the composer.
    fn render_question(
        &self,
        question: cedian_index::ThreadQuestion,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
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
                    .text_size(px(13.))
                    .map(|button| {
                        if recommended {
                            button
                                .bg(colors.text_accent)
                                .border_color(colors.text_accent)
                                .text_color(colors.background)
                        } else {
                            button
                                .bg(colors.element_background)
                                .border_color(colors.border)
                                .text_color(colors.text)
                                .hover(|b| b.bg(colors.element_hover))
                        }
                    })
                    .on_click(cx.listener(move |view, _, _, cx| view.submit(answer.clone(), cx)))
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
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.text_accent)
                    .child("ถามคุณ"),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(24.))
                    .child(line_breaks::wrapped(&question.question)),
            )
            .child(options)
            .child(
                div()
                    .text_size(px(12.))
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

    /// The panel's open tools stacked in rail order, beside the rail.
    fn render_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let live = !matches!(self.connection, Connection::Replay);
        // A widened review sits in the main area, so the panel keeps the rest.
        let shown: Vec<Tool> = self
            .tools
            .iter()
            .copied()
            .filter(|t| !(*t == Tool::Review && (self.review_wide || !live)))
            .collect();
        let details = shown
            .contains(&Tool::Details)
            .then(|| Details::of(self.agent.read(cx).transcript()));
        let info = self.project().info();
        let mut panel = div()
            .w(px(tool_rail::PANEL_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(colors.border);
        for (n, tool) in shown.iter().enumerate() {
            let body: AnyElement = match tool {
                Tool::Editor => div()
                    .key_context("ToolEditor")
                    .on_action(cx.listener(Self::attach_editor_selection))
                    .size_full()
                    .p_2()
                    .child(self.editor.clone())
                    .into_any_element(),
                Tool::Terminal => match self.project().shells.get(&self.project().shell_dir()) {
                    Some(shell) => shell.clone().into_any_element(),
                    None => div().into_any_element(),
                },
                Tool::Review => self.review.clone().into_any_element(),
                Tool::Details => div()
                    .id("details")
                    .size_full()
                    .overflow_y_scroll()
                    .child(thread_details::render(
                        info,
                        details.as_ref().unwrap_or(&Details::default()),
                        cx,
                    ))
                    .into_any_element(),
            };
            let tool = *tool;
            if n > 0 {
                // A 5 px strip over the line between two tools drags their shares.
                panel = panel.child(
                    div()
                        .id(("divider", n))
                        .h(px(5.))
                        .flex_none()
                        .cursor_row_resize()
                        .border_t_1()
                        .border_color(colors.border)
                        .hover(|d| d.bg(colors.element_hover))
                        .on_drag(DividerDrag(n - 1), |drag, _, _, cx| cx.new(|_| *drag)),
                );
            }
            panel =
                panel.child(
                    div()
                        .flex_basis(px(0.))
                        .flex_grow(self.layout.tool_size(tool))
                        .min_h(px(60.))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
                            div()
                                .h(px(28.))
                                .flex_none()
                                .px_3()
                                .flex()
                                .items_center()
                                .bg(colors.panel_background)
                                .text_size(px(11.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(colors.text_muted)
                                .child(div().flex_grow(1.).child(tool.label()))
                                .child(
                                    div()
                                        .id(SharedString::from(format!("close-{}", tool.key())))
                                        .px_1()
                                        .cursor_pointer()
                                        .hover(|c| c.text_color(colors.text))
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.toggle_tool(tool, cx)
                                        }))
                                        .child("×"),
                                ),
                        )
                        .child(div().flex_1().min_h_0().child(body)),
                );
        }
        let dragged = shown.clone();
        let panel = panel.on_drag_move(cx.listener(
            move |view, event: &DragMoveEvent<DividerDrag>, _, cx| {
                let height = f32::from(event.bounds.size.height);
                if height > 0. {
                    let at = f32::from(event.event.position.y - event.bounds.top()) / height;
                    let divider = event.drag(cx).0;
                    view.resize_tools(&dragged, divider, at, cx);
                }
            },
        ));
        let rail = tool_rail::rail(&self.tools, self.waiting_hunks, cx, |tool| {
            Box::new(cx.listener(move |view, _, _, cx| view.toggle_tool(tool, cx)))
        });
        div()
            .flex_none()
            .h_full()
            .flex()
            .when(!shown.is_empty(), |row| row.child(panel))
            .child(rail)
            .into_any_element()
    }

    /// The sidebar folded (`⌘B`): each thread as its first letter over its state dot, in the
    /// same order, the one open marked and the ones that need you ringed.
    fn render_folded_threads(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let status_colors = cx.theme().status();
        let mut rail = div()
            .id("threads")
            .w(px(SIDEBAR_FOLDED))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .py_2()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .text_color(colors.text)
            .overflow_y_scroll();
        for group in Group::ALL {
            for info in self
                .project()
                .threads
                .iter()
                .filter(|t| Group::of(t) == group)
            {
                let id = info.thread.id.clone();
                let selected = self.project().current.as_ref() == Some(&id);
                let initial: String = info
                    .thread
                    .title
                    .chars()
                    .find(|c| c.is_alphanumeric())
                    .map_or_else(|| "·".to_owned(), |c| c.to_uppercase().collect());
                rail = rail.child(
                    div()
                        .id(SharedString::from(format!("folded-{id}")))
                        .relative()
                        .size(px(36.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(2.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .when(selected, |row| row.bg(colors.element_selected))
                        .hover(|row| row.bg(colors.element_hover))
                        .when(info.needs_you, |row| {
                            row.border_1()
                                .border_color(if info.status == Status::Failed {
                                    status_colors.error
                                } else {
                                    colors.text_accent
                                })
                        })
                        .on_click(cx.listener(move |view, _, _, cx| view.select(id.clone(), cx)))
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(initial),
                        )
                        .child(thread_list::dot(info.status, cx)),
                );
            }
        }
        rail.into_any_element()
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.layout.sidebar_folded = !self.layout.sidebar_folded;
        self.save_layout(cx);
        cx.notify();
    }

    /// A divider dragged to `at` (a fraction of the panel's height) between two of `shown`.
    fn resize_tools(&mut self, shown: &[Tool], divider: usize, at: f32, cx: &mut Context<Self>) {
        let mut weights: Vec<f32> = shown.iter().map(|t| self.layout.tool_size(*t)).collect();
        tool_rail::resize(&mut weights, divider, at);
        for (tool, weight) in shown.iter().zip(weights) {
            self.layout.tool_sizes.insert(tool.key().to_owned(), weight);
        }
        self.save_layout(cx);
        cx.notify();
    }

    fn submit(&mut self, text: String, cx: &mut Context<Self>) {
        if self.client.is_none() {
            self.error = Some("not connected to the daemon".into());
            cx.notify();
            return;
        }
        self.error = None;
        if self.project().current.is_none() {
            if self.project().awaiting_thread {
                self.error = Some("the new thread has not started yet".into());
                cx.notify();
                return;
            }
            self.project_mut().awaiting_thread = true;
        }
        let project = self.project();
        self.request(Request::Prompt {
            cwd: project.cwd.clone(),
            text,
            thread: project.current.clone(),
            worktree: project.current.is_none() && self.use_worktree,
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

    fn render_threads(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.layout.sidebar_folded {
            return self.render_folded_threads(cx);
        }
        let colors = cx.theme().colors();
        let status_colors = cx.theme().status();
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
                .filter(|t| Group::of(t) == group)
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
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.text_muted)
                    .child(group.label())
                    .when(counted > 0, |header| {
                        header.child(thread_list::badge(counted, failed, cx))
                    }),
            );
            for info in members {
                let id = info.thread.id.clone();
                let selected = self.project().current.as_ref() == Some(&id);
                let word = thread_list::state_word(info.status);
                let when = match info.status {
                    Status::Working => String::new(),
                    _ => thread_list::since(&info.thread.updated_at, now),
                };
                let meta = if when.is_empty() {
                    word.to_owned()
                } else {
                    format!("{word} · {when}")
                };
                list = list.child(
                    div()
                        .id(SharedString::from(id.clone()))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(6.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .when(selected, |row| row.bg(colors.element_selected))
                        .hover(|row| row.bg(colors.element_hover))
                        .on_click(cx.listener(move |view, _, _, cx| view.select(id.clone(), cx)))
                        .child(thread_list::dot(info.status, cx))
                        .child(
                            div()
                                .flex_grow(1.)
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .line_height(px(20.))
                                        .truncate()
                                        .when(info.needs_you, |title| {
                                            title.font_weight(gpui::FontWeight::SEMIBOLD)
                                        })
                                        .child(info.thread.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .line_height(px(18.))
                                        .text_color(if info.status == Status::Asking {
                                            colors.text_accent
                                        } else {
                                            colors.text_muted
                                        })
                                        .child(meta),
                                ),
                        )
                        .when(info.needs_you, |row| {
                            row.child(div().flex_none().size(px(6.)).rounded_full().bg(
                                if info.status == Status::Failed {
                                    status_colors.error
                                } else {
                                    colors.text_accent
                                },
                            ))
                        }),
                );
            }
        }
        div()
            .w(px(SIDEBAR_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .text_color(colors.text)
            .child(list)
            .into_any_element()
    }
}

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

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Text sizes are in rems, so the interface size setting scales all of it, as in Zed.
        window.set_rem_size(theme_settings::ThemeSettings::get_global(cx).ui_font_size(cx));
        if !matches!(self.connection, Connection::Replay) && self.tools.contains(&Tool::Terminal) {
            self.ensure_shell(window, cx);
        }
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
        let threads = live.then(|| self.render_threads(cx));
        let question = self
            .project()
            .info()
            .filter(|t| t.status == Status::Asking)
            .and_then(|t| t.thread.question.clone())
            .map(|question| self.render_question(question, cx));
        let colors = cx.theme().colors();
        let status_colors = cx.theme().status();
        let failure = match &self.connection {
            Connection::Failed(error) => Some(error.clone()),
            _ => None,
        };
        let notice = failure.or_else(|| self.error.clone());
        let background = colors.background;
        let text = colors.text;
        let wide = self.review_wide;
        let body = div()
            .flex_grow(1.)
            .min_h_0()
            .w_full()
            .flex()
            .children(threads)
            .child(
                div()
                    .flex_grow(1.)
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .border_r_1()
                    .border_color(colors.border)
                    .child(div().flex_grow(1.).min_h_0().map(|main| {
                        if wide {
                            main.child(self.review.clone())
                        } else {
                            main.child(self.agent.clone())
                        }
                    }))
                    .when_some(notice, |column, notice| {
                        column.child(
                            div()
                                .px_3()
                                .py_1()
                                .text_xs()
                                .text_color(status_colors.error)
                                .child(notice),
                        )
                    })
                    .when(!wide, |column| column.children(question))
                    .when(
                        !wide && live && self.project().current.is_none(),
                        |column| {
                            let word = if self.use_worktree {
                                "In a new worktree: on"
                            } else {
                                "In a new worktree: off"
                            };
                            column.child(
                                div()
                                    .id("worktree")
                                    .px_3()
                                    .py_1()
                                    .text_xs()
                                    .cursor_pointer()
                                    .text_color(colors.text_muted)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.use_worktree = !view.use_worktree;
                                        cx.notify();
                                    }))
                                    .child(word),
                            )
                        },
                    )
                    .when(!wide && live, |column| column.child(self.composer.clone())),
            )
            // A widened review leaves the panel the other tools, and the rail.
            .when(live, |body| body.child(self.render_tools(cx)));
        let (daemon, connected, cedia): (SharedString, bool, Option<SharedString>) =
            match &self.connection {
                Connection::Replay => ("replay".into(), false, None),
                Connection::Connecting => ("กำลังต่อ daemon".into(), false, None),
                Connection::Connected { cedia } => ("daemon".into(), true, Some(cedia.clone())),
                Connection::Failed(_) => ("ต่อ daemon ไม่ได้".into(), false, None),
            };
        let working = self.projects.iter().map(Project::working).sum();
        let status = status_bar::bar(
            StatusLine {
                daemon,
                connected,
                cedia,
                working,
                waiting,
            },
            div()
                .id("open-settings")
                .on_click(cx.listener(|view, _, _, cx| view.open_settings(cx)))
                .child("Settings"),
            cx,
        );
        let tabs: Vec<ProjectTab> = self.projects.iter().map(Project::tab).collect();
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
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_action(
                cx.listener(|view, _: &zed_actions::OpenSettings, _, cx| view.open_settings(cx)),
            )
            .on_action(cx.listener(|view, _: &StopRun, _, cx| view.stop(cx)))
            .on_action(
                cx.listener(|view, _: &ToggleEditor, _, cx| view.toggle_tool(Tool::Editor, cx)),
            )
            .on_action(
                cx.listener(|view, _: &ToggleTerminal, _, cx| view.toggle_tool(Tool::Terminal, cx)),
            )
            .on_action(
                cx.listener(|view, _: &ToggleReview, _, cx| view.toggle_tool(Tool::Review, cx)),
            )
            .on_action(
                cx.listener(|view, _: &ToggleDetails, _, cx| view.toggle_tool(Tool::Details, cx)),
            )
            .on_action(cx.listener(|view, _: &HideTools, _, cx| view.hide_tools(cx)))
            .on_action(cx.listener(|view, _: &ToggleSidebar, _, cx| view.toggle_sidebar(cx)))
            .when(live, |root| {
                root.on_action(cx.listener(|view, _: &NewTab, _, cx| view.pick_project(cx)))
                    .on_action(
                        cx.listener(|view, _: &CloseTab, _, cx| view.close_tab(view.active, cx)),
                    )
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
            })
            .on_action(cx.listener(|view, _: &FocusOmnibox, window, cx| {
                view.toolbar
                    .update(cx, |toolbar, cx| toolbar.focus(window, cx))
            }))
            .font(
                theme_settings::ThemeSettings::get_global(cx)
                    .ui_font
                    .clone(),
            )
            .bg(background)
            .text_color(text)
            .child(strip)
            .when(live, |root| root.child(self.toolbar.clone()))
            .child(body)
            .when(live, |root| root.child(status))
    }
}
