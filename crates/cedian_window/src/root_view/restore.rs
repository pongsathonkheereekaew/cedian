//! Whole-turn undo and rewind, cedia's (`cedia threads undo|redo|rewind --json`): the latest
//! receipt puts its turn's files back and forward again, and a message of the person's that
//! started a run takes the thread back to just before it, after a question, since a rewind
//! cannot be redone. What cedia did, or why it did not, shows in a card over the composer,
//! with a button to go again with `--force` when cedia says that would get past it.

use cedian_protocol::{Restored, RestoredAction, RewindResult};
use cedian_wire::Request;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, px,
};
use theme::ActiveTheme as _;

use super::RootView;
use crate::{line_breaks, type_scale::TypeScale as _};

/// The card over the composer.
pub(super) enum RestoreCard {
    /// Asked before a rewind.
    Confirm {
        thread: String,
        n: u32,
        prompt: String,
    },
    /// What cedia did, or why not; `retry` goes again with `--force`.
    Said {
        thread: String,
        text: String,
        retry: Option<Retry>,
    },
}

#[derive(Clone, Copy)]
pub(super) enum Retry {
    Restore { redo: bool },
    Rewind { n: u32 },
}

/// The files, named in one line.
fn files(names: &[String]) -> String {
    names.join(", ")
}

/// What an undo or a redo did, and whether to offer going again with `--force`.
pub(super) fn restored_text(result: &Restored) -> (String, Option<Retry>) {
    let redo = result.action == RestoredAction::Redo;
    let mut lines = Vec::new();
    if !result.restored.is_empty() {
        let verb = if redo {
            "ใส่ไฟล์กลับแล้ว"
        } else {
            "คืนไฟล์แล้ว"
        };
        lines.push(format!("{verb}: {}", files(&result.restored)));
    }
    if !result.skipped.is_empty() {
        lines.push(format!(
            "ไม่ได้แตะ {} เพราะถูกแก้หลังรอบนั้น",
            files(&result.skipped)
        ));
    }
    if lines.is_empty() {
        lines.push("ไฟล์เป็นแบบนั้นอยู่แล้ว".into());
    }
    if !redo && !result.restored.is_empty() {
        lines.push("คืนแค่ไฟล์: คำสั่งที่รอบนั้นรัน และของนอก working tree ยังเหมือนเดิม".into());
    }
    let retry = (!result.skipped.is_empty()).then_some(Retry::Restore { redo });
    (lines.join("\n"), retry)
}

/// What a rewind did, or why it did not.
pub(super) fn rewound_text(n: u32, result: &RewindResult) -> (String, Option<Retry>) {
    match result {
        RewindResult::Variant0 { restored, .. } => {
            let mut text = format!("ย้อน thread ไปก่อนรอบที่ {n} แล้ว");
            if !restored.is_empty() {
                text.push_str(&format!("\nคืนไฟล์: {}", files(restored)));
            }
            (text, None)
        }
        RewindResult::Variant1 { error, force } => (
            format!("ยังไม่ได้ย้อน: {error}"),
            (*force == Some(true)).then_some(Retry::Rewind { n }),
        ),
    }
}

impl RootView {
    /// The person asked to rewind to just before the run `text` started: ask first.
    pub(super) fn ask_rewind(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(info) = self.project().info() else {
            return;
        };
        let runs = &info.thread.runs;
        let Some(at) = runs
            .iter()
            .rposition(|r| !r.prompt.is_empty() && text.starts_with(r.prompt.as_str()))
        else {
            return;
        };
        self.restore = Some(RestoreCard::Confirm {
            thread: info.thread.id.clone(),
            n: at as u32 + 1,
            prompt: runs[at].prompt.clone(),
        });
        cx.notify();
    }

    pub(super) fn restore_turn(&mut self, redo: bool, force: bool, cx: &mut Context<Self>) {
        let Some(thread) = self.project().current.clone() else {
            return;
        };
        self.restore = None;
        self.request(Request::Restore {
            cwd: self.project().cwd.clone(),
            thread,
            redo,
            force,
        });
        cx.notify();
    }

    fn rewind(&mut self, thread: String, n: u32, force: bool, cx: &mut Context<Self>) {
        self.restore = None;
        self.request(Request::Rewind {
            cwd: self.project().cwd.clone(),
            thread,
            n,
            force,
        });
        cx.notify();
    }

    /// cedia's answer to an undo or a redo of `thread`.
    pub(super) fn on_restored(&mut self, thread: String, result: Restored, cx: &mut Context<Self>) {
        // The receipt offers redo while the files are as the undo left them.
        let after = self
            .project()
            .threads
            .iter()
            .find(|t| t.thread.id == thread)
            .and_then(|t| t.thread.last_turn.as_ref())
            .map(|turn| turn.after.clone());
        match (result.action, after) {
            (RestoredAction::Undo, Some(after)) if result.skipped.is_empty() => {
                self.undone.insert(thread.clone(), after);
            }
            (RestoredAction::Redo, _) if result.skipped.is_empty() => {
                self.undone.remove(&thread);
            }
            _ => {}
        }
        let (text, retry) = restored_text(&result);
        self.restore = Some(RestoreCard::Said {
            thread,
            text,
            retry,
        });
        self.show_offers(cx);
        cx.notify();
    }

    /// cedia's answer to a rewind of `thread`: a done one reads the thread again, since the
    /// conversation now goes on from before run `n`.
    pub(super) fn on_rewound(
        &mut self,
        thread: String,
        n: u32,
        result: RewindResult,
        cx: &mut Context<Self>,
    ) {
        let (text, retry) = rewound_text(n, &result);
        if matches!(result, RewindResult::Variant0 { .. }) {
            self.undone.remove(&thread);
            if self.project().current.as_ref() == Some(&thread) {
                self.show_current(cx);
            }
        }
        self.restore = Some(RestoreCard::Said {
            thread,
            text,
            retry,
        });
        cx.notify();
    }

    /// The card for the open thread, if it has one.
    pub(super) fn render_restore(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let card = self.restore.as_ref()?;
        let shown = match card {
            RestoreCard::Confirm { thread, .. } | RestoreCard::Said { thread, .. } => thread,
        };
        if self.project().current.as_ref() != Some(shown) {
            return None;
        }
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
            button("restore-close", false)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.restore = None;
                    cx.notify();
                }))
                .child(word)
        };
        let (title, text, buttons) = match card {
            RestoreCard::Confirm { thread, n, prompt } => {
                let (thread, n) = (thread.clone(), *n);
                let line = prompt.lines().next().unwrap_or_default();
                (
                    "ย้อน thread",
                    format!(
                        "กลับไปก่อน \u{201c}{line}\u{201d}: บทสนทนาจากตรงนั้นหายไป และไฟล์ที่รอบหลังจากนั้นแก้จะถูกคืน ย้อนแล้วทำซ้ำไม่ได้"
                    ),
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            button("restore-go", true)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.rewind(thread.clone(), n, false, cx)
                                }))
                                .child("ย้อน"),
                        )
                        .child(close("ยกเลิก")),
                )
            }
            RestoreCard::Said {
                thread,
                text,
                retry,
            } => {
                let thread = thread.clone();
                let force = retry.map(|retry| {
                    let word = match retry {
                        Retry::Restore { .. } => "ทับไฟล์เหล่านั้นด้วย",
                        Retry::Rewind { .. } => "ย้อนทั้งอย่างนั้น",
                    };
                    button("restore-force", false)
                        .on_click(cx.listener(move |view, _, _, cx| match retry {
                            Retry::Restore { redo } => view.restore_turn(redo, true, cx),
                            Retry::Rewind { n } => view.rewind(thread.clone(), n, true, cx),
                        }))
                        .child(word)
                });
                (
                    "ย้อนไฟล์",
                    text.clone(),
                    div().flex().gap_2().children(force).child(close("ปิด")),
                )
            }
        };
        Some(
            div()
                .debug_selector(|| "restore-card".into())
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
                        .child(title),
                )
                .child(div().text_message().child(line_breaks::wrapped(&text)))
                .child(buttons)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn restored(action: &str, restored: &[&str], skipped: &[&str]) -> Restored {
        serde_json::from_value(serde_json::json!({
            "action": action, "restored": restored, "skipped": skipped,
        }))
        .expect("restored")
    }

    #[test]
    fn an_undo_says_what_moved_and_offers_force_for_what_it_left() {
        let (text, retry) = restored_text(&restored("undo", &["a.txt"], &["b.txt"]));
        assert!(text.contains("คืนไฟล์แล้ว: a.txt"), "{text}");
        assert!(text.contains("ไม่ได้แตะ b.txt"), "{text}");
        assert!(matches!(retry, Some(Retry::Restore { redo: false })));
        let (text, retry) = restored_text(&restored("redo", &[], &[]));
        assert_eq!(text, "ไฟล์เป็นแบบนั้นอยู่แล้ว");
        assert!(retry.is_none());
    }

    #[test]
    fn a_refused_rewind_offers_force_only_when_cedia_says_it_goes_past() {
        let refused: RewindResult =
            serde_json::from_str(r#"{"error":"b.txt changed","force":true}"#).expect("result");
        assert!(matches!(
            rewound_text(2, &refused).1,
            Some(Retry::Rewind { n: 2 })
        ));
        let out_of_range: RewindResult =
            serde_json::from_str(r#"{"error":"has runs 1 to 1"}"#).expect("result");
        assert!(rewound_text(5, &out_of_range).1.is_none());
        let done: RewindResult =
            serde_json::from_str(r#"{"run":"r-2","restored":["a.txt"]}"#).expect("result");
        let (text, retry) = rewound_text(2, &done);
        assert!(
            text.contains("ก่อนรอบที่ 2") && text.contains("a.txt"),
            "{text}"
        );
        assert!(retry.is_none());
    }
}
