//! The thread's changes since its first snapshot, hunk by hunk (Interface, Diff review): accept,
//! undo, or reject with a reason the thread is told. J and K move between hunks, Y accepts, N
//! rejects. In the right column the files stack; widened into the main area it has the files
//! (280 px), the open file's hunks, and the checks that cover that file (280 px).

use cedian_agent::FileChecks;
use cedian_wire::{Decision, FileReview, Hunk, HunkState, Review};
use editor::Editor;
use gpui::{
    AnyElement, App, AppContext as _, Context, Div, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, actions, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::type_scale::TypeScale as _;
use crate::{agent_pane::AgentPane, tokens};

actions!(
    review,
    [Next, Previous, Accept, Reject, ConfirmReject, Cancel]
);

/// The widths of the files and evidence columns when the review fills the main area.
const FILES_WIDTH: f32 = 248.;
const SIDE_WIDTH: f32 = 280.;

pub enum ReviewEvent {
    Decide {
        hunk: String,
        decision: Decision,
    },
    Refresh,
    /// Ask the thread to run a check again, the file having changed after it.
    Rerun {
        command: String,
        path: String,
    },
}

pub struct ReviewPane {
    thread: Option<String>,
    review: Option<Review>,
    /// The block's header draws the toolbar (the block holds no other tab).
    pub bar_in_header: bool,
    /// Index into every hunk of the review, in file order.
    selected: usize,
    /// The open file, which follows the selected hunk; a file with no hunks can be opened too.
    file: usize,
    /// The hunk being rejected and the box its reason goes in.
    rejecting: Option<(String, Entity<Editor>)>,
    wide: bool,
    /// Under 380 px a hunk's buttons show only their keys.
    compact: bool,
    /// The thread's transcript, for the checks that cover a file.
    agent: Option<WeakEntity<AgentPane>>,
    focus: FocusHandle,
}

impl EventEmitter<ReviewEvent> for ReviewPane {}

impl Focusable for ReviewPane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ReviewPane {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            thread: None,
            review: None,
            bar_in_header: false,
            selected: 0,
            file: 0,
            rejecting: None,
            wide: false,
            compact: false,
            agent: None,
            focus: cx.focus_handle(),
        }
    }

    /// Reads the open file's checks from the agent pane, and draws again when it changes.
    pub fn set_agent(&mut self, agent: &Entity<AgentPane>, cx: &mut Context<Self>) {
        self.agent = Some(agent.downgrade());
        cx.observe(agent, |pane, _, cx| {
            if pane.wide {
                cx.notify();
            }
        })
        .detach();
    }

    /// `wide`: files beside the hunks; `compact`: the hunk buttons as their keys.
    pub fn set_width(&mut self, wide: bool, compact: bool, cx: &mut Context<Self>) {
        if (self.wide, self.compact) != (wide, compact) {
            self.wide = wide;
            self.compact = compact;
            cx.notify();
        }
    }

    pub fn set_thread(&mut self, thread: Option<String>, cx: &mut Context<Self>) {
        if self.thread != thread {
            self.thread = thread;
            self.review = None;
            self.selected = 0;
            self.file = 0;
            self.rejecting = None;
            cx.notify();
        }
    }

    pub fn set_review(&mut self, review: Review, cx: &mut Context<Self>) {
        let count = hunks(&review).count();
        self.selected = self.selected.min(count.saturating_sub(1));
        self.file = self.file.min(review.files.len().saturating_sub(1));
        if let Some((id, _)) = &self.rejecting
            && !hunks(&review).any(|h| &h.id == id && rejectable(h))
        {
            self.rejecting = None;
        }
        self.review = Some(review);
        cx.notify();
    }

    fn hunk(&self, index: usize) -> Option<&Hunk> {
        hunks(self.review.as_ref()?).nth(index)
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        if let Some(review) = &self.review {
            self.file = file_of(review, index).unwrap_or(self.file);
        }
        cx.notify();
    }

    fn open_file(&mut self, file: usize, cx: &mut Context<Self>) {
        self.file = file;
        if let Some(first) = self.review.as_ref().and_then(|r| first_hunk(r, file)) {
            self.selected = first;
        }
        cx.notify();
    }

    // The reason box sits inside the list, so while it is open J, K, Y and N are text for it.
    fn next(&mut self, _: &Next, _: &mut Window, cx: &mut Context<Self>) {
        if self.rejecting.is_some() {
            cx.propagate();
            return;
        }
        if self.hunk(self.selected + 1).is_some() {
            self.select(self.selected + 1, cx);
        }
    }

    fn previous(&mut self, _: &Previous, _: &mut Window, cx: &mut Context<Self>) {
        if self.rejecting.is_some() {
            cx.propagate();
            return;
        }
        self.select(self.selected.saturating_sub(1), cx);
    }

    fn accept(&mut self, _: &Accept, _: &mut Window, cx: &mut Context<Self>) {
        if self.rejecting.is_some() {
            cx.propagate();
            return;
        }
        self.decide_selected(true, cx);
    }

    fn reject(&mut self, _: &Reject, window: &mut Window, cx: &mut Context<Self>) {
        if self.rejecting.is_some() {
            cx.propagate();
            return;
        }
        if let Some(hunk) = self.hunk(self.selected).cloned() {
            self.start_reject(&hunk, window, cx);
        }
    }

    fn decide_selected(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(hunk) = self.hunk(self.selected).cloned() else {
            return;
        };
        let decision = match (accept, hunk.state) {
            (true, HunkState::Pending) => Decision::Accept,
            (false, HunkState::Accepted) => Decision::Undo,
            _ => return,
        };
        cx.emit(ReviewEvent::Decide {
            hunk: hunk.id,
            decision,
        });
    }

    /// รับที่เหลือทั้งหมด: accepts every hunk still waiting.
    fn accept_rest(&mut self, cx: &mut Context<Self>) {
        let Some(review) = &self.review else {
            return;
        };
        let pending: Vec<String> = hunks(review)
            .filter(|h| h.state == HunkState::Pending)
            .map(|h| h.id.clone())
            .collect();
        for hunk in pending {
            cx.emit(ReviewEvent::Decide {
                hunk,
                decision: Decision::Accept,
            });
        }
    }

    fn start_reject(&mut self, hunk: &Hunk, window: &mut Window, cx: &mut Context<Self>) {
        if !rejectable(hunk) {
            return;
        }
        let editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("ทำไม? thread จะเห็นข้อความนี้", window, cx);
            editor
        });
        window.focus(&editor.focus_handle(cx), cx);
        self.rejecting = Some((hunk.id.clone(), editor));
        cx.notify();
    }

    fn confirm_reject(&mut self, _: &ConfirmReject, window: &mut Window, cx: &mut Context<Self>) {
        let Some((hunk, editor)) = self.rejecting.take() else {
            return;
        };
        let reason = editor.read(cx).text(cx);
        cx.emit(ReviewEvent::Decide {
            hunk,
            decision: Decision::Reject { reason },
        });
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        self.rejecting = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn render_hunk(&self, index: usize, hunk: &Hunk, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let status = cx.theme().status();
        let selected = index == self.selected;
        let compact = self.compact;
        let stale = hunk.state == HunkState::Stale;
        let badge = match hunk.state {
            HunkState::Pending => None,
            HunkState::Accepted => Some(
                badge("รับแล้ว")
                    .bg(colors.element_selected)
                    .text_color(colors.text),
            ),
            HunkState::Rejected => Some(
                badge("ปฏิเสธแล้ว")
                    .bg(status.error)
                    .text_color(colors.background),
            ),
            HunkState::Stale => Some(
                badge("STALE")
                    .border_1()
                    .border_color(status.warning)
                    .text_color(status.warning),
            ),
        };
        // The buttons never shrink or wrap; the hunk's header gives way with an ellipsis.
        let mut actions = div().flex_none().flex().gap_1();
        match hunk.state {
            HunkState::Pending => {
                actions = actions
                    .child(
                        button(
                            cx,
                            ("accept", index),
                            if compact { "Y" } else { "รับ" },
                            (!compact).then_some("Y"),
                        )
                        .border_color(colors.border_selected)
                        .on_click(cx.listener(move |pane, _, _, cx| {
                            pane.select(index, cx);
                            pane.decide_selected(true, cx);
                        })),
                    )
                    .child(
                        button(
                            cx,
                            ("reject", index),
                            if compact { "N" } else { "ปฏิเสธ" },
                            (!compact).then_some("N"),
                        )
                        .on_click(cx.listener(
                            move |pane, _, window, cx| {
                                pane.select(index, cx);
                                if let Some(hunk) = pane.hunk(index).cloned() {
                                    pane.start_reject(&hunk, window, cx);
                                }
                            },
                        )),
                    );
            }
            HunkState::Accepted => {
                actions = actions.child(button(cx, ("undo", index), "ย้อนกลับ", None).on_click(
                    cx.listener(move |pane, _, _, cx| {
                        pane.select(index, cx);
                        pane.decide_selected(false, cx);
                    }),
                ));
            }
            HunkState::Stale | HunkState::Rejected => {}
        }
        let folded = matches!(hunk.state, HunkState::Accepted | HunkState::Rejected);
        // The lines as wide as the longest, so a tint runs the whole line when it scrolls
        // sideways.
        let mut lines = div().flex_none().min_w_full().flex().flex_col();
        if !folded {
            for line in &hunk.lines {
                let row = div().px_2().whitespace_nowrap();
                lines = lines.child(match line.chars().next() {
                    Some('+') => row
                        .bg(tokens::Tokens::of(cx).diff_add.opacity(0.1))
                        .text_color(colors.text)
                        .child(format!("+ {}", &line[1..])),
                    Some('-') => row
                        .bg(status.error.opacity(0.1))
                        .text_color(colors.text)
                        .child(format!("− {}", &line[1..])),
                    _ => row
                        .text_color(colors.text_muted)
                        .child(format!("  {}", line.get(1..).unwrap_or_default())),
                });
            }
        }
        let body = div()
            .id(SharedString::from(format!("hunk-lines-{}", hunk.id)))
            .border_t_1()
            .border_color(colors.border)
            .py_1()
            .font(tokens::code_font())
            .text_code()
            .flex()
            .overflow_x_scroll()
            .child(lines);
        let rejecting = self
            .rejecting
            .as_ref()
            .filter(|(id, _)| id == &hunk.id)
            .map(|(_, editor)| editor.clone());
        div()
            .id(SharedString::from(format!("hunk-{}", hunk.id)))
            .flex()
            .flex_col()
            .rounded(px(12.))
            .overflow_hidden()
            .border_1()
            .border_color(if selected {
                colors.text_accent
            } else if stale {
                status.warning.opacity(0.4)
            } else {
                colors.border
            })
            .bg(if stale {
                status.warning.opacity(0.08)
            } else {
                colors.editor_background
            })
            .on_click(cx.listener(move |pane, _, window, cx| {
                pane.select(index, cx);
                window.focus(&pane.focus, cx);
            }))
            .child(
                div()
                    .min_h(px(30.))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_meta()
                    .child(
                        div()
                            .flex_grow(1.)
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .font(tokens::code_font())
                            .text_color(colors.text_muted)
                            .child(hunk.header.clone()),
                    )
                    .children(badge)
                    .child(actions),
            )
            .when(stale, |el| {
                el.child(
                    div()
                        .px_2()
                        .pb_2()
                        .text_meta()
                        .text_color(status.warning)
                        .child(
                            "ไฟล์นี้ถูกแก้หลังจาก agent เขียน hunk นี้ จึงปฏิเสธไม่ได้: \
                             การเอาบรรทัดเก่ากลับจะทับสิ่งที่แก้ไว้",
                        ),
                )
            })
            .when(!folded, |el| el.child(body))
            .when_some(rejecting, |el, editor| {
                el.child(
                    div()
                        .key_context("RejectReason")
                        .on_action(cx.listener(Self::confirm_reject))
                        .on_action(cx.listener(Self::cancel))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_2()
                        .border_t_1()
                        .border_color(colors.border)
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .rounded(px(7.))
                                .border_1()
                                .border_color(colors.border)
                                .bg(colors.background)
                                .child(editor),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .child(
                                    button(
                                        cx,
                                        ("confirm-reject", index),
                                        "ปฏิเสธและบอก thread",
                                        Some("Enter"),
                                    )
                                    .border_color(status.error)
                                    .on_click(cx.listener(
                                        |pane, _, window, cx| {
                                            pane.confirm_reject(&ConfirmReject, window, cx)
                                        },
                                    )),
                                )
                                .child(
                                    button(cx, ("cancel-reject", index), "ยกเลิก", Some("Esc"))
                                        .on_click(cx.listener(|pane, _, window, cx| {
                                            pane.cancel(&Cancel, window, cx)
                                        })),
                                ),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_file_row(
        &self,
        n: usize,
        file: &FileReview,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = cx.theme().colors();
        let open = self.wide && n == self.file;
        div()
            .id(("file", n))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .h(px(30.))
            .rounded(px(7.))
            .cursor_pointer()
            .text_meta()
            .when(open, |row| row.bg(colors.element_selected))
            .hover(|row| row.bg(colors.element_hover))
            .on_click(cx.listener(move |pane, _, _, cx| pane.open_file(n, cx)))
            .child(
                div()
                    .flex_grow(1.)
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .font(tokens::code_font())
                    .child(file.path.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .font(tokens::numeric_font())
                    .text_color(colors.text_muted)
                    .child(if file.binary {
                        "binary".to_owned()
                    } else {
                        format!("+{} −{}", file.added, file.removed)
                    }),
            )
            .child(file_tag(file, cx))
    }

    fn render_evidence(&self, cx: &mut Context<Self>) -> Div {
        let colors = cx.theme().colors();
        let status = cx.theme().status();
        let path = self
            .review
            .as_ref()
            .and_then(|r| r.files.get(self.file))
            .map(|f| f.path.clone());
        let checks: FileChecks = match (&path, self.agent.as_ref().and_then(|a| a.upgrade())) {
            (Some(path), Some(agent)) => agent.read(cx).file_checks(path),
            _ => FileChecks::default(),
        };
        let label = |text: &'static str| {
            div()
                .text_label()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.text_muted)
                .child(text)
        };
        let mut column = div()
            .w(px(SIDE_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_l_1()
            .border_color(colors.border)
            .text_meta()
            .child(label("หลักฐานของไฟล์นี้"));
        if checks.covering.is_empty() && checks.outdated.is_empty() {
            column = column.child(
                div()
                    .text_color(colors.text_muted)
                    .child("ยังไม่มี check ที่ครอบไฟล์นี้"),
            );
        }
        for command in &checks.covering {
            column = column.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_none()
                            .text_color(tokens::Tokens::of(cx).diff_add)
                            .child("✓"),
                    )
                    .child(div().font(tokens::code_font()).child(command.clone())),
            );
        }
        if !checks.outdated.is_empty() {
            column = column.child(label("ถูกแก้ทับหลัง check"));
        }
        for (n, command) in checks.outdated.iter().enumerate() {
            let rerun = (command.clone(), path.clone().unwrap_or_default());
            column = column.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().flex_none().text_color(status.warning).child("!"))
                            .child(div().font(tokens::code_font()).child(command.clone())),
                    )
                    .child(
                        button(cx, ("rerun", n), "ให้ thread รัน check ใหม่", None).on_click(
                            cx.listener(move |_, _, _, cx| {
                                cx.emit(ReviewEvent::Rerun {
                                    command: rerun.0.clone(),
                                    path: rerun.1.clone(),
                                })
                            }),
                        ),
                    ),
            );
        }
        column
    }
}

fn hunks(review: &Review) -> impl Iterator<Item = &Hunk> {
    review.files.iter().flat_map(|file| &file.hunks)
}

/// The file the `index`th hunk is in.
fn file_of(review: &Review, index: usize) -> Option<usize> {
    let mut seen = 0;
    for (n, file) in review.files.iter().enumerate() {
        seen += file.hunks.len();
        if index < seen {
            return Some(n);
        }
    }
    None
}

fn first_hunk(review: &Review, file: usize) -> Option<usize> {
    let before: usize = review.files.iter().take(file).map(|f| f.hunks.len()).sum();
    review
        .files
        .get(file)
        .filter(|f| !f.hunks.is_empty())
        .map(|_| before)
}

fn rejectable(hunk: &Hunk) -> bool {
    matches!(hunk.state, HunkState::Pending | HunkState::Accepted)
}

/// รอรีวิว, รับครบ, or `stale n`.
fn file_tag(file: &FileReview, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let stale = file
        .hunks
        .iter()
        .filter(|h| h.state == HunkState::Stale)
        .count();
    let pending = file.hunks.iter().any(|h| h.state == HunkState::Pending);
    let all_accepted = file.hunks.iter().all(|h| h.state == HunkState::Accepted);
    if stale > 0 {
        badge(&format!("stale {stale}"))
            .border_1()
            .border_color(status.warning)
            .text_color(status.warning)
    } else if pending {
        badge("รอรีวิว")
            .border_1()
            .border_color(colors.border)
            .text_color(colors.text_muted)
    } else if all_accepted {
        badge("รับครบ").text_color(tokens::Tokens::of(cx).diff_add)
    } else {
        badge("ตัดสินครบ").text_color(colors.text_muted)
    }
}

fn badge(text: &str) -> Div {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(4.))
        .text_label()
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.to_owned())
}

fn button(
    cx: &App,
    id: (&'static str, usize),
    label: &'static str,
    key: Option<&'static str>,
) -> Stateful<Div> {
    let colors = cx.theme().colors();
    div()
        .id(id)
        .flex_none()
        .h(px(26.))
        .px_2()
        .flex()
        .items_center()
        .gap_1p5()
        .rounded(px(7.))
        .border_1()
        .border_color(colors.border)
        .bg(colors.element_background)
        .hover(|b| b.bg(colors.element_hover))
        .cursor_pointer()
        .text_meta()
        .text_color(colors.text)
        .child(label)
        .children(key.map(|key| div().text_label().text_color(colors.text_muted).child(key)))
}

impl ReviewPane {
    /// What is left to decide, รับที่เหลือทั้งหมด and refresh: in the block's header when the
    /// block holds only the review, else over the hunks.
    pub fn toolbar(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let muted = cx.theme().colors().text_muted;
        let review = self.review.clone();
        let (decided, total, pending, added, removed) =
            review.as_ref().map_or((0, 0, 0, 0, 0), |r| {
                let all: Vec<&Hunk> = hunks(r).collect();
                (
                    all.iter()
                        .filter(|h| matches!(h.state, HunkState::Accepted | HunkState::Rejected))
                        .count(),
                    all.len(),
                    all.iter().filter(|h| h.state == HunkState::Pending).count(),
                    r.files.iter().map(|f| f.added).sum::<usize>(),
                    r.files.iter().map(|f| f.removed).sum::<usize>(),
                )
            });
        div()
            .id("review-toolbar")
            .debug_selector(|| "review-toolbar".into())
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap_2()
            // The block's header names it; this line counts what is left.
            .when(total > 0, |row| {
                row.child(
                    div()
                        .font(tokens::numeric_font())
                        .text_meta()
                        .text_color(muted)
                        .child(format!("+{added} −{removed} · {decided}/{total} hunk")),
                )
            })
            .child(div().flex_grow(1.))
            .when(pending > 0, |row| {
                row.child(
                    button(cx, ("accept-rest", 0), "รับที่เหลือทั้งหมด", None)
                        .on_click(cx.listener(|pane, _, _, cx| pane.accept_rest(cx))),
                )
            })
            .child(
                icon_button(cx, "refresh-review", "icons/rotate_cw.svg")
                    .tooltip(|_, cx| {
                        crate::tooltip::text(
                            "อ่านใหม่ · เทียบกับ snapshot แรกของ thread ไม่ใช่ git HEAD".into(),
                            cx,
                        )
                    })
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ReviewEvent::Refresh))),
            )
    }
}

impl Render for ReviewPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let (muted, panel, text, border) = (
            colors.text_muted,
            colors.panel_background,
            colors.text,
            colors.border,
        );
        let review = self.review.clone();
        let empty: Option<&'static str> = match (&self.thread, &review) {
            (None, _) => Some("ยังไม่ได้เปิด thread"),
            (Some(_), None) => Some("กำลังอ่านสิ่งที่ thread เปลี่ยน"),
            (Some(_), Some(review)) if review.base.is_none() || review.files.is_empty() => {
                Some("thread นี้ยังไม่ได้แก้ไฟล์ไหน")
            }
            _ => None,
        };
        let wide = self.wide;
        let header = (!self.bar_in_header).then(|| {
            div()
                .flex_none()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(border)
                .flex()
                .child(self.toolbar(cx))
        });
        let mut hunk_list = div()
            .id("review")
            .key_context("ReviewList")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::accept))
            .on_action(cx.listener(Self::reject))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .flex_grow(1.)
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll();
        if let Some(message) = empty {
            hunk_list = hunk_list.child(div().text_ui().text_color(muted).child(message));
        } else if let Some(review) = &review {
            let mut index = 0;
            for (n, file) in review.files.iter().enumerate() {
                let shown = !wide || n == self.file;
                if shown && !wide {
                    hunk_list = hunk_list.child(self.render_file_row(n, file, cx));
                }
                for hunk in &file.hunks {
                    if shown {
                        hunk_list = hunk_list.child(self.render_hunk(index, hunk, cx));
                    }
                    index += 1;
                }
            }
        }
        let body = if wide && empty.is_none() {
            let files = review.as_ref().map(|review| {
                div()
                    .id("review-files")
                    .w(px(FILES_WIDTH))
                    .flex_none()
                    .h_full()
                    .flex()
                    .flex_col()
                    .gap_px()
                    .p_2()
                    .border_r_1()
                    .border_color(border)
                    .overflow_y_scroll()
                    .children(
                        review
                            .files
                            .iter()
                            .enumerate()
                            .map(|(n, file)| self.render_file_row(n, file, cx)),
                    )
            });
            div()
                .flex()
                .flex_grow(1.)
                .min_h_0()
                .children(files)
                .child(hunk_list)
                .child(self.render_evidence(cx))
        } else {
            div().flex().flex_grow(1.).min_h_0().child(hunk_list)
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(panel)
            .text_color(text)
            .children(header)
            .child(body)
    }
}

fn icon_button(cx: &App, id: &'static str, icon: &'static str) -> Stateful<Div> {
    let colors = cx.theme().colors();
    div()
        .id(id)
        .flex_none()
        .size(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .cursor_pointer()
        .hover(|b| b.bg(colors.element_hover))
        .child(svg().path(icon).size(px(14.)).text_color(colors.text_muted))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, hunks: usize) -> FileReview {
        FileReview {
            path: path.to_owned(),
            binary: false,
            added: 0,
            removed: 0,
            hunks: (0..hunks)
                .map(|n| {
                    serde_json::from_value(serde_json::json!({
                        "id": format!("{path}-{n}"),
                        "path": path,
                        "header": "@@ -1 +1 @@",
                        "new_start": 1,
                        "lines": [],
                        "state": "pending",
                    }))
                    .expect("hunk")
                })
                .collect(),
        }
    }

    #[test]
    fn hunks_map_to_their_files() {
        let review = Review {
            base: Some("t".into()),
            files: vec![file("a", 2), file("bin", 0), file("c", 1)],
        };
        assert_eq!(file_of(&review, 0), Some(0));
        assert_eq!(file_of(&review, 1), Some(0));
        assert_eq!(file_of(&review, 2), Some(2));
        assert_eq!(file_of(&review, 3), None);
        assert_eq!(first_hunk(&review, 2), Some(2));
        assert_eq!(first_hunk(&review, 1), None);
    }
}
