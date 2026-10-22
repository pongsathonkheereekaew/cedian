//! Find a file by name (`⌘P`): a field over the window that matches what is typed against
//! every file of the open thread's directory, its worktree when it has one, the list the Files
//! block reads (git's files in a repository). Letters match in order, not side by side, so
//! `rvblk` finds `root_view/blocks.rs`; the best matches come first. Up and Down move, Enter
//! shows the file in the Editor, Escape closes.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use editor::{Editor, EditorEvent};
use fuzzy::{StringMatch, StringMatchCandidate};
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, HighlightStyle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, StyledText, Task, Window, actions, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::{tokens, type_scale::TypeScale as _};

actions!(file_finder, [FindFile, Next, Previous, Pick, Close]);

/// Rows shown at once.
const SHOWN: usize = 12;
const WIDTH: f32 = 600.;

pub enum FinderEvent {
    /// Show this file (an absolute path) in the Editor.
    Open(PathBuf),
    Closed,
}

pub struct FileFinder {
    input: Entity<Editor>,
    root: PathBuf,
    candidates: Arc<Vec<StringMatchCandidate>>,
    matches: Vec<StringMatch>,
    selected: usize,
    cancel: Arc<AtomicBool>,
    search: Option<Task<()>>,
}

impl EventEmitter<FinderEvent> for FileFinder {}

impl Focusable for FileFinder {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl FileFinder {
    /// `files` are relative to `root`.
    pub fn new(
        root: PathBuf,
        files: &[PathBuf],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("ค้นหาไฟล์ตามชื่อ", window, cx);
            editor
        });
        cx.subscribe(&input, |finder, _, event, cx| {
            if matches!(event, EditorEvent::Edited { .. }) {
                finder.find(cx);
            }
        })
        .detach();
        let candidates = files
            .iter()
            .enumerate()
            .map(|(n, path)| StringMatchCandidate::new(n, &path.to_string_lossy()))
            .collect();
        let mut finder = Self {
            input,
            root,
            candidates: Arc::new(candidates),
            matches: Vec::new(),
            selected: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            search: None,
        };
        finder.find(cx);
        finder
    }

    fn query(&self, cx: &gpui::App) -> String {
        self.input.read(cx).text(cx)
    }

    /// Matches the query in the background; a newer query cancels the one before.
    fn find(&mut self, cx: &mut Context<Self>) {
        self.cancel.store(true, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        let query = self.query(cx);
        let candidates = self.candidates.clone();
        if query.trim().is_empty() {
            // Nothing typed: the first files, in the tree's order.
            self.matches = candidates
                .iter()
                .take(SHOWN)
                .map(|c| StringMatch {
                    candidate_id: c.id,
                    score: 0.,
                    positions: Vec::new(),
                    string: c.string.clone(),
                })
                .collect();
            self.selected = 0;
            self.search = None;
            cx.notify();
            return;
        }
        let executor = cx.background_executor().clone();
        self.search = Some(cx.spawn(async move |this, cx| {
            let query = query.replace(' ', "");
            let matches = fuzzy::match_strings(
                candidates.as_slice(),
                &query,
                false,
                true,
                SHOWN,
                &cancel,
                executor,
            )
            .await;
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            this.update(cx, |finder, cx| {
                finder.matches = matches;
                finder.selected = 0;
                cx.notify();
            })
            .ok();
        }));
    }

    fn next(&mut self, _: &Next, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected + 1 < self.matches.len() {
            self.selected += 1;
            cx.notify();
        }
    }

    fn previous(&mut self, _: &Previous, _: &mut Window, cx: &mut Context<Self>) {
        self.selected = self.selected.saturating_sub(1);
        cx.notify();
    }

    fn pick(&mut self, _: &Pick, _: &mut Window, cx: &mut Context<Self>) {
        self.open(self.selected, cx);
    }

    fn open(&mut self, row: usize, cx: &mut Context<Self>) {
        if let Some(found) = self.matches.get(row) {
            cx.emit(FinderEvent::Open(self.root.join(&found.string)));
        }
    }

    fn close(&mut self, _: &Close, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(FinderEvent::Closed);
    }
}

/// The path with the letters that matched in the accent's text colour and weight.
fn highlighted(found: &StringMatch, color: gpui::Hsla) -> StyledText {
    let style = HighlightStyle {
        color: Some(color),
        font_weight: Some(gpui::FontWeight::SEMIBOLD),
        ..Default::default()
    };
    StyledText::new(found.string.clone())
        .with_highlights(found.ranges().map(|range| (range, style)))
}

impl Render for FileFinder {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let mut list = div().flex().flex_col().gap_px().p_1();
        if self.matches.is_empty() {
            let note = if self.candidates.is_empty() {
                "ยังไม่มีรายการไฟล์ของโฟลเดอร์นี้"
            } else {
                "ไม่พบไฟล์"
            };
            list = list.child(
                div()
                    .px_2()
                    .py_2()
                    .text_meta()
                    .text_color(colors.text_muted)
                    .child(note),
            );
        }
        for (row, found) in self.matches.iter().enumerate() {
            list = list.child(
                div()
                    .id(("file", row))
                    .px_2()
                    .py(px(5.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(row == self.selected, |r| r.bg(colors.element_selected))
                    .hover(|r| r.bg(colors.element_hover))
                    .on_click(cx.listener(move |finder, _, _, cx| finder.open(row, cx)))
                    .child(
                        svg()
                            .path("icons/file.svg")
                            .size(px(14.))
                            .flex_none()
                            .text_color(colors.text_muted),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_ui()
                            .font(tokens::code_font())
                            .child(highlighted(found, colors.text_accent)),
                    ),
            );
        }
        div()
            .key_context("FileFinder")
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::pick))
            .on_action(cx.listener(Self::close))
            .debug_selector(|| "file-finder".into())
            .w(px(WIDTH))
            .max_w_full()
            .flex()
            .flex_col()
            .rounded(px(12.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_lg()
            .overflow_hidden()
            .child(
                div()
                    .h(px(40.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(colors.border_variant)
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
                            .child(format!("{} ไฟล์", self.candidates.len())),
                    ),
            )
            .child(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_follow_the_matched_letters() {
        let found = StringMatch {
            candidate_id: 0,
            score: 1.,
            positions: vec![0, 5],
            string: "root_view.rs".into(),
        };
        assert_eq!(found.ranges().collect::<Vec<_>>(), [0..1, 5..6]);
    }
}
