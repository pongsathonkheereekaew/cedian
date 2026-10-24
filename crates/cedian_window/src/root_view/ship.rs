//! Commit and pull request from the latest receipt, cedia's (`cedia commit`, `cedia pr`), as
//! the Claude desktop app and Cursor end a change: Commit asks cedia for a message for the
//! files the thread changed, shows it in a card over the composer to edit, and commits only
//! those files when the person says so; เปิด PR asks where the branch would go, says it, and
//! pushes and opens the pull request only on the person's yes. cedia refuses the default
//! branch and a detached HEAD, and the card says why.

use std::path::PathBuf;

use cedian_protocol::{CommitDraft, Committed, PrOpened, PrPlan};
use cedian_wire::Request;
use editor::Editor;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use theme::ActiveTheme as _;

use super::RootView;
use crate::{line_breaks, type_scale::TypeScale as _};

/// The card over the composer, for one thread.
pub(super) struct ShipCard {
    pub thread: String,
    /// Where cedia runs: the thread's worktree, else the project.
    cwd: PathBuf,
    step: Step,
}

enum Step {
    /// Waiting on cedia, which is doing this.
    Busy(&'static str),
    /// The message cedia drafted, in a field to edit, and the files it covers.
    Draft {
        files: Vec<String>,
        branch: Option<String>,
        message: Entity<Editor>,
    },
    /// Where a pull request would go, asked before the push.
    Plan {
        branch: String,
        base: Option<String>,
    },
    /// What happened, or why not; a commit offers the pull request next.
    Said {
        title: &'static str,
        text: String,
        url: Option<String>,
        then_pr: bool,
    },
}

/// What a commit did, or why it did not.
fn committed_text(result: &Committed) -> (&'static str, String, bool) {
    match result {
        Committed::Variant0 {
            commit,
            subject,
            files,
        } => (
            "commit แล้ว",
            format!(
                "{} {subject}\n{}",
                &commit[..commit.len().min(7)],
                files.join(", ")
            ),
            true,
        ),
        Committed::Variant1 { error } => ("ยังไม่ได้ commit", error.clone(), false),
    }
}

impl RootView {
    /// The open thread and where cedia should run for it.
    fn ship_target(&self) -> Option<(String, PathBuf)> {
        let info = self.project().info()?;
        let cwd = info
            .cwd
            .clone()
            .unwrap_or_else(|| self.project().cwd.clone());
        Some((info.thread.id.clone(), cwd))
    }

    /// Commit: cedia drafts the message; the field waits for it.
    pub(super) fn start_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((thread, cwd)) = self.ship_target() else {
            return;
        };
        self.restore = None;
        self.request(Request::CommitDraft {
            cwd: cwd.clone(),
            thread: thread.clone(),
        });
        // Made now, while there is a window to make it in; the draft fills it.
        let message = cx.new(|cx| {
            let mut editor = Editor::auto_height(3, 12, window, cx);
            editor.set_placeholder_text("ข้อความ commit", window, cx);
            editor
        });
        self.ship_message = Some(message);
        self.ship = Some(ShipCard {
            thread,
            cwd,
            step: Step::Busy("cedia กำลังเขียนข้อความ commit จาก diff…"),
        });
        cx.notify();
    }

    /// เปิด PR: where the branch would go comes first.
    pub(super) fn start_pr(&mut self, cx: &mut Context<Self>) {
        let Some((thread, cwd)) = self.ship_target() else {
            return;
        };
        self.restore = None;
        self.ask_pr_plan(thread, cwd, cx);
    }

    fn ask_pr_plan(&mut self, thread: String, cwd: PathBuf, cx: &mut Context<Self>) {
        self.request(Request::PrCheck {
            cwd: cwd.clone(),
            thread: thread.clone(),
        });
        self.ship = Some(ShipCard {
            thread,
            cwd,
            step: Step::Busy("กำลังดู branch…"),
        });
        cx.notify();
    }

    fn ship_step(&mut self, thread: &str, step: Step, cx: &mut Context<Self>) {
        if let Some(card) = self.ship.as_mut().filter(|card| card.thread == thread) {
            card.step = step;
            cx.notify();
        }
    }

    pub(super) fn on_commit_draft(
        &mut self,
        thread: String,
        draft: CommitDraft,
        cx: &mut Context<Self>,
    ) {
        let step = match draft {
            CommitDraft::Variant0 {
                files,
                message: text,
                branch,
            } => {
                let Some(message) = self.ship_message.clone() else {
                    return;
                };
                // The field's buffer takes the text without a window.
                if let Some(buffer) = message.read(cx).buffer().read(cx).as_singleton() {
                    buffer.update(cx, |buffer, cx| buffer.set_text(text, cx));
                }
                Step::Draft {
                    files,
                    branch,
                    message,
                }
            }
            CommitDraft::Variant1 { error } => Step::Said {
                title: "ยังไม่ได้ commit",
                text: error,
                url: None,
                then_pr: false,
            },
        };
        self.ship_step(&thread, step, cx);
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        let Some(card) = &self.ship else {
            return;
        };
        let Step::Draft { files, message, .. } = &card.step else {
            return;
        };
        let text = message.read(cx).text(cx).trim().to_owned();
        if text.is_empty() {
            return;
        }
        let (thread, cwd, files) = (card.thread.clone(), card.cwd.clone(), files.clone());
        self.request(Request::Commit {
            cwd,
            thread: thread.clone(),
            message: text,
            files,
        });
        self.ship_step(&thread, Step::Busy("กำลัง commit…"), cx);
    }

    pub(super) fn on_committed(
        &mut self,
        thread: String,
        result: Committed,
        cx: &mut Context<Self>,
    ) {
        let (title, text, done) = committed_text(&result);
        self.ship_step(
            &thread,
            Step::Said {
                title,
                text,
                url: None,
                then_pr: done && self.can_pr,
            },
            cx,
        );
    }

    pub(super) fn on_pr_plan(&mut self, thread: String, plan: PrPlan, cx: &mut Context<Self>) {
        let step = match (plan.blocked, plan.branch) {
            (None, Some(branch)) => Step::Plan {
                branch,
                base: plan.base,
            },
            (blocked, _) => Step::Said {
                title: "ยังเปิด PR ไม่ได้",
                text: blocked.unwrap_or_else(|| "ไม่ได้อยู่บน branch".into()),
                url: None,
                then_pr: false,
            },
        };
        self.ship_step(&thread, step, cx);
    }

    fn open_pr(&mut self, cx: &mut Context<Self>) {
        let Some(card) = &self.ship else {
            return;
        };
        let (thread, cwd) = (card.thread.clone(), card.cwd.clone());
        self.request(Request::Pr {
            cwd,
            thread: thread.clone(),
        });
        self.ship_step(&thread, Step::Busy("กำลัง push และเปิด PR…"), cx);
    }

    pub(super) fn on_pr_opened(
        &mut self,
        thread: String,
        result: PrOpened,
        cx: &mut Context<Self>,
    ) {
        let step = match result {
            PrOpened::Variant0 {
                url, branch, base, ..
            } => Step::Said {
                title: "เปิด PR แล้ว",
                text: [
                    Some(match &base {
                        Some(base) => format!("{branch} → {base}"),
                        None => branch,
                    }),
                    url.clone(),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("\n"),
                url,
                then_pr: false,
            },
            PrOpened::Variant1 { error } => Step::Said {
                title: "ยังไม่ได้เปิด PR",
                text: error,
                url: None,
                then_pr: false,
            },
        };
        self.ship_step(&thread, step, cx);
    }

    /// The card for the open thread, if it has one.
    pub(super) fn render_ship(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let card = self
            .ship
            .as_ref()
            .filter(|card| self.project().current.as_ref() == Some(&card.thread))?;
        let colors = cx.theme().colors();
        let tokens = crate::tokens::Tokens::of(cx);
        let button = |id: &'static str, primary: bool| {
            let b = div()
                .id(id)
                .debug_selector(move || id.into())
                .h(px(28.))
                .px_3()
                .flex()
                .items_center()
                .rounded(px(7.))
                .border_1()
                .cursor_pointer()
                .text_ui();
            if primary {
                b.bg(tokens.primary)
                    .border_color(tokens.primary)
                    .text_color(tokens.on_primary)
            } else {
                b.bg(colors.element_background)
                    .border_color(colors.border)
                    .text_color(colors.text)
                    .hover(|b| b.bg(colors.element_hover))
            }
        };
        let close = |word: &'static str| {
            button("ship-close", false)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.ship = None;
                    view.ship_message = None;
                    cx.notify();
                }))
                .child(word)
        };
        let muted = colors.text_muted;
        let (title, body, buttons): (&str, AnyElement, gpui::Div) = match &card.step {
            Step::Busy(doing) => (
                "Commit และ PR",
                div()
                    .text_message()
                    .text_color(muted)
                    .child(*doing)
                    .into_any_element(),
                div().flex().gap_2().child(close("ปิด")),
            ),
            Step::Draft {
                files,
                branch,
                message,
            } => (
                "Commit",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_meta().text_color(muted).child(format!(
                            "{} ไฟล์ที่ thread นี้แก้{}: {}",
                            files.len(),
                            branch
                                .as_ref()
                                .map(|b| format!(" บน {b}"))
                                .unwrap_or_default(),
                            files.join(", ")
                        )))
                    .child(
                        div()
                            .debug_selector(|| "ship-message".into())
                            .px_2()
                            .py_1p5()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.editor_background)
                            .text_message()
                            .child(message.clone()),
                    )
                    .into_any_element(),
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("ship-commit", true)
                            .on_click(cx.listener(|view, _, _, cx| view.commit(cx)))
                            .child("Commit"),
                    )
                    .child(close("ยกเลิก")),
            ),
            Step::Plan { branch, base } => (
                "เปิด PR",
                div()
                    .text_message()
                    .child(line_breaks::wrapped(&match base {
                        Some(base) => {
                            format!("push {branch} ขึ้น origin แล้วเปิด pull request เข้า {base} ผ่าน gh")
                        }
                        None => format!("push {branch} ขึ้น origin แล้วเปิด pull request ผ่าน gh"),
                    }))
                    .into_any_element(),
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("ship-pr", true)
                            .on_click(cx.listener(|view, _, _, cx| view.open_pr(cx)))
                            .child("push และเปิด PR"),
                    )
                    .child(close("ยกเลิก")),
            ),
            Step::Said {
                title,
                text,
                url,
                then_pr,
            } => {
                let link = url.clone().map(|url| {
                    button("ship-open-link", true)
                        .on_click(move |_, _, cx| cx.open_url(&url))
                        .child("เปิดใน browser")
                });
                let next = then_pr.then(|| {
                    let (thread, cwd) = (card.thread.clone(), card.cwd.clone());
                    button("ship-then-pr", true)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.ask_pr_plan(thread.clone(), cwd.clone(), cx)
                        }))
                        .child("เปิด PR")
                });
                (
                    *title,
                    div()
                        .text_message()
                        .child(line_breaks::wrapped(text))
                        .into_any_element(),
                    div()
                        .flex()
                        .gap_2()
                        .children(link)
                        .children(next)
                        .child(close("ปิด")),
                )
            }
        };
        Some(
            div()
                .debug_selector(|| "ship-card".into())
                .flex()
                .flex_col()
                .gap_2()
                .mx_3()
                .my_2()
                .p_3()
                .rounded(px(12.))
                .border_1()
                .border_color(colors.border)
                .bg(colors.element_background)
                .text_color(colors.text)
                .child(
                    div()
                        .text_label()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title.to_owned()),
                )
                .child(body)
                .child(buttons)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_commit_says_its_short_hash_subject_and_files_or_why_not() {
        let done: Committed = serde_json::from_str(
            r#"{"commit":"abc1234def","subject":"Make f.txt say good","files":["f.txt","g.txt"]}"#,
        )
        .expect("committed");
        let (title, text, ok) = committed_text(&done);
        assert_eq!(title, "commit แล้ว");
        assert_eq!(text, "abc1234 Make f.txt say good\nf.txt, g.txt");
        assert!(ok);
        let refused: Committed =
            serde_json::from_str(r#"{"error":"nothing this thread changed differs from HEAD"}"#)
                .expect("refused");
        let (_, text, ok) = committed_text(&refused);
        assert!(text.contains("nothing this thread changed"));
        assert!(!ok);
    }
}
