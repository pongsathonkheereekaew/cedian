//! The toolbar under the project tabs (Interface, Layout: toolbar, 44 px): the project folder
//! and the open thread's worktree, the omnibox (`⌘K`) that finds a thread or starts one with
//! the task typed, and Thread ใหม่, the one primary button.

use cedian_wire::{Status, ThreadInfo};
use editor::{Editor, EditorEvent};
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, actions, deferred, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::{thread_list, tokens};

actions!(
    omnibox,
    [FocusOmnibox, SelectNext, SelectPrevious, Confirm, Dismiss]
);

pub const TOOLBAR_HEIGHT: f32 = 44.;

/// Threads the omnibox lists under what was typed.
const MATCHES: usize = 6;

pub enum ToolbarEvent {
    Open(String),
    /// Start a thread with this task.
    Start(String),
    NewThread,
    /// Escape left the omnibox; focus goes back where the person types.
    Dismissed,
}

/// A thread as the omnibox finds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub id: String,
    pub title: SharedString,
    pub status: Status,
}

impl Found {
    pub fn of(info: &ThreadInfo) -> Self {
        Self {
            id: info.thread.id.clone(),
            title: info.thread.title.clone().into(),
            status: info.status,
        }
    }
}

pub struct Toolbar {
    project: SharedString,
    /// The open thread's worktree branch, when it has one.
    branch: Option<String>,
    threads: Vec<Found>,
    omnibox: Entity<Editor>,
    selected: usize,
}

impl EventEmitter<ToolbarEvent> for Toolbar {}

impl Toolbar {
    pub fn new(project: SharedString, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let omnibox = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("ค้นหา thread หรือพิมพ์งานใหม่", window, cx);
            editor
        });
        cx.subscribe(&omnibox, |toolbar, _, event, cx| {
            if matches!(event, EditorEvent::Edited { .. }) {
                toolbar.selected = 0;
                cx.notify();
            }
        })
        .detach();
        Self {
            project,
            branch: None,
            threads: Vec::new(),
            omnibox,
            selected: 0,
        }
    }

    pub fn set(&mut self, threads: Vec<Found>, branch: Option<String>, cx: &mut Context<Self>) {
        if threads != self.threads || branch != self.branch {
            self.threads = threads;
            self.branch = branch;
            cx.notify();
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.omnibox.focus_handle(cx), cx);
    }

    fn query(&self, cx: &Context<Self>) -> String {
        self.omnibox.read(cx).text(cx)
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        // The row after the matches starts a thread with the text.
        let count = found(&self.threads, &self.query(cx)).len();
        if self.selected < count {
            self.selected += 1;
            cx.notify();
        }
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.selected = self.selected.saturating_sub(1);
        cx.notify();
    }

    /// Enter opens the selected thread, or starts one with the text when nothing matches.
    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.query(cx);
        if query.trim().is_empty() {
            return;
        }
        let matches = found(&self.threads, &query);
        let event = match matches.get(self.selected) {
            Some(n) => ToolbarEvent::Open(self.threads[*n].id.clone()),
            None => ToolbarEvent::Start(query.trim().to_owned()),
        };
        self.clear(window, cx);
        cx.emit(event);
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        self.clear(window, cx);
        cx.emit(ToolbarEvent::Dismissed);
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.omnibox
            .update(cx, |editor, cx| editor.clear(window, cx));
        self.selected = 0;
        cx.notify();
    }
}

/// The threads whose title holds every word typed, ignoring case, best first by order.
pub fn found(threads: &[Found], query: &str) -> Vec<usize> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Vec::new();
    }
    threads
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            let title = t.title.to_lowercase();
            words.iter().all(|w| title.contains(w.as_str()))
        })
        .map(|(n, _)| n)
        .take(MATCHES)
        .collect()
}

#[cfg(target_os = "macos")]
const OMNIBOX_KEY: &str = "⌘K";
#[cfg(not(target_os = "macos"))]
const OMNIBOX_KEY: &str = "Ctrl+K";

impl Render for Toolbar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let query = self.query(cx);
        let focused = self.omnibox.focus_handle(cx).is_focused(window);
        let matches = found(&self.threads, &query);
        let typed = !query.trim().is_empty();
        let mut list = div()
            .absolute()
            .top(px(34.))
            .left_0()
            .right_0()
            .flex()
            .flex_col()
            .p_1()
            .rounded(px(8.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .text_size(px(13.));
        for (row, n) in matches.iter().enumerate() {
            let thread = &self.threads[*n];
            let id = thread.id.clone();
            list = list.child(
                div()
                    .id(("found", row))
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(row == self.selected, |r| r.bg(colors.element_selected))
                    .hover(|r| r.bg(colors.element_hover))
                    .on_click(cx.listener(move |toolbar, _, window, cx| {
                        toolbar.clear(window, cx);
                        cx.emit(ToolbarEvent::Open(id.clone()));
                    }))
                    .child(thread_list::dot(thread.status, cx))
                    .child(
                        div()
                            .flex_grow(1.)
                            .min_w_0()
                            .truncate()
                            .child(thread.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(colors.text_muted)
                            .child(thread_list::state_word(thread.status)),
                    ),
            );
        }
        let start_row = matches.len();
        list = list.child(
            div()
                .h(px(30.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .rounded(px(6.))
                .when(self.selected >= start_row, |r| {
                    r.bg(colors.element_selected)
                })
                .text_color(colors.text_muted)
                .child(
                    svg()
                        .path("icons/plus.svg")
                        .size(px(12.))
                        .text_color(colors.text_muted),
                )
                .child(format!("เริ่ม thread ใหม่: {}", query.trim())),
        );
        let omnibox = div()
            .key_context("Omnibox")
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::dismiss))
            .relative()
            .flex_grow(1.)
            .max_w(px(560.))
            .min_w(px(200.))
            .h(px(30.))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(8.))
            .border_1()
            .border_color(if focused {
                colors.text_accent
            } else {
                colors.border
            })
            .bg(colors.editor_background)
            .child(
                svg()
                    .path("icons/magnifying_glass.svg")
                    .size(px(14.))
                    .text_color(colors.text_muted),
            )
            .child(div().flex_grow(1.).min_w_0().child(self.omnibox.clone()))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(colors.text_muted)
                    .child(OMNIBOX_KEY),
            )
            .when(focused && typed, |o| {
                o.child(deferred(list).with_priority(1))
            });
        div()
            .h(px(TOOLBAR_HEIGHT))
            .flex_none()
            .px_3()
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        svg()
                            .path("icons/folder.svg")
                            .size(px(14.))
                            .text_color(colors.text_muted),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.project.clone()),
                    )
                    .child(
                        div()
                            .h(px(22.))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_1()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(colors.border)
                            .text_size(px(12.))
                            .text_color(colors.text_muted)
                            .child(
                                svg()
                                    .path("icons/git_branch.svg")
                                    .size(px(12.))
                                    .text_color(colors.text_muted),
                            )
                            .child(
                                div().font(tokens::code_font()).child(
                                    self.branch
                                        .clone()
                                        .unwrap_or_else(|| "main checkout".into()),
                                ),
                            ),
                    ),
            )
            .child(div().flex_grow(1.).flex().justify_center().child(omnibox))
            .child(
                div()
                    .id("new-thread")
                    .flex_none()
                    .h(px(30.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .rounded(px(7.))
                    .bg(colors.text_accent)
                    .text_color(colors.background)
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ToolbarEvent::NewThread)))
                    .child(
                        svg()
                            .path("icons/plus.svg")
                            .size(px(12.))
                            .text_color(colors.background),
                    )
                    .child("Thread ใหม่"),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(id: &str, title: &str) -> Found {
        Found {
            id: id.to_owned(),
            title: title.to_owned().into(),
            status: Status::Done,
        }
    }

    #[test]
    fn every_word_must_be_in_the_title() {
        let threads = [
            thread("a", "Make f.txt say good"),
            thread("b", "แก้ปุ่มหยุด"),
            thread("c", "say hello"),
        ];
        assert_eq!(found(&threads, "SAY"), [0, 2]);
        assert_eq!(found(&threads, "say good"), [0]);
        assert_eq!(found(&threads, "ปุ่ม"), [1]);
        assert!(found(&threads, "  ").is_empty());
    }
}
