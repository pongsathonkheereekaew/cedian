//! The window: the project's threads, the open thread with its composer, and the editor and
//! terminal beside them. It keeps only what the daemon can send again.

use std::{path::PathBuf, sync::Arc};

use cedian_protocol::Line;
use cedian_wire::{Notice, Request, Status, ThreadInfo};
use futures::StreamExt as _;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _, px,
};
use theme::ActiveTheme as _;

use crate::{
    agent_pane::AgentPane,
    composer::{Composer, ComposerEvent},
    daemon_client::DaemonClient,
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
    /// Opens the most recent thread when the first list arrives.
    first_list: bool,
    error: Option<SharedString>,
    agent: Entity<AgentPane>,
    composer: Entity<Composer>,
    tools: Entity<SpikeView>,
}

impl RootView {
    pub fn replay(agent: Entity<AgentPane>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self::new(PathBuf::new(), agent, window, cx);
        view.connection = Connection::Replay;
        view
    }

    /// Connects to the daemon in the background and shows `cwd`'s threads.
    pub fn live(cwd: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let agent = cx.new(|_| AgentPane::new("new thread"));
        let view = Self::new(cwd, agent, window, cx);
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
        Self {
            cwd,
            client: None,
            connection: Connection::Connecting,
            threads: Vec::new(),
            current: None,
            status: None,
            awaiting_thread: false,
            first_list: true,
            error: None,
            agent,
            composer,
            tools,
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
            }
            Notice::Threads { cwd, threads } if cwd == self.cwd => {
                self.threads = threads
                    .into_iter()
                    .filter(|t| !t.thread.archived && t.thread.parent.is_none())
                    .collect();
                if std::mem::take(&mut self.first_list)
                    && self.current.is_none()
                    && !self.awaiting_thread
                    && let Some(latest) = self.threads.first()
                {
                    let id = latest.thread.id.clone();
                    self.select(id, cx);
                }
                self.retitle(cx);
            }
            Notice::Threads { .. } => {}
            Notice::Started { cwd, thread } if cwd == self.cwd && self.awaiting_thread => {
                self.awaiting_thread = false;
                self.current = Some(thread);
                self.status = Some(Status::Working);
                self.retitle(cx);
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
                }
            }
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
            None if self.awaiting_thread => "new thread · starting".to_owned(),
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
        self.status = None;
        self.agent
            .update(cx, |agent, cx| agent.reset(thread.clone(), cx));
        self.request(Request::Open {
            cwd: self.cwd.clone(),
            thread: thread.clone(),
        });
        self.current = Some(thread);
        self.retitle(cx);
    }

    fn new_thread(&mut self, cx: &mut Context<Self>) {
        if let Some(old) = self.current.take() {
            self.request(Request::Close { thread: old });
        }
        self.awaiting_thread = false;
        self.status = None;
        self.agent
            .update(cx, |agent, cx| agent.reset("new thread", cx));
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
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(connection),
            )
            .into_any_element()
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let live = !matches!(self.connection, Connection::Replay);
        let threads = live.then(|| self.render_threads(cx));
        let colors = cx.theme().colors();
        let status_colors = cx.theme().status();
        let failure = match &self.connection {
            Connection::Failed(error) => Some(error.clone()),
            _ => None,
        };
        let notice = failure.or_else(|| self.error.clone());
        let working = self.status == Some(Status::Working);
        div()
            .size_full()
            .flex()
            .bg(colors.background)
            .children(threads)
            .child(
                div()
                    .flex_grow(1.)
                    .h_full()
                    .flex()
                    .flex_col()
                    .border_r_1()
                    .border_color(colors.border)
                    .child(div().flex_grow(1.).min_h_0().child(self.agent.clone()))
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
                    .when(live, |column| column.child(self.composer.clone())),
            )
            .child(div().w(px(460.)).h_full().child(self.tools.clone()))
    }
}
