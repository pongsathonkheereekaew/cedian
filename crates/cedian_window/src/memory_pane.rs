//! The Memory block (Interface, Blocks): the project's memory, the short facts every new
//! session of the project gets in its first turn, the newest first, each with who kept it and
//! when. Read only until cedia offers a write: the person keeps and forgets notes with `cedia
//! memory add|forget`, a writer session with its `remember` tool.

use cedian_index::MemoryNote;
use gpui::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use theme::ActiveTheme as _;

use crate::{line_breaks::wrapped, thread_list::since, type_scale::TypeScale as _};

#[derive(Default)]
pub struct MemoryPane {
    /// Oldest first, as cedia keeps them; `None` until the daemon has answered.
    notes: Option<Vec<MemoryNote>>,
}

impl MemoryPane {
    pub fn set(&mut self, notes: Vec<MemoryNote>, cx: &mut Context<Self>) {
        self.notes = Some(notes);
        cx.notify();
    }

    /// Another project: what was listed was the last one's.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.notes = None;
        cx.notify();
    }

    pub fn count(&self) -> usize {
        self.notes.as_ref().map_or(0, Vec::len)
    }
}

/// Who kept a note: the person, or a session of a thread.
pub fn kept_by(from: &str) -> &'static str {
    if from == "user" {
        "คุณ"
    } else {
        "session"
    }
}

impl Render for MemoryPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let mut pane = div()
            .id("memory")
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .text_ui()
            .text_color(colors.text);
        let hint = div()
            .px_3()
            .pt_2()
            .pb_1()
            .text_meta()
            .text_color(colors.text_muted)
            .child(wrapped(
                "อ่านอย่างเดียว: เพิ่มหรือลบโน้ตด้วย cedia memory add | forget หรือให้ thread จำด้วย remember",
            ));
        let notes = match &self.notes {
            Some(notes) if !notes.is_empty() => notes,
            shown => {
                let word = if shown.is_none() {
                    "กำลังอ่าน"
                } else {
                    "โปรเจกต์นี้ยังไม่มีโน้ต"
                };
                return pane
                    .child(
                        div()
                            .px_3()
                            .pt_2()
                            .text_color(colors.text_muted)
                            .child(word),
                    )
                    .child(hint);
            }
        };
        let now = chrono::Utc::now();
        pane = pane.child(hint);
        for (n, note) in notes.iter().rev().enumerate() {
            pane = pane.child(
                div()
                    .debug_selector(move || format!("note-{n}"))
                    .flex_none()
                    .mx_3()
                    .my_1()
                    .px_2p5()
                    .py_1p5()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .min_w_0()
                    .overflow_hidden()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors.border_variant)
                    .child(
                        div()
                            .debug_selector(move || format!("note-text-{n}"))
                            .child(wrapped(&note.text)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1p5()
                            .text_meta()
                            .text_color(colors.text_muted)
                            .child(kept_by(&note.from))
                            .child("·")
                            .child(since(&note.at, now))
                            .child(
                                div()
                                    .font_family("Lilex")
                                    .text_color(colors.text_placeholder)
                                    .child(note.id.clone()),
                            ),
                    ),
            );
        }
        pane.child(div().h(px(8.)).flex_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_says_who_kept_it() {
        assert_eq!(kept_by("user"), "คุณ");
        assert_eq!(kept_by("3f2a9c1e"), "session");
    }
}
