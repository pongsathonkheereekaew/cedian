//! The Editor block as an editor (Review and editing): a file from Files, `⌘P`, `⌘⇧F` or
//! Outputs opens to edit and `⌘S` writes it; a diff from Git stays read only. A file left with
//! unsaved edits keeps them until it comes back. Each second the file on disk is checked: a
//! change the agent made is read in while the person has no edits, and while they have, a line
//! over the Editor offers ใช้ของฉัน (Override: their text is written, and the thread told with
//! `cedia threads send`) or ใช้ของ agent (Discard).

use std::{path::PathBuf, time::Duration};

use cedian_layout::Kind;
use cedian_wire::Request;
use editor::Editor;
use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use theme::ActiveTheme as _;

use super::{RootView, blocks};
use crate::{
    editor_file::{DiskChange, EditorFile, Opened, Save},
    type_scale::TypeScale as _,
};

/// How often the open file is checked on disk.
const WATCH_EVERY: Duration = Duration::from_secs(1);

impl RootView {
    /// Shows `path` in the Editor, to edit when it is text, and opens the Editor when it is
    /// closed. Edits this file had when it was left come back.
    pub(super) fn show_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.keep_draft(cx);
        let (opened, file) = EditorFile::open(&path);
        let (text, editable) = match opened {
            Opened::Text(text) => (self.editor_drafts.remove(&path).unwrap_or(text), true),
            Opened::Shown(line) => (line, false),
        };
        let name = path
            .strip_prefix(self.project().shell_dir())
            .unwrap_or(&path)
            .display()
            .to_string();
        self.editor_file = file;
        self.editor_conflict = None;
        self.editor
            .update(cx, |editor, _| editor.set_read_only(!editable));
        load(&self.editor, text, window, cx);
        self.editor_edited(cx);
        self.watch_editor_file(window, cx);
        self.editor_shows = Some(name.into());
        self.open_kind(Kind::Editor, blocks::ToolTarget { against: None }, cx);
        cx.notify();
    }

    /// Puts `text` in the Editor, read only, under `name`: a diff, not a file.
    pub(super) fn show_text(
        &mut self,
        name: SharedString,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.keep_draft(cx);
        self.editor_file = None;
        self.editor_conflict = None;
        self.editor_watch = None;
        self.editor
            .update(cx, |editor, _| editor.set_read_only(true));
        load(&self.editor, text, window, cx);
        self.editor_dirty = false;
        self.editor_shows = Some(name);
        self.open_kind(Kind::Editor, blocks::ToolTarget { against: None }, cx);
        cx.notify();
    }

    /// Keeps the open file's unsaved edits for when it comes back.
    fn keep_draft(&mut self, cx: &mut Context<Self>) {
        if let Some(file) = self.editor_file.take() {
            let text = self.editor.read(cx).text(cx);
            if file.dirty(&text) {
                self.editor_drafts.insert(file.path, text);
            }
        }
    }

    pub(super) fn editor_edited(&mut self, cx: &mut Context<Self>) {
        let dirty = match &self.editor_file {
            Some(file) => file.dirty(&self.editor.read(cx).text(cx)),
            None => false,
        };
        if dirty != self.editor_dirty {
            self.editor_dirty = dirty;
            cx.notify();
        }
    }

    /// The Editor's tab: the file's name, with a dot while it has unsaved edits.
    pub(super) fn editor_label(&self) -> Option<SharedString> {
        let name = self.editor_shows.clone()?;
        Some(if self.editor_dirty {
            format!("{name} ●").into()
        } else {
            name
        })
    }

    /// `⌘S`: writes the person's text. Over an agent's change (Override) the thread is told,
    /// so the model sees its write was replaced.
    pub(super) fn save_editor(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.editor.read(cx).text(cx);
        let Some(file) = self.editor_file.as_mut() else {
            return;
        };
        if let Err(error) = file.save(&text) {
            self.error = Some(format!("บันทึก {} ไม่ได้: {error}", file.path.display()).into());
            cx.notify();
            return;
        }
        let path = file.path.clone();
        if self.editor_conflict.take().is_some()
            && let Some(thread) = self.project().current.clone()
        {
            self.request(Request::Send {
                cwd: self.project().cwd.clone(),
                thread,
                text: format!(
                    "{} ถูกแก้ใน editor และบันทึกทับสิ่งที่คุณเขียนไว้ อ่านไฟล์ใหม่ก่อนแก้ต่อ",
                    path.display()
                ),
            });
        }
        self.editor_dirty = false;
        cx.notify();
    }

    /// ใช้ของ agent: the person's edits give way to what the disk holds now.
    pub(super) fn discard_edits(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(text), Some(file)) = (self.editor_conflict.take(), self.editor_file.as_mut())
        else {
            return;
        };
        file.take(text.clone());
        load(&self.editor, text, window, cx);
        self.editor_edited(cx);
        cx.notify();
    }

    fn watch_editor_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor_file.is_none() {
            self.editor_watch = None;
            return;
        }
        self.editor_watch = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(WATCH_EVERY).await;
                let held = this
                    .update_in(cx, |view, window, cx| view.check_disk(window, cx))
                    .unwrap_or(false);
                if !held {
                    break;
                }
            }
        }));
    }

    /// Reads in an agent's change when the person has no edits, or holds it for their choice.
    /// False once the Editor holds no file.
    pub(super) fn check_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let text = self.editor.read(cx).text(cx);
        let Some(file) = self.editor_file.as_mut() else {
            return false;
        };
        match file.check(&text) {
            DiskChange::Same | DiskChange::Gone => {}
            DiskChange::Reload(disk) => {
                file.take(disk.clone());
                self.editor_conflict = None;
                load(&self.editor, disk, window, cx);
                self.editor_edited(cx);
                cx.notify();
            }
            DiskChange::Conflict(disk) => {
                if self.editor_conflict.as_ref() != Some(&disk) {
                    self.editor_conflict = Some(disk);
                    cx.notify();
                }
            }
        }
        true
    }

    /// The line over the Editor while an agent's change waits on the person's choice.
    pub(super) fn render_editor_conflict(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.editor_conflict.as_ref()?;
        let colors = cx.theme().colors();
        let warning = cx.theme().status().warning;
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .flex_none()
                .h(px(24.))
                .px_2()
                .flex()
                .items_center()
                .rounded(px(6.))
                .border_1()
                .border_color(colors.border)
                .cursor_pointer()
                .text_color(colors.text)
                .hover(|b| b.bg(colors.element_hover))
                .child(label)
        };
        Some(
            div()
                .debug_selector(|| "editor-conflict".into())
                .flex()
                .items_center()
                .gap_2()
                .mb_2()
                .px_3()
                .py_1()
                .rounded(px(8.))
                .border_1()
                .border_color(warning.opacity(0.3))
                .bg(warning.opacity(0.07))
                .text_meta()
                .child(
                    crate::tooltip::cut(
                        "conflict-line",
                        "agent แก้ไฟล์นี้ระหว่างที่คุณแก้อยู่ ยังไม่ได้บันทึกของคุณ",
                    )
                    .flex_grow(1.)
                    .text_color(colors.text),
                )
                .child(button("editor-override", "ใช้ของฉัน").on_click(
                    cx.listener(|view, _, window, cx| view.save_editor(&Save, window, cx)),
                ))
                .child(
                    button("editor-discard", "ใช้ของ agent").on_click(
                        cx.listener(|view, _, window, cx| view.discard_edits(window, cx)),
                    ),
                )
                .into_any_element(),
        )
    }
}

/// Puts `text` in the editor as what it was opened with: the replacement is no edit to undo,
/// so `⌘Z` never brings back the file shown before. The cursor goes to the start.
fn load(editor: &Entity<Editor>, text: String, window: &mut Window, cx: &mut App) {
    editor.update(cx, |editor, cx| {
        if let Some(buffer) = editor.buffer().read(cx).as_singleton() {
            buffer.update(cx, |buffer, cx| {
                buffer.start_transaction();
                buffer.set_text(text, cx);
                if let Some(id) = buffer.end_transaction(cx) {
                    buffer.forget_transaction(id);
                }
            });
        }
        editor.move_to_beginning(&editor::actions::MoveToBeginning, window, cx);
    });
}
