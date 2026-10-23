//! Where the person types to the thread: Enter or the send button sends, Shift-Enter starts a
//! new line. Messages the running thread has not seen yet sit inside it above the text as
//! rows, to edit or take back (Interface, Polish: composer). A `/` at the start lists the
//! project's commands (cedia's skills) over the field, narrowed as the person types. An `@`
//! starting the last word lists the files of the thread's directory, matched as `⌘P` does;
//! a pick puts `@<path> ` in its place, which the model reads as the file to look at.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::type_scale::TypeScale as _;
use cedian_protocol::CommandEntry;
use editor::{Editor, EditorEvent};
use fuzzy::{StringMatch, StringMatchCandidate};
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, FontWeight, HighlightStyle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledText, Task, Window, actions, deferred, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

actions!(
    composer,
    [Submit, CommandUp, CommandDown, CommandPick, CommandDismiss]
);

/// The most commands the list shows at once.
const SHOWN_COMMANDS: usize = 8;

pub struct Composer {
    editor: Entity<Editor>,
    queue: Vec<Queued>,
    /// The project's `/` commands, as cedia last listed them.
    commands: Vec<CommandEntry>,
    /// The highlighted command in the list.
    selected: usize,
    /// The text the person closed the list on with Escape; it opens again once it changes.
    dismissed: Option<String>,
    /// The field holds one of the person's sent messages, to send again in a fork.
    editing: bool,
    /// The files `@` lists: the directory and its file count they were built for, and each
    /// path relative to it.
    files: (PathBuf, usize),
    candidates: Arc<Vec<StringMatchCandidate>>,
    /// The `@` word being matched and the files that match it, best first.
    mention: Option<(String, Vec<StringMatch>)>,
    cancel: Arc<AtomicBool>,
    search: Option<Task<()>>,
}

/// A message sent to a running thread that no session has seen yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Queued {
    pub id: String,
    pub text: SharedString,
}

pub enum ComposerEvent {
    Submit(String),
    /// A `/` was typed at the start: the project's commands are wanted, fresh.
    WantCommands,
    /// The person stopped editing a sent message.
    CancelEdit,
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
        cx.subscribe(&editor, |composer, _, event, cx| {
            if matches!(event, EditorEvent::BufferEdited) {
                composer.edited(cx);
            }
        })
        .detach();
        Self {
            editor,
            queue: Vec::new(),
            commands: Vec::new(),
            selected: 0,
            dismissed: None,
            editing: false,
            files: (PathBuf::new(), 0),
            candidates: Arc::new(Vec::new()),
            mention: None,
            cancel: Arc::new(AtomicBool::new(false)),
            search: None,
        }
    }

    /// Whether `@` already lists these files.
    pub fn has_files(&self, root: &Path, count: usize) -> bool {
        self.files.0 == root && self.files.1 == count
    }

    /// The files `@` lists, relative to `root`; built again only when they change.
    pub fn set_files(&mut self, root: &Path, files: &[PathBuf]) {
        if self.has_files(root, files.len()) {
            return;
        }
        self.files = (root.to_owned(), files.len());
        self.candidates = Arc::new(
            files
                .iter()
                .enumerate()
                .map(|(n, path)| StringMatchCandidate::new(n, &path.to_string_lossy()))
                .collect(),
        );
    }

    /// Matches the `@` word at the end of the text in the background, or drops the list.
    fn find_mention(&mut self, text: &str, cx: &mut Context<Self>) {
        self.cancel.store(true, Ordering::Relaxed);
        let Some(word) = mention_word(text) else {
            self.mention = None;
            self.search = None;
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        let candidates = self.candidates.clone();
        let word = word.to_owned();
        let executor = cx.background_executor().clone();
        self.search = Some(cx.spawn(async move |this, cx| {
            let matches = if word.is_empty() {
                candidates
                    .iter()
                    .take(SHOWN_COMMANDS)
                    .map(|c| StringMatch {
                        candidate_id: c.id,
                        score: 0.,
                        positions: Vec::new(),
                        string: c.string.clone(),
                    })
                    .collect()
            } else {
                fuzzy::match_strings(
                    candidates.as_slice(),
                    &word,
                    false,
                    true,
                    SHOWN_COMMANDS,
                    &cancel,
                    executor,
                )
                .await
            };
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            this.update(cx, |composer, cx| {
                composer.mention = Some((word, matches));
                composer.selected = 0;
                cx.notify();
            })
            .ok();
        }));
    }

    /// The files listed for the `@` word, when the list is open.
    fn mentions(&self, cx: &gpui::App) -> &[StringMatch] {
        let text = self.editor.read(cx).text(cx);
        match &self.mention {
            Some((word, found))
                if self.dismissed.as_ref() != Some(&text)
                    && mention_word(&text) == Some(word.as_str()) =>
            {
                found
            }
            _ => &[],
        }
    }

    /// Replaces the `@` word with the picked file's path and a space.
    fn pick_mention(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.mentions(cx).get(index).map(|m| m.string.clone()) else {
            return;
        };
        let text = self.editor.read(cx).text(cx);
        let Some(word) = mention_word(&text) else {
            return;
        };
        let keep = &text[..text.len() - word.len() - 1];
        let text = format!("{keep}@{path} ");
        self.mention = None;
        self.editor.update(cx, |editor, cx| {
            editor.set_text(text, window, cx);
            editor.move_to_end(&editor::actions::MoveToEnd, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
        });
        cx.notify();
    }

    /// Puts a sent message in the field to edit, under a line saying a send forks the
    /// thread there.
    pub fn edit_sent(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = true;
        self.dismissed = Some(text.to_owned());
        self.editor.update(cx, |editor, cx| {
            editor.set_text(text.to_owned(), window, cx);
            editor.move_to_end(&editor::actions::MoveToEnd, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
        });
        cx.notify();
    }

    /// Leaves editing a sent message and keeps the text as a draft: the thread changed.
    pub fn forget_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            self.editing = false;
            cx.notify();
        }
    }

    /// Leaves editing a sent message: the field empties.
    pub fn stop_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing {
            self.editing = false;
            self.editor
                .update(cx, |editor, cx| editor.clear(window, cx));
            cx.notify();
        }
    }

    pub fn set_commands(&mut self, commands: Vec<CommandEntry>, cx: &mut Context<Self>) {
        self.commands = commands;
        self.selected = 0;
        cx.notify();
    }

    fn edited(&mut self, cx: &mut Context<Self>) {
        let text = self.editor.read(cx).text(cx);
        if text == "/" {
            cx.emit(ComposerEvent::WantCommands);
        }
        if self.dismissed.as_ref().is_some_and(|d| *d != text) {
            self.dismissed = None;
        }
        self.find_mention(&text, cx);
        self.selected = 0;
        cx.notify();
    }

    /// The commands the text so far names, when the list is open: the text is `/` and a
    /// word, nothing after it yet. A name that starts with the word comes first.
    fn matching(&self, cx: &gpui::App) -> Vec<&CommandEntry> {
        let text = self.editor.read(cx).text(cx);
        if self.dismissed.as_ref() == Some(&text) {
            return Vec::new();
        }
        let Some(word) = text.strip_prefix('/') else {
            return Vec::new();
        };
        if word.contains(char::is_whitespace) {
            return Vec::new();
        }
        let word = word.to_lowercase();
        let mut found: Vec<&CommandEntry> = self
            .commands
            .iter()
            .filter(|c| c.name.to_lowercase().contains(&word))
            .collect();
        found.sort_by_key(|c| !c.name.to_lowercase().starts_with(&word));
        found
    }

    /// How many rows the open list shows: the files for `@`, else the commands.
    fn rows(&self, cx: &gpui::App) -> usize {
        let files = self.mentions(cx).len();
        if files > 0 {
            files
        } else {
            self.matching(cx).len().min(SHOWN_COMMANDS)
        }
    }

    fn command_up(&mut self, _: &CommandUp, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.rows(cx);
        if n > 0 {
            self.selected = (self.selected + n - 1) % n;
            cx.notify();
        }
    }

    fn command_down(&mut self, _: &CommandDown, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.rows(cx);
        if n > 0 {
            self.selected = (self.selected + 1) % n;
            cx.notify();
        }
    }

    fn command_pick(&mut self, _: &CommandPick, window: &mut Window, cx: &mut Context<Self>) {
        if self.mentions(cx).is_empty() {
            self.pick(self.selected, window, cx);
        } else {
            self.pick_mention(self.selected, window, cx);
        }
    }

    /// Puts `/<name> ` in the field, for the text the command is for.
    fn pick(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.matching(cx).get(index).map(|c| c.name.clone()) else {
            return;
        };
        self.use_command(&name, window, cx);
    }

    /// Puts `/<name> ` in the field and focuses it: a command picked here or elsewhere.
    pub fn use_command(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = format!("/{name} ");
        self.dismissed = Some(text.clone());
        self.editor.update(cx, |editor, cx| {
            editor.set_text(text, window, cx);
            editor.move_to_end(&editor::actions::MoveToEnd, window, cx);
            window.focus(&editor.focus_handle(cx), cx);
        });
        cx.notify();
    }

    fn command_dismiss(&mut self, _: &CommandDismiss, _: &mut Window, cx: &mut Context<Self>) {
        self.dismissed = Some(self.editor.read(cx).text(cx));
        cx.notify();
    }

    /// The list over the field, opening upward.
    fn command_list(&self, found: &[&CommandEntry], cx: &mut Context<Self>) -> gpui::AnyElement {
        let colors = cx.theme().colors();
        let mut list = div()
            .id("command-list")
            .flex()
            .flex_col()
            .p(px(5.))
            .rounded(px(12.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_lg()
            .text_ui()
            .text_color(colors.text)
            .on_mouse_down_out(cx.listener(|composer, _, _, cx| {
                composer.dismissed = Some(composer.editor.read(cx).text(cx));
                cx.notify();
            }));
        for (n, command) in found.iter().take(SHOWN_COMMANDS).enumerate() {
            let on = n == self.selected;
            list = list.child(
                div()
                    .id(("command", n))
                    .flex_none()
                    .px(px(9.))
                    .py(px(6.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .rounded(px(7.))
                    .cursor_pointer()
                    .when(on, |r| r.bg(colors.element_selected))
                    .hover(|r| r.bg(colors.element_hover))
                    .on_click(
                        cx.listener(move |composer, _, window, cx| composer.pick(n, window, cx)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .max_w(px(220.))
                            .truncate()
                            .font(crate::tokens::code_font())
                            .child(format!("/{}", command.name)),
                    )
                    .when_some(command.argument_hint.clone(), |r, hint| {
                        r.child(
                            div()
                                .flex_none()
                                .max_w(px(120.))
                                .truncate()
                                .font(crate::tokens::code_font())
                                .text_meta()
                                .text_color(colors.text_placeholder)
                                .child(hint),
                        )
                    })
                    .child(
                        crate::tooltip::cut("description", command.description.clone())
                            .flex_grow(1.)
                            .text_meta()
                            .text_color(colors.text_muted),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_label()
                            .text_color(colors.text_placeholder)
                            .child(
                                if command.source == cedian_protocol::CommandEntrySource::User {
                                    "ของคุณ"
                                } else {
                                    "repo"
                                },
                            ),
                    ),
            );
        }
        let more = found.len().saturating_sub(SHOWN_COMMANDS);
        if more > 0 {
            list = list.child(
                div()
                    .px(px(9.))
                    .py_1()
                    .text_label()
                    .text_color(colors.text_placeholder)
                    .child(format!("และอีก {more} · พิมพ์ต่อเพื่อกรอง")),
            );
        }
        deferred(
            div()
                .absolute()
                .bottom_full()
                .left_4()
                .right_4()
                .mb_1()
                .child(list),
        )
        .with_priority(1)
        .into_any_element()
    }

    /// The files for `@`, over the field as the commands are, with the matched letters marked.
    fn file_list(&self, found: &[StringMatch], cx: &mut Context<Self>) -> gpui::AnyElement {
        let colors = cx.theme().colors();
        let marked = HighlightStyle {
            color: Some(colors.text_accent),
            font_weight: Some(FontWeight::SEMIBOLD),
            ..Default::default()
        };
        let mut list = div()
            .id("file-list")
            .debug_selector(|| "mention-list".into())
            .flex()
            .flex_col()
            .p(px(5.))
            .rounded(px(12.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_lg()
            .text_ui()
            .text_color(colors.text)
            .on_mouse_down_out(cx.listener(|composer, _, _, cx| {
                composer.dismissed = Some(composer.editor.read(cx).text(cx));
                cx.notify();
            }));
        for (n, found) in found.iter().enumerate() {
            let path = StyledText::new(found.string.clone())
                .with_highlights(found.ranges().map(|range| (range, marked)));
            list = list.child(
                div()
                    .id(("mention", n))
                    .flex_none()
                    .px(px(9.))
                    .py(px(6.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(7.))
                    .cursor_pointer()
                    .when(n == self.selected, |r| r.bg(colors.element_selected))
                    .hover(|r| r.bg(colors.element_hover))
                    .on_click(cx.listener(move |composer, _, window, cx| {
                        composer.pick_mention(n, window, cx)
                    }))
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
                            .font(crate::tokens::code_font())
                            .child(path),
                    ),
            );
        }
        deferred(
            div()
                .absolute()
                .bottom_full()
                .left_4()
                .right_4()
                .mb_1()
                .child(list),
        )
        .with_priority(1)
        .into_any_element()
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

    #[cfg(test)]
    pub fn text(&self, cx: &gpui::App) -> String {
        self.editor.read(cx).text(cx)
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.editor.focus_handle(cx), cx);
    }

    fn submit(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        self.dismissed = None;
        self.editing = false;
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
        let files = self.mentions(cx).to_vec();
        let found = self.matching(cx);
        let listing = !found.is_empty() || !files.is_empty();
        let list = if !files.is_empty() {
            Some(self.file_list(&files, cx))
        } else {
            (!found.is_empty()).then(|| self.command_list(&found, cx))
        };
        let colors = cx.theme().colors();
        let tokens = crate::tokens::Tokens::of(cx);
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
                .rounded(px(10.))
                .bg(colors.element_hover)
                .text_meta()
                .child(
                    div()
                        .flex_none()
                        .text_label()
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
        let send = div()
            .id("send")
            .flex_none()
            .size(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(9.))
            .bg(tokens.primary)
            .cursor_pointer()
            .hover(|b| b.opacity(0.9))
            .on_click(cx.listener(|view, _, window, cx| view.submit(&Submit, window, cx)))
            .child(
                svg()
                    .path("icons/arrow_up.svg")
                    .size(px(16.))
                    .text_color(tokens.on_primary),
            );
        div()
            .key_context("Composer")
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::command_up))
            .on_action(cx.listener(Self::command_down))
            .on_action(cx.listener(Self::command_pick))
            .on_action(cx.listener(Self::command_dismiss))
            .relative()
            .children(list)
            .px_4()
            .pt_1()
            .pb_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p(px(10.))
                    .rounded(px(20.))
                    .border_1()
                    .border_color(colors.border_selected)
                    .bg(colors.elevated_surface_background)
                    .shadow_md()
                    .when(self.editing, |c| {
                        c.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_1()
                                .rounded(px(10.))
                                .bg(colors.element_hover)
                                .text_meta()
                                .child(
                                    div()
                                        .flex_none()
                                        .text_label()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(working)
                                        .child("แก้ไขข้อความ"),
                                )
                                .child(
                                    div()
                                        .flex_grow(1.)
                                        .min_w_0()
                                        .truncate()
                                        .text_color(colors.text_muted)
                                        .child("ส่งแล้วจะแยกเป็น thread ใหม่จากข้อความนี้ thread เดิมยังอยู่"),
                                )
                                .child(button(("edit-cancel", 0), "ยกเลิก").on_click(cx.listener(
                                    |composer, _, window, cx| {
                                        composer.stop_editing(window, cx);
                                        cx.emit(ComposerEvent::CancelEdit);
                                    },
                                ))),
                        )
                    })
                    .children(queue)
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap_2()
                            .child(
                                div()
                                    // The list's keys (up, down, Tab, Enter, Escape) while
                                    // it is open.
                                    .when(listing, |d| d.key_context("CommandList"))
                                    .flex_grow(1.)
                                    .min_w_0()
                                    .min_h(px(32.))
                                    .pl_1()
                                    .py(px(6.))
                                    .child(self.editor.clone()),
                            )
                            .child(send),
                    ),
            )
    }
}

/// The word after an `@` that starts the text's last word, while it is being typed: the text
/// does not end in a space.
fn mention_word(text: &str) -> Option<&str> {
    let last = text.rsplit(char::is_whitespace).next()?;
    last.strip_prefix('@')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mention_is_the_last_word_after_an_at() {
        assert_eq!(mention_word("look at @src/ma"), Some("src/ma"));
        assert_eq!(mention_word("@"), Some(""));
        assert_eq!(mention_word("look at @src/main.rs "), None);
        assert_eq!(mention_word("mail me@x"), None);
        assert_eq!(mention_word("plain"), None);
    }
}
