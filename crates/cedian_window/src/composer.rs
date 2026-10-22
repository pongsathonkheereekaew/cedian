//! Where the person types to the thread: Enter sends, Shift-Enter starts a new line. Messages
//! the running thread has not seen yet sit above the text as chips, to edit or take back
//! (Interface, Agent pane: Composer).

use editor::Editor;
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, actions, div, px,
};
use theme::ActiveTheme as _;

actions!(composer, [Submit]);

pub struct Composer {
    editor: Entity<Editor>,
    queue: Vec<Queued>,
}

/// A message sent to a running thread that no session has seen yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Queued {
    pub id: String,
    pub text: SharedString,
}

pub enum ComposerEvent {
    Submit(String),
    /// Take the message out of the thread's queue.
    Unsend(String),
}

impl EventEmitter<ComposerEvent> for Composer {}

impl Composer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut editor = Editor::auto_height(1, 8, window, cx);
            editor.set_placeholder_text("สั่ง cedia · Enter ส่ง, Shift-Enter ขึ้นบรรทัดใหม่", window, cx);
            editor
        });
        Self {
            editor,
            queue: Vec::new(),
        }
    }

    pub fn set_queue(&mut self, queue: Vec<Queued>, cx: &mut Context<Self>) {
        if queue != self.queue {
            self.queue = queue;
            cx.notify();
        }
    }

    /// Takes the message back and puts its text in the composer to change and send again.
    fn edit(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(queued) = self.queue.get(index).cloned() else {
            return;
        };
        self.editor.update(cx, |editor, cx| {
            editor.set_text(queued.text.to_string(), window, cx);
            window.focus(&editor.focus_handle(cx), cx);
        });
        self.remove(index, cx);
    }

    fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.queue.len() {
            let queued = self.queue.remove(index);
            cx.emit(ComposerEvent::Unsend(queued.id));
            cx.notify();
        }
    }

    /// Puts text the person picked elsewhere (a selection in the editor or the shell) in at
    /// the cursor as a fenced block, and focuses the field.
    pub fn attach(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let block = format!("```\n{}\n```\n", text.trim_end_matches('\n'));
        self.editor.update(cx, |editor, cx| {
            editor.insert(&block, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
        });
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
        let working = cx.theme().status().info;
        let button = |id: (&'static str, usize), label: &'static str| {
            div()
                .id(id)
                .flex_none()
                .h(px(22.))
                .px_2()
                .flex()
                .items_center()
                .rounded(px(6.))
                .cursor_pointer()
                .text_color(colors.text_muted)
                .hover(|b| b.bg(colors.element_hover).text_color(colors.text))
                .child(label)
        };
        let queue = self.queue.iter().enumerate().map(|(n, queued)| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded(px(8.))
                .bg(colors.element_background)
                .text_size(px(12.))
                .line_height(px(18.))
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(working)
                        .child("ส่งที่ turn ถัดไป"),
                )
                .child(
                    div()
                        .flex_grow(1.)
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_color(colors.text)
                        .child(queued.text.clone()),
                )
                .child(
                    button(("queued-edit", n), "แก้")
                        .on_click(cx.listener(move |view, _, window, cx| view.edit(n, window, cx))),
                )
                .child(
                    button(("queued-remove", n), "ลบ")
                        .on_click(cx.listener(move |view, _, _, cx| view.remove(n, cx))),
                )
        });
        div()
            .key_context("Composer")
            .on_action(cx.listener(Self::submit))
            .px_4()
            .pt_1()
            .pb_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.editor_background)
                    .children(queue)
                    .child(div().px_1().py(px(2.)).child(self.editor.clone())),
            )
    }
}
