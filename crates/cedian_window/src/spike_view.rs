//! Phase A spike: a Zed editor above a shell from Zed's terminal crate.

use std::path::PathBuf;

use editor::Editor;
use gpui::{
    AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div,
};
use theme::ActiveTheme as _;

use crate::shell_pane::ShellPane;

pub struct SpikeView {
    editor: Entity<Editor>,
    shell: Entity<ShellPane>,
}

impl SpikeView {
    pub fn new(cwd: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut editor = Editor::multi_line(window, cx);
            editor.set_text("cedian spike: Zed's editor in a GPUI window.\n", window, cx);
            editor
        });
        window.focus(&editor.focus_handle(cx), cx);
        let shell = cx.new(|cx| ShellPane::new(cwd, window, cx));
        Self { editor, shell }
    }
}

impl Render for SpikeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .text_color(colors.text)
            .child(
                div()
                    .h_1_2()
                    .p_2()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(self.editor.clone()),
            )
            .child(div().h_1_2().child(self.shell.clone()))
    }
}
