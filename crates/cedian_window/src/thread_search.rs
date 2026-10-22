//! Search at the top of the sidebar (Interface, Phase C: search moves to the top of the
//! thread list; `⌘K` focuses it). While something is typed, the sidebar lists the threads of
//! every tab whose title holds each word, the front tab's first, in place of its groups, then
//! the layouts by name and the blocks to add (ใหม่…, so typing `ใหม่` lists them all); Up
//! and Down move, Enter opens, Escape clears and goes back to the composer. It only finds:
//! threads are started by the project's coordinator, not from here.

use std::path::{Path, PathBuf};

use cedian_layout::Kind;
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
    /// Switch to a layout or add a block.
    Act(Act),
    /// Escape left the search; focus goes back where the person types.
    Dismissed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    /// The layout at this place in the project's list.
    Layout(usize),
    /// Open a block of this kind, or focus the one a layout holds once.
    Add(Kind),
}

/// A layout or a block to add, as the search finds it by name.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub act: Act,
    pub label: SharedString,
    /// Its key, or เปิดอยู่ for a block the layout already shows.
    pub note: Option<SharedString>,
}

impl Action {
    /// The word a row is grouped under, which the search matches too.
    fn group(&self) -> &'static str {
        match self.act {
            Act::Layout(_) => "layout",
            Act::Add(_) => "ใหม่…",
        }
    }

    fn icon(&self) -> &'static str {
        match self.act {
            Act::Layout(_) => "icons/blocks.svg",
            Act::Add(kind) => crate::block_header::icon(kind),
        }
    }
}

/// A row of the results.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    Thread(usize),
    Action(usize),
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
    actions: Vec<Action>,
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
            actions: Vec::new(),
            selected: 0,
        }
    }

    pub fn set(&mut self, threads: Vec<Found>, cx: &mut Context<Self>) {
        if threads != self.threads {
            self.threads = threads;
            cx.notify();
        }
    }

    pub fn set_actions(&mut self, actions: Vec<Action>, cx: &mut Context<Self>) {
        if actions != self.actions {
            self.actions = actions;
            cx.notify();
        }
    }

    fn hits(&self, cx: &gpui::App) -> Vec<Hit> {
        let query = self.query(cx);
        found(&self.threads, &query)
            .into_iter()
            .map(Hit::Thread)
            .chain(acted(&self.actions, &query).into_iter().map(Hit::Action))
            .collect()
    }

    fn event(&self, hit: Hit) -> SearchEvent {
        match hit {
            Hit::Thread(n) => SearchEvent::Open {
                cwd: self.threads[n].cwd.clone(),
                thread: self.threads[n].id.clone(),
            },
            Hit::Action(n) => SearchEvent::Act(self.actions[n].act),
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
        let count = self.hits(cx).len();
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
        if let Some(hit) = self.hits(cx).get(self.selected) {
            let event = self.event(*hit);
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
        let hits = self.hits(cx);
        let mut list = div().flex().flex_col().gap_px().pt_1();
        if hits.is_empty() {
            return list.child(
                div()
                    .px_2()
                    .py_2()
                    .text_meta()
                    .text_color(colors.text_muted)
                    .child("ไม่พบ thread"),
            );
        }
        for (row, hit) in hits.into_iter().enumerate() {
            let (lead, title, meta, note) = match hit {
                Hit::Thread(n) => {
                    let thread = &self.threads[n];
                    let meta = match &thread.elsewhere {
                        Some(project) => {
                            format!("{} · {project}", thread_list::state_word(thread.status))
                        }
                        None => thread_list::state_word(thread.status).to_owned(),
                    };
                    (
                        thread_list::dot(thread.status, cx).into_any_element(),
                        thread.title.clone(),
                        meta,
                        None,
                    )
                }
                Hit::Action(n) => {
                    let action = &self.actions[n];
                    (
                        svg()
                            .path(action.icon())
                            .size(px(14.))
                            .flex_none()
                            .text_color(colors.text_muted)
                            .into_any_element(),
                        action.label.clone(),
                        action.group().to_owned(),
                        action.note.clone(),
                    )
                }
            };
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
                        let event = search.event(hit);
                        search.clear(window, cx);
                        cx.emit(event);
                    }))
                    .child(lead)
                    .child(
                        div()
                            .flex_grow(1.)
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(crate::tooltip::cut("title", title).text_ui())
                            .child(div().text_meta().text_color(colors.text_muted).child(meta)),
                    )
                    .children(note.map(|note| {
                        div()
                            .flex_none()
                            .text_label()
                            .text_color(colors.text_muted)
                            .child(note)
                    })),
            );
        }
        list
    }
}

/// The layouts and blocks whose group and name hold every word typed, ignoring case.
fn acted(actions: &[Action], query: &str) -> Vec<usize> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Vec::new();
    }
    actions
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            let text = format!("{} {}", a.group(), a.label).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .map(|(n, _)| n)
        .collect()
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
            // The body's gap above the sheets is above the search too.
            .px_2()
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
                        colors.border_selected
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

    #[test]
    fn layouts_and_blocks_are_found_by_name_or_group() {
        let action = |act, label: &str| Action {
            act,
            label: label.to_owned().into(),
            note: None,
        };
        let actions = [
            action(Act::Layout(0), "โฟกัส"),
            action(Act::Layout(1), "เขียนโค้ด"),
            action(Act::Add(Kind::Terminal), "Terminal"),
            action(Act::Add(Kind::Review), "Review"),
        ];
        assert_eq!(acted(&actions, "ใหม่"), [2, 3]);
        assert_eq!(acted(&actions, "term"), [2]);
        assert_eq!(acted(&actions, "layout โค้ด"), [1]);
        assert!(acted(&actions, "").is_empty());
    }
}
