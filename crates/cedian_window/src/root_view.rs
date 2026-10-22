//! The window: the project's threads, the open thread with its composer, and the editor and
//! terminal beside them. It keeps only what the daemon can send again.

use std::{path::PathBuf, sync::Arc};

use cedian_protocol::Line;
use cedian_wire::{Notice, Request, Status, ThreadInfo};
use futures::StreamExt as _;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, Focusable as _, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Task, Window, div, prelude::FluentBuilder as _, px,
};
use settings::Settings as _;
use theme::ActiveTheme as _;

use crate::{
    agent_pane::AgentPane,
    composer::{Composer, ComposerEvent},
    daemon_client::DaemonClient,
    desktop::Desktop,
    layout::Layout,
    review_pane::{ReviewEvent, ReviewPane},
    settings_view::{SettingsEvent, SettingsView},
    spike_view::SpikeView,
};

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
    cwd: PathBuf,
    client: Option<Arc<DaemonClient>>,
    connection: Connection,
    threads: Vec<ThreadInfo>,
    /// The thread in the agent pane, if it has one yet.
    current: Option<String>,
    status: Option<Status>,
    /// A prompt went out with no thread, and the daemon has not named the thread yet.
    awaiting_thread: bool,
    /// A new thread runs in its own git worktree.
    use_worktree: bool,
    /// What the daemon is doing before the new thread's run starts.
    preparing: Option<String>,
    /// Opens the most recent thread when the first list arrives.
    first_list: bool,
    error: Option<SharedString>,
    agent: Entity<AgentPane>,
    composer: Entity<Composer>,
    tools: Entity<SpikeView>,
    review: Entity<ReviewPane>,
    /// The right column shows the review instead of the editor and terminal.
    showing_review: bool,
    settings: Entity<SettingsView>,
    /// The center column shows the settings instead of the thread.
    showing_settings: bool,
    /// What the next window opens with; saved shortly after it changes.
    layout: Layout,
    saving_layout: Option<Task<()>>,
}

impl RootView {
    pub fn replay(agent: Entity<AgentPane>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self::new(PathBuf::new(), agent, window, cx);
        view.connection = Connection::Replay;
        view
    }

    /// Connects to the daemon in the background and shows `cwd`'s threads.
    pub fn live(cwd: PathBuf, layout: Layout, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let agent = cx.new(|_| AgentPane::new("new thread"));
        let mut view = Self::new(cwd, agent, window, cx);
        view.showing_review = layout.showing_review;
        view.layout = layout;
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
        cwd: PathBuf,
        agent: Entity<AgentPane>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let shell_cwd = (!cwd.as_os_str().is_empty()).then(|| cwd.clone());
        let tools = cx.new(|cx| SpikeView::new(shell_cwd, window, cx));
        let composer = cx.new(|cx| Composer::new(window, cx));
        cx.subscribe(&composer, |view, _, event, cx| match event {
            ComposerEvent::Submit(text) => view.submit(text.clone(), cx),
        })
        .detach();
        let settings = cx.new(|_| SettingsView::default());
        cx.subscribe(&settings, |view, _, event, cx| match event {
            SettingsEvent::Set { key, value } => view.request(Request::SetDesktop {
                key: (*key).to_owned(),
                value: value.clone(),
            }),
            SettingsEvent::Close => {
                view.showing_settings = false;
                cx.notify();
            }
        })
        .detach();
        let review = cx.new(ReviewPane::new);
        cx.subscribe(&review, |view, _, event, _| match event {
            ReviewEvent::Decide { hunk, decision } => {
                if let Some(thread) = view.current.clone() {
                    view.request(Request::Decide {
                        cwd: view.cwd.clone(),
                        thread,
                        hunk: hunk.clone(),
                        decision: decision.clone(),
                    });
                }
            }
            ReviewEvent::Refresh => view.refresh_review(),
        })
        .detach();
        Self {
            cwd,
            client: None,
            connection: Connection::Connecting,
            threads: Vec::new(),
            current: None,
            status: None,
            awaiting_thread: false,
            use_worktree: true,
            preparing: None,
            first_list: true,
            error: None,
            agent,
            composer,
            tools,
            review,
            showing_review: false,
            settings,
            showing_settings: false,
            layout: Layout::default(),
            saving_layout: None,
        }
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
                    Ok(version) => format!("cedia {}", version.version),
                    Err(error) => {
                        self.error = Some(error.into());
                        "cedia not found".to_owned()
                    }
                };
                self.connection = Connection::Connected {
                    cedia: cedia.into(),
                };
                self.request(Request::Threads {
                    cwd: self.cwd.clone(),
                });
                self.request(Request::Settings);
            }
            Notice::Settings { path, desktop } => {
                Desktop::from_value(&desktop).apply(cx);
                self.settings
                    .update(cx, |view, cx| view.set(path, desktop, cx));
            }
            Notice::Threads { cwd, threads } if cwd == self.cwd => {
                self.threads = threads
                    .into_iter()
                    .filter(|t| !t.thread.archived && t.thread.parent.is_none())
                    .collect();
                if std::mem::take(&mut self.first_list)
                    && self.current.is_none()
                    && !self.awaiting_thread
                {
                    let saved = self.layout.open.get(&self.cwd.display().to_string());
                    let thread = self
                        .threads
                        .iter()
                        .find(|t| Some(&t.thread.id) == saved)
                        .or(self.threads.first());
                    if let Some(thread) = thread {
                        let id = thread.thread.id.clone();
                        self.select(id, cx);
                    }
                }
                self.retitle(cx);
            }
            Notice::Threads { .. } => {}
            Notice::Preparing { cwd, step } if cwd == self.cwd && self.awaiting_thread => {
                self.preparing = Some(step);
                self.retitle(cx);
            }
            Notice::Preparing { .. } => {}
            Notice::Started { cwd, thread } if cwd == self.cwd && self.awaiting_thread => {
                self.awaiting_thread = false;
                self.preparing = None;
                self.current = Some(thread);
                self.status = Some(Status::Working);
                self.retitle(cx);
                self.review_current(cx);
            }
            Notice::Started { .. } => {}
            Notice::Line { thread, line } if self.current.as_ref() == Some(&thread) => {
                match Line::parse(&line) {
                    Ok(line) => self.agent.update(cx, |agent, cx| agent.apply(&line, cx)),
                    Err(error) => log::warn!("{thread}: {error}"),
                }
            }
            Notice::Line { .. } | Notice::Caught { .. } => {}
            Notice::Status { thread, status } => {
                if let Some(info) = self.threads.iter_mut().find(|t| t.thread.id == thread) {
                    info.status = status;
                }
                if self.current.as_ref() == Some(&thread) {
                    self.status = Some(status);
                    self.retitle(cx);
                    self.refresh_review();
                }
            }
            Notice::Review { thread, review } if self.current.as_ref() == Some(&thread) => {
                self.review
                    .update(cx, |pane, cx| pane.set_review(review, cx));
            }
            Notice::Review { .. } => {}
            Notice::Error { message } => self.error = Some(message.into()),
        }
        cx.notify();
    }

    fn retitle(&mut self, cx: &mut Context<Self>) {
        let title = match &self.current {
            Some(id) => {
                let name = self
                    .threads
                    .iter()
                    .find(|t| &t.thread.id == id)
                    .map_or(id.as_str(), |t| t.thread.title.as_str());
                match self.status {
                    Some(status) => format!("{name} · {}", status_word(status)),
                    None => name.to_owned(),
                }
            }
            None if self.awaiting_thread => match &self.preparing {
                Some(step) => format!("new thread · {step}"),
                None => "new thread · starting".to_owned(),
            },
            None => "new thread".to_owned(),
        };
        self.agent
            .update(cx, |agent, cx| agent.set_title(title, cx));
    }

    fn select(&mut self, thread: String, cx: &mut Context<Self>) {
        if self.current.as_ref() == Some(&thread) {
            return;
        }
        if let Some(old) = self.current.take() {
            self.request(Request::Close { thread: old });
        }
        self.awaiting_thread = false;
        self.preparing = None;
        self.status = None;
        self.agent
            .update(cx, |agent, cx| agent.reset(thread.clone(), cx));
        self.request(Request::Open {
            cwd: self.cwd.clone(),
            thread: thread.clone(),
        });
        self.current = Some(thread);
        self.retitle(cx);
        self.review_current(cx);
    }

    fn new_thread(&mut self, cx: &mut Context<Self>) {
        if let Some(old) = self.current.take() {
            self.request(Request::Close { thread: old });
        }
        self.awaiting_thread = false;
        self.preparing = None;
        self.status = None;
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

    fn show_review(&mut self, on: bool, cx: &mut Context<Self>) {
        self.showing_review = on;
        self.layout.showing_review = on;
        self.save_layout(cx);
        self.refresh_review();
        cx.notify();
    }

    /// Points the review at the open thread.
    fn review_current(&mut self, cx: &mut Context<Self>) {
        let cwd = self.cwd.display().to_string();
        let changed = match &self.current {
            Some(thread) => self.layout.open.insert(cwd, thread.clone()).as_ref() != Some(thread),
            None => false,
        };
        if changed {
            self.save_layout(cx);
        }
        let thread = self.current.clone();
        self.review
            .update(cx, |pane, cx| pane.set_thread(thread, cx));
        self.refresh_review();
    }

    fn refresh_review(&self) {
        if let (true, Some(thread)) = (self.showing_review, &self.current) {
            self.request(Request::Review {
                cwd: self.cwd.clone(),
                thread: thread.clone(),
            });
        }
    }

    fn render_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let live = !matches!(self.connection, Connection::Replay);
        let tab = |id: &'static str, label: &'static str, on: bool| {
            div()
                .id(id)
                .px_2()
                .py_1()
                .text_xs()
                .cursor_pointer()
                .text_color(if on { colors.text } else { colors.text_muted })
                .when(on, |tab| tab.border_b_1().border_color(colors.text_accent))
                .child(label)
        };
        div()
            .w(px(460.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .when(live, |column| {
                column.child(
                    div()
                        .flex()
                        .flex_none()
                        .gap_1()
                        .px_1()
                        .border_b_1()
                        .border_color(colors.border)
                        .bg(colors.panel_background)
                        .child(
                            tab("tools-tab", "Editor and terminal", !self.showing_review).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.show_review(false, cx);
                                }),
                            ),
                        )
                        .child(tab("review-tab", "Review", self.showing_review).on_click(
                            cx.listener(|view, _, window, cx| {
                                view.show_review(true, cx);
                                window.focus(&view.review.focus_handle(cx), cx);
                            }),
                        )),
                )
            })
            .child(div().flex_grow(1.).min_h_0().map(|body| {
                if self.showing_review {
                    body.child(self.review.clone())
                } else {
                    body.child(self.tools.clone())
                }
            }))
            .into_any_element()
    }

    fn submit(&mut self, text: String, cx: &mut Context<Self>) {
        if self.client.is_none() {
            self.error = Some("not connected to the daemon".into());
            cx.notify();
            return;
        }
        self.error = None;
        if self.current.is_none() {
            if self.awaiting_thread {
                self.error = Some("the new thread has not started yet".into());
                cx.notify();
                return;
            }
            self.awaiting_thread = true;
        }
        self.request(Request::Prompt {
            cwd: self.cwd.clone(),
            text,
            thread: self.current.clone(),
            worktree: self.current.is_none() && self.use_worktree,
        });
        self.retitle(cx);
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(thread) = self.current.clone() {
            self.request(Request::Stop { thread });
        }
        cx.notify();
    }

    fn render_threads(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let status_colors = cx.theme().status();
        let connection: SharedString = match &self.connection {
            Connection::Replay => "replay".into(),
            Connection::Connecting => "connecting to the daemon".into(),
            Connection::Connected { cedia } => cedia.clone(),
            Connection::Failed(_) => "daemon unreachable".into(),
        };
        let mut list = div()
            .id("threads")
            .flex()
            .flex_col()
            .flex_grow(1.)
            .overflow_y_scroll();
        for info in &self.threads {
            let id = info.thread.id.clone();
            let selected = self.current.as_ref() == Some(&id);
            let color = status_color(info.status, status_colors, colors.text_muted);
            list = list.child(
                div()
                    .id(SharedString::from(id.clone()))
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .when(selected, |row| row.bg(colors.element_selected))
                    .hover(|row| row.bg(colors.element_hover))
                    .on_click(cx.listener(move |view, _, _, cx| view.select(id.clone(), cx)))
                    .child(div().text_sm().child(info.thread.title.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(color)
                            .child(status_word(info.status)),
                    ),
            );
        }
        div()
            .w(px(220.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .text_color(colors.text)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_2()
                    .py_2()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(div().text_sm().child("Threads"))
                    .child(
                        div()
                            .id("new-thread")
                            .text_sm()
                            .cursor_pointer()
                            .text_color(colors.text_accent)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.new_thread(cx);
                                view.composer
                                    .update(cx, |composer, cx| composer.focus(window, cx));
                            }))
                            .child("New"),
                    ),
            )
            .child(list)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(connection)
                    .child(
                        div()
                            .id("open-settings")
                            .cursor_pointer()
                            .text_color(colors.text_accent)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.showing_settings = !view.showing_settings;
                                view.request(Request::Settings);
                                cx.notify();
                            }))
                            .child("Settings"),
                    ),
            )
            .into_any_element()
    }
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

fn status_word(status: Status) -> &'static str {
    match status {
        Status::Working => "working",
        Status::Done => "done",
        Status::Failed => "failed",
        Status::Stopped => "stopped",
        Status::Interrupted => "interrupted",
    }
}

fn status_color(status: Status, colors: &theme::StatusColors, idle: Hsla) -> Hsla {
    match status {
        Status::Working => colors.info,
        Status::Done => colors.success,
        Status::Failed => colors.error,
        Status::Interrupted => colors.warning,
        Status::Stopped => idle,
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Text sizes are in rems, so the interface size setting scales all of it, as in Zed.
        window.set_rem_size(theme_settings::ThemeSettings::get_global(cx).ui_font_size(cx));
        let live = !matches!(self.connection, Connection::Replay);
        let threads = live.then(|| self.render_threads(cx));
        let colors = cx.theme().colors();
        let status_colors = cx.theme().status();
        let failure = match &self.connection {
            Connection::Failed(error) => Some(error.clone()),
            _ => None,
        };
        let notice = failure.or_else(|| self.error.clone());
        let working = self.status == Some(Status::Working) && !self.showing_settings;
        let live = live && !self.showing_settings;
        div()
            .size_full()
            .flex()
            .bg(colors.background)
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
                    .child(div().flex_grow(1.).min_h_0().map(|body| {
                        if self.showing_settings {
                            body.child(self.settings.clone())
                        } else {
                            body.child(self.agent.clone())
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
                    .when(working, |column| {
                        column.child(
                            div()
                                .id("stop")
                                .px_3()
                                .py_1()
                                .text_xs()
                                .cursor_pointer()
                                .text_color(colors.text_accent)
                                .on_click(cx.listener(|view, _, _, cx| view.stop(cx)))
                                .child("Stop the run"),
                        )
                    })
                    .when(live && self.current.is_none(), |column| {
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
                    })
                    .when(live, |column| column.child(self.composer.clone())),
            )
            .child(self.render_tools(cx))
    }
}
