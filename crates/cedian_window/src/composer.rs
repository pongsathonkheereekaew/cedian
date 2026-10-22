//! Where the person types to the thread: Enter sends, Shift-Enter starts a new line.

use editor::Editor;
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Window, actions, div,
};
use theme::ActiveTheme as _;

actions!(composer, [Submit]);

pub struct Composer {
    editor: Entity<Editor>,
}

pub enum ComposerEvent {
    Submit(String),
}

impl EventEmitter<ComposerEvent> for Composer {}

impl Composer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut editor = Editor::auto_height(1, 8, window, cx);
            editor.set_placeholder_text(
                "Ask cedia. Enter sends, Shift-Enter adds a line",
                window,
                cx,
            );
            editor
        });
        Self { editor }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.editor.focus_handle(cx), cx);
    }

    fn submit(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.editor.read(cx).text(cx);
        if text.trim().is_empty() {
            return;
        }
        self.editor
            .update(cx, |editor, cx| editor.clear(window, cx));
        cx.emit(ComposerEvent::Submit(text.trim_end().to_owned()));
    }
}

impl Render for Composer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div()
            .key_context("Composer")
            .on_action(cx.listener(Self::submit))
            .p_2()
            .border_t_1()
            .border_color(colors.border)
            .bg(colors.editor_background)
            .child(self.editor.clone())
    }
}
