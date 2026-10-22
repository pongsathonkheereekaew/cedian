//! The project chat (Projects): its row at the top of Threads (`⌘0`), and its block, the
//! conversation with the project's coordinator, the threads it opened as cards, and its own
//! composer. Its thread is the project's newest on cedia's `coordinator` profile; the first
//! message starts one.

use cedian_layout::Kind;
use cedian_wire::Request;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use super::{COMPOSER_WIDTH, RootView, blocks::ToolTarget};
use crate::{
    block_keys,
    project::CHAT_PROFILE,
    thread_list,
    tokens::{self, Tokens},
    type_scale::TypeScale as _,
};

/// The most cards of opened threads the chat block lists above its conversation.
const CARDS: usize = 4;

impl RootView {
    /// The row above the groups: lime dot when the chat waits on the person.
    pub(super) fn render_chat_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let tokens = Tokens::of(cx);
        let chat = self.project().chat_thread();
        let calls = chat.is_some_and(|t| t.needs_you);
        let opened = self.project().opened_by_chat().len();
        let shown = self.showing(Kind::Chat);
        div()
            .px_2()
            .pt_1()
            .child(
                div()
                    .id("chat-row")
                    .h(px(32.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .when(shown, |row| row.bg(colors.element_selected))
                    .hover(|row| row.bg(colors.element_hover))
                    .on_click(cx.listener(|view, _, _, cx| view.open_chat(cx)))
                    .child(
                        svg()
                            .path("icons/chat.svg")
                            .size(px(14.))
                            .text_color(colors.text_muted),
                    )
                    .child(
                        div()
                            .flex_grow(1.)
                            .min_w_0()
                            .truncate()
                            .text_ui()
                            .when(calls, |t| t.font_weight(gpui::FontWeight::SEMIBOLD))
                            .child("แชทโปรเจกต์"),
                    )
                    .when(opened > 0, |row| {
                        row.child(
                            div()
                                .flex_none()
                                .font(tokens::numeric_font())
                                .text_label()
                                .text_color(colors.text_placeholder)
                                .child(opened.to_string()),
                        )
                    })
                    .when(calls, |row| {
                        row.child(
                            div()
                                .flex_none()
                                .size(px(7.))
                                .rounded_full()
                                .bg(tokens.primary),
                        )
                    })
                    .when(!calls, |row| {
                        row.child(
                            div()
                                .flex_none()
                                .text_label()
                                .text_color(colors.text_placeholder)
                                .child(block_keys::shown(block_keys::CHAT)),
                        )
                    }),
            )
            .into_any_element()
    }

    /// The Project chat block.
    pub(super) fn render_chat(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let project = self.project();
        let opened = project.opened_by_chat();
        let empty = project.chat_thread().is_none() && !project.awaiting_chat;
        let mut cards = div().flex_none().flex().flex_col().gap_1().px_3().pt_2();
        for info in opened.iter().take(CARDS) {
            let id = info.thread.id.clone();
            cards = cards.child(
                div()
                    .id(SharedString::from(format!("chat-card-{id}")))
                    .h(px(34.))
                    .px_2p5()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(9.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.element_background)
                    .cursor_pointer()
                    .hover(|c| c.bg(colors.element_hover))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.select(id.clone(), cx);
                        view.ensure_agent(cx);
                    }))
                    .child(thread_list::dot(info.status, cx))
                    .child(
                        div()
                            .flex_grow(1.)
                            .min_w_0()
                            .truncate()
                            .text_ui()
                            .child(info.thread.title.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_label()
                            .text_color(colors.text_placeholder)
                            .child(thread_list::since(
                                &info.thread.updated_at,
                                chrono::Utc::now(),
                            )),
                    ),
            );
        }
        let more = opened.len().saturating_sub(CARDS);
        div()
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .when(!opened.is_empty(), |b| {
                b.child(cards.when(more > 0, |c| {
                    c.child(
                        div()
                            .px_1()
                            .text_meta()
                            .text_color(colors.text_placeholder)
                            .child(format!("และอีก {more} thread ใน Threads")),
                    )
                }))
            })
            .child(div().flex_grow(1.).min_h_0().map(|b| {
                if empty {
                    b.flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .px_6()
                        .text_color(colors.text_muted)
                        .child(div().text_ui().child("บอกสิ่งที่อยากทำในโปรเจกต์นี้"))
                        .child(
                            div()
                                .text_meta()
                                .text_color(colors.text_placeholder)
                                .child("แชทโปรเจกต์จะเปิด thread ให้แต่ละงาน"),
                        )
                } else {
                    b.child(self.chat.clone())
                }
            }))
            .child(
                div().w_full().flex_none().flex().justify_center().child(
                    div()
                        .w_full()
                        .min_w_0()
                        .max_w(px(COMPOSER_WIDTH))
                        .pb_2()
                        .child(self.chat_composer.clone()),
                ),
            )
            .into_any_element()
    }

    /// A message to the project chat: into its thread, or a new one on the coordinator
    /// profile, in the project itself (it opens worktrees for the threads it starts).
    pub(super) fn submit_chat(&mut self, text: String, cx: &mut Context<Self>) {
        if self.client.is_none() {
            self.error = Some("not connected to the daemon".into());
            cx.notify();
            return;
        }
        let thread = self
            .project()
            .chat_open
            .clone()
            .or_else(|| self.chat_thread_id());
        if thread.is_none() {
            if self.project().awaiting_chat {
                return;
            }
            self.project_mut().awaiting_chat = true;
        }
        let project = self.project();
        self.request(Request::Prompt {
            cwd: project.cwd.clone(),
            text,
            thread,
            worktree: false,
            profile: Some(CHAT_PROFILE.to_owned()),
            model: None,
            permission: None,
        });
        if !self.showing(Kind::Chat) {
            self.open_kind(Kind::Chat, ToolTarget { against: None }, cx);
        }
        cx.notify();
    }
}
