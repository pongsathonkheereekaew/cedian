//! Search at the top of the sidebar (Interface, Phase C: search moves to the top of the
//! thread list; `⌘K` focuses it). While something is typed, the sidebar lists the threads of
//! every tab whose title holds each word, the front tab's first, in place of its groups; Up and
//! Down move, Enter opens, Escape clears and goes back to the composer. It only finds: threads
//! are started by the project's coordinator, not from here.

use std::path::{Path, PathBuf};

use cedian_wire::{Status, ThreadInfo};
use editor::{Editor, EditorEvent};
use gpui::{
    AppContext as _, Context, Div, Entity, EventEmitter, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, actions, div, prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::type_scale::TypeScale as _;
use crate::{thread_list, tokens};

actions!(
    thread_search,
    [FocusSearch, SelectNext, SelectPrevious, Confirm, Dismiss]
);

/// Threads listed for what was typed.
const MATCHES: usize = 12;

#[cfg(target_os = "macos")]
const SEARCH_KEY: &str = "⌘K";
#[cfg(not(target_os = "macos"))]
const SEARCH_KEY: &str = "Ctrl+K";

pub enum SearchEvent {
    /// Open the thread, in the tab of the project that holds it.
    Open { cwd: PathBuf, thread: String },
    /// Escape left the search; focus goes back where the person types.
    Dismissed,
}

/// A thread as the search finds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub id: String,
    pub title: SharedString,
    pub status: Status,
    /// The project whose tab holds the thread.
    pub cwd: PathBuf,
    /// The project's name, for a thread of a tab behind the front one.
    pub elsewhere: Option<SharedString>,
}

impl Found {
    pub fn of(info: &ThreadInfo, cwd: &Path, elsewhere: Option<SharedString>) -> Self {
        Self {
            id: info.thread.id.clone(),
            title: info.thread.title.clone().into(),
            status: info.status,
            cwd: cwd.to_owned(),
            elsewhere,
        }
    }
}

pub struct ThreadSearch {
    input: Entity<Editor>,
    threads: Vec<Found>,
    selected: usize,
}

impl EventEmitter<SearchEvent> for ThreadSearch {}

impl ThreadSearch {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("ค้นหา thread", window, cx);
            editor
        });
        cx.subscribe(&input, |search, _, event, cx| {
            if matches!(event, EditorEvent::Edited { .. }) {
                search.selected = 0;
                cx.notify();
            }
        })
        .detach();
        Self {
            input,
            threads: Vec::new(),
            selected: 0,
        }
    }

    pub fn set(&mut self, threads: Vec<Found>, cx: &mut Context<Self>) {
        if threads != self.threads {
            self.threads = threads;
            cx.notify();
        }
    }

    /// Whether something is typed, so the sidebar shows matches instead of its groups.
    pub fn searching(&self, cx: &gpui::App) -> bool {
        !self.query(cx).trim().is_empty()
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.input.focus_handle(cx), cx);
    }

    fn query(&self, cx: &gpui::App) -> String {
        self.input.read(cx).text(cx)
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        let count = found(&self.threads, &self.query(cx)).len();
        if self.selected + 1 < count {
            self.selected += 1;
            cx.notify();
        }
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.selected = self.selected.saturating_sub(1);
        cx.notify();
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        let matches = found(&self.threads, &self.query(cx));
        if let Some(n) = matches.get(self.selected) {
            let thread = &self.threads[*n];
            let event = SearchEvent::Open {
                cwd: thread.cwd.clone(),
                thread: thread.id.clone(),
            };
            self.clear(window, cx);
            cx.emit(event);
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        self.clear(window, cx);
        cx.emit(SearchEvent::Dismissed);
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |editor, cx| editor.clear(window, cx));
        self.selected = 0;
        cx.notify();
    }

    fn results(&self, cx: &mut Context<Self>) -> Div {
        let colors = cx.theme().colors().clone();
        let matches = found(&self.threads, &self.query(cx));
        let mut list = div().flex().flex_col().gap_px().pt_1();
        if matches.is_empty() {
            return list.child(
                div()
                    .px_2()
                    .py_2()
                    .text_meta()
                    .text_color(colors.text_muted)
                    .child("ไม่พบ thread"),
            );
        }
        for (row, n) in matches.iter().enumerate() {
            let thread = &self.threads[*n];
            let (cwd, id) = (thread.cwd.clone(), thread.id.clone());
            list = list.child(
                div()
                    .id(("found", row))
                    .px_2()
                    .py(px(6.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .when(row == self.selected, |r| r.bg(colors.element_selected))
                    .hover(|r| r.bg(colors.element_hover))
                    .on_click(cx.listener(move |search, _, window, cx| {
                        search.clear(window, cx);
                        cx.emit(SearchEvent::Open {
                            cwd: cwd.clone(),
                            thread: id.clone(),
                        });
                    }))
                    .child(thread_list::dot(thread.status, cx))
                    .child(
                        div()
                            .flex_grow(1.)
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().text_ui().truncate().child(thread.title.clone()))
                            .child(div().text_meta().text_color(colors.text_muted).child(
                                match &thread.elsewhere {
                                    Some(project) => format!(
                                        "{} · {project}",
                                        thread_list::state_word(thread.status)
                                    ),
                                    None => thread_list::state_word(thread.status).to_owned(),
                                },
                            )),
                    ),
            );
        }
        list
    }
}

/// The threads whose title holds every word typed, ignoring case, in the order given.
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

impl Render for ThreadSearch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let focused = self.input.focus_handle(cx).is_focused(window);
        let searching = self.searching(cx);
        let results = searching.then(|| self.results(cx));
        div()
            .key_context("ThreadSearch")
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::dismiss))
            .flex()
            .flex_col()
            .px_2()
            .pt_2()
            .child(
                div()
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
                    .bg(tokens::Tokens::of(cx).field)
                    .child(
                        svg()
                            .path("icons/magnifying_glass.svg")
                            .size(px(14.))
                            .text_color(colors.text_muted),
                    )
                    .child(div().flex_grow(1.).min_w_0().child(self.input.clone()))
                    .child(
                        div()
                            .text_label()
                            .text_color(colors.text_muted)
                            .child(SEARCH_KEY),
                    ),
            )
            .children(results)
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
            cwd: PathBuf::from("/repo"),
            elsewhere: None,
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
