//! The Outputs block (Interface, Blocks): the files the project's threads produced, across
//! threads, the most recently changed first, each with the thread that changed it last. The
//! daemon reads them from the `changed` paths of the threads' snapshots; a click shows the file
//! in the Editor. A file a later change or a removed worktree took away stays listed, off.

use std::path::PathBuf;

use cedian_index::Output;
use gpui::{
    Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use theme::ActiveTheme as _;

use crate::{thread_list::since, type_scale::TypeScale as _};

pub enum OutputsEvent {
    /// Show this file (an absolute path) in the Editor.
    Open(PathBuf),
}

#[derive(Default)]
pub struct OutputsPane {
    /// `None` until the daemon has answered.
    outputs: Option<Vec<Output>>,
}

impl EventEmitter<OutputsEvent> for OutputsPane {}

impl OutputsPane {
    pub fn set(&mut self, outputs: Vec<Output>, cx: &mut Context<Self>) {
        self.outputs = Some(outputs);
        cx.notify();
    }

    /// Another project: what was listed was the last one's.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.outputs = None;
        cx.notify();
    }
}

/// How many threads the files came from.
fn threads_of(outputs: &[Output]) -> usize {
    let mut threads: Vec<&str> = outputs.iter().map(|o| o.thread.as_str()).collect();
    threads.sort_unstable();
    threads.dedup();
    threads.len()
}

impl Render for OutputsPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let mut pane = div()
            .id("outputs")
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .text_ui()
            .text_color(colors.text);
        let outputs = match &self.outputs {
            Some(outputs) if !outputs.is_empty() => outputs,
            shown => {
                let word = if shown.is_none() {
                    "กำลังอ่าน"
                } else {
                    "thread ในโปรเจกต์นี้ยังไม่ได้สร้างหรือแก้ไฟล์"
                };
                return pane.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_color(colors.text_muted)
                        .child(word),
                );
            }
        };
        let now = chrono::Utc::now();
        pane = pane.child(
            div()
                .px_3()
                .pt_2()
                .pb_1()
                .text_label()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colors.text_muted)
                .child(format!(
                    "{} ไฟล์ จาก {} thread",
                    outputs.len(),
                    threads_of(outputs)
                )),
        );
        for (n, output) in outputs.iter().enumerate() {
            let when = since(&output.at, now);
            let path = output.cwd.join(&output.path);
            pane = pane.child(
                div()
                    .id(("output", n))
                    .debug_selector(move || format!("output-{n}"))
                    .flex_none()
                    .px_3()
                    .py_1()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .when(output.exists, |row| {
                        row.cursor_pointer()
                            .hover(|r| r.bg(colors.element_hover))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(OutputsEvent::Open(path.clone()))
                            }))
                    })
                    .child(
                        crate::tooltip::cut(("output-path", n), output.path.clone())
                            .debug_selector(move || format!("output-path-{n}"))
                            .when(!output.exists, |p| p.text_color(colors.text_placeholder)),
                    )
                    .child(
                        div()
                            .debug_selector(move || format!("output-meta-{n}"))
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .min_w_0()
                            .text_meta()
                            .text_color(colors.text_muted)
                            .child(crate::tooltip::cut(
                                ("output-thread", n),
                                SharedString::from(output.title.clone()),
                            ))
                            .child(div().flex_none().child(if output.exists {
                                when
                            } else {
                                format!("{when} · ไม่มีแล้ว")
                            })),
                    ),
            );
        }
        pane.child(div().h(px(8.)).flex_none())
    }
}
