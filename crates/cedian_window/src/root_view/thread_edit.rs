//! Renaming and deleting a thread from the thread list (Interface, Phase C: Threads), as in the
//! Claude desktop app: the pencil on a row turns its title into a field (`⏎` keeps the new
//! title, `esc` or leaving it keeps the old), and the bin asks in the row first, since a delete
//! cannot be undone. Both go through cedia (`threads rename`, `threads delete`), whose files the
//! list is read from; the row changes at once and the next read of the list agrees.

use editor::{Editor, EditorEvent};
use gpui::{
    AnyElement, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, actions,
    div, px,
};
use theme::ActiveTheme as _;

use super::RootView;
use crate::type_scale::TypeScale as _;

actions!(thread_edit, [ConfirmRename, CancelRename]);

/// The row whose title is being edited, and its field.
pub(super) struct Renaming {
    pub thread: String,
    field: Entity<Editor>,
}

impl RootView {
    /// The pencil: the row's title becomes a field holding it, all selected.
    pub(super) fn start_rename(
        &mut self,
        thread: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title = self.thread_title(thread);
        let field = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_text(title, window, cx);
            editor.select_all(&editor::actions::SelectAll, window, cx);
            editor
        });
        cx.subscribe_in(&field, window, |view, _, event, window, cx| {
            if matches!(event, EditorEvent::Blurred) {
                view.confirm_rename(&ConfirmRename, window, cx);
            }
        })
        .detach();
        // The press on the row asked for the block's input; the field is the input now.
        self.refocus = false;
        window.focus(&field.focus_handle(cx), cx);
        self.deleting = None;
        self.renaming = Some(Renaming {
            thread: thread.to_owned(),
            field,
        });
        cx.notify();
    }

    /// `⏎`: the new title, when it is one, goes to cedia and shows at once.
    pub(super) fn confirm_rename(
        &mut self,
        _: &ConfirmRename,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(renaming) = self.renaming.take() else {
            return;
        };
        let title = renaming.field.read(cx).text(cx).trim().to_owned();
        if !title.is_empty() && title != self.thread_title(&renaming.thread) {
            let cwd = self.project().cwd.clone();
            if let Some(info) = self
                .project_mut()
                .threads
                .iter_mut()
                .find(|t| t.thread.id == renaming.thread)
            {
                info.thread.title = title.clone();
            }
            self.request(cedian_wire::Request::Rename {
                cwd,
                thread: renaming.thread,
                title,
            });
        }
        cx.notify();
    }

    pub(super) fn cancel_rename(
        &mut self,
        _: &CancelRename,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.renaming.take().is_some() {
            cx.notify();
        }
    }

    /// The bin: the row asks before anything goes.
    pub(super) fn ask_delete(&mut self, thread: &str, cx: &mut Context<Self>) {
        self.renaming = None;
        self.deleting = Some(thread.to_owned());
        cx.notify();
    }

    /// ลบ: the thread leaves the list now; cedia deletes its files.
    fn delete_thread(&mut self, thread: &str, cx: &mut Context<Self>) {
        self.deleting = None;
        let cwd = self.project().cwd.clone();
        self.project_mut().threads.retain(|t| t.thread.id != thread);
        self.request(cedian_wire::Request::Delete {
            cwd,
            thread: thread.to_owned(),
        });
        if self.project().current.as_deref() == Some(thread) {
            self.new_thread(cx);
        }
        cx.notify();
    }

    fn thread_title(&self, thread: &str) -> String {
        self.project()
            .threads
            .iter()
            .find(|t| t.thread.id == thread)
            .map_or_else(|| thread.to_owned(), |t| t.thread.title.clone())
    }

    /// The field in place of the row's title while it is renamed.
    pub(super) fn render_rename_field(
        &self,
        thread: &str,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let renaming = self.renaming.as_ref().filter(|r| r.thread == thread)?;
        let colors = cx.theme().colors();
        Some(
            div()
                .id("thread-rename")
                .debug_selector(|| "thread-rename".into())
                .key_context("ThreadRename")
                .on_action(cx.listener(Self::confirm_rename))
                .on_action(cx.listener(Self::cancel_rename))
                // A click in the field edits; it does not open the thread.
                .on_click(|_, _, cx| cx.stop_propagation())
                .flex_grow(1.)
                .min_w_0()
                .h(px(24.))
                .px_1p5()
                .flex()
                .items_center()
                .rounded(px(6.))
                .border_1()
                .border_color(colors.border_focused)
                .bg(colors.editor_background)
                .text_ui()
                .child(renaming.field.clone())
                .into_any_element(),
        )
    }

    /// The row while it asks: ลบถาวร? with ลบ and ยกเลิก.
    pub(super) fn render_delete_confirm(
        &self,
        thread: &str,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.deleting.as_deref() != Some(thread) {
            return None;
        }
        let colors = cx.theme().colors();
        let error = cx.theme().status().error;
        let button = |id: &'static str| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .flex_none()
                .h(px(22.))
                .px_2()
                .flex()
                .items_center()
                .rounded(px(6.))
                .cursor_pointer()
                .text_meta()
        };
        let delete = thread.to_owned();
        Some(
            div()
                .debug_selector(|| "thread-delete-confirm".into())
                .flex_grow(1.)
                .min_w_0()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .flex_grow(1.)
                        .min_w_0()
                        .truncate()
                        .text_ui()
                        .text_color(colors.text)
                        .child("ลบถาวร?"),
                )
                .child(
                    button("thread-delete-yes")
                        .bg(error.opacity(0.15))
                        .text_color(error)
                        .hover(|b| b.bg(error.opacity(0.25)))
                        .child("ลบ")
                        .on_click(cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            view.delete_thread(&delete, cx);
                        })),
                )
                .child(
                    button("thread-delete-no")
                        .text_color(colors.text_muted)
                        .hover(|b| b.bg(colors.element_hover))
                        .child("ยกเลิก")
                        .on_click(cx.listener(|view, _, _, cx| {
                            cx.stop_propagation();
                            view.deleting = None;
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}
