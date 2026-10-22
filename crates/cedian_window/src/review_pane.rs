//! The thread's changes since its first snapshot, hunk by hunk: accept, undo, or reject with a
//! reason the thread is told. J and K move between hunks, Y accepts, N rejects.

use cedian_wire::{Decision, Hunk, HunkState, Review};
use editor::Editor;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, actions, div,
    prelude::FluentBuilder as _,
};
use theme::ActiveTheme as _;

actions!(
    review,
    [Next, Previous, Accept, Reject, ConfirmReject, Cancel]
);

pub enum ReviewEvent {
    Decide { hunk: String, decision: Decision },
    Refresh,
}

pub struct ReviewPane {
    thread: Option<String>,
    review: Option<Review>,
    /// Index into every hunk of the review, in file order.
    selected: usize,
    /// The hunk being rejected and the box its reason goes in.
    rejecting: Option<(String, Entity<Editor>)>,
    focus: FocusHandle,
}

impl EventEmitter<ReviewEvent> for ReviewPane {}

impl Focusable for ReviewPane {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ReviewPane {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            thread: None,
            review: None,
            selected: 0,
            rejecting: None,
            focus: cx.focus_handle(),
        }
    }

    pub fn set_thread(&mut self, thread: Option<String>, cx: &mut Context<Self>) {
        if self.thread != thread {
            self.thread = thread;
            self.review = None;
            self.selected = 0;
            self.rejecting = None;
            cx.notify();
        }
    }

    pub fn set_review(&mut self, review: Review, cx: &mut Context<Self>) {
        let count = hunks(&review).count();
        self.selected = self.selected.min(count.saturating_sub(1));
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

    // The reason box takes focus on the next frame; keys typed before it do not move the list.
    fn next(&mut self, _: &Next, _: &mut Window, cx: &mut Context<Self>) {
        if self.rejecting.is_some() {
            return;
        }
        if self.hunk(self.selected + 1).is_some() {
            self.selected += 1;
            cx.notify();
        }
    }

    fn previous(&mut self, _: &Previous, _: &mut Window, cx: &mut Context<Self>) {
        if self.rejecting.is_some() {
            return;
        }
        self.selected = self.selected.saturating_sub(1);
        cx.notify();
    }

    fn accept(&mut self, _: &Accept, _: &mut Window, cx: &mut Context<Self>) {
        if self.rejecting.is_some() {
            return;
        }
        self.decide_selected(true, cx);
    }

    fn reject(&mut self, _: &Reject, window: &mut Window, cx: &mut Context<Self>) {
        if self.rejecting.is_some() {
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

    fn start_reject(&mut self, hunk: &Hunk, window: &mut Window, cx: &mut Context<Self>) {
        if !rejectable(hunk) {
            return;
        }
        let editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Why? The thread sees this", window, cx);
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
        let (badge, badge_color) = match hunk.state {
            HunkState::Pending => ("pending", colors.text_muted),
            HunkState::Accepted => ("accepted", status.success),
            HunkState::Rejected => ("rejected", status.error),
            HunkState::Stale => ("STALE", status.warning),
        };
        let mut actions = div().flex().gap_3();
        let action = |id: &str, label: &'static str| {
            div()
                .id(SharedString::from(format!("{id}-{}", hunk.id)))
                .cursor_pointer()
                .text_color(colors.text_accent)
                .child(label)
        };
        match hunk.state {
            HunkState::Pending => {
                actions = actions
                    .child(action("accept", "Accept (Y)").on_click(cx.listener(
                        move |pane, _, _, cx| {
                            pane.selected = index;
                            pane.decide_selected(true, cx);
                        },
                    )))
                    .child(action("reject", "Reject (N)").on_click(cx.listener(
                        move |pane, _, window, cx| {
                            pane.selected = index;
                            if let Some(hunk) = pane.hunk(index).cloned() {
                                pane.start_reject(&hunk, window, cx);
                            }
                        },
                    )));
            }
            HunkState::Accepted => {
                actions = actions.child(action("undo", "Undo").on_click(cx.listener(
                    move |pane, _, _, cx| {
                        pane.selected = index;
                        pane.decide_selected(false, cx);
                    },
                )));
            }
            HunkState::Stale => {
                actions = actions.child(
                    div()
                        .text_color(colors.text_muted)
                        .child("edited by hand after the agent, so it cannot be rejected"),
                );
            }
            HunkState::Rejected => {}
        }
        let collapsed = matches!(hunk.state, HunkState::Accepted | HunkState::Rejected);
        let mut body = div().font_family("Lilex").text_xs().flex().flex_col();
        if !collapsed {
            for line in &hunk.lines {
                let color = match line.chars().next() {
                    Some('+') => status.success,
                    Some('-') => status.error,
                    _ => colors.text_muted,
                };
                body = body.child(div().px_2().text_color(color).child(line.clone()));
            }
        }
        let rejecting = self
            .rejecting
            .as_ref()
            .filter(|(id, _)| id == &hunk.id)
            .map(|(_, editor)| editor.clone());
        div()
            .id(SharedString::from(format!("hunk-{}", hunk.id)))
            .flex()
            .flex_col()
            .mx_2()
            .my_1()
            .border_1()
            .border_color(if selected {
                colors.border_focused
            } else {
                colors.border
            })
            .when(hunk.state == HunkState::Stale, |el| {
                el.bg(status.warning.opacity(0.08))
            })
            .on_click(cx.listener(move |pane, _, window, cx| {
                pane.selected = index;
                window.focus(&pane.focus, cx);
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_color(colors.text_muted)
                                    .child(hunk.header.clone()),
                            )
                            .child(div().text_color(badge_color).child(badge)),
                    )
                    .child(actions),
            )
            .child(body)
            .when_some(rejecting, |el, editor| {
                el.child(
                    div()
                        .key_context("RejectReason")
                        .on_action(cx.listener(Self::confirm_reject))
                        .on_action(cx.listener(Self::cancel))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_2()
                        .border_t_1()
                        .border_color(colors.border)
                        .child(
                            div()
                                .p_1()
                                .border_1()
                                .border_color(colors.border)
                                .bg(colors.editor_background)
                                .child(editor),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .text_xs()
                                .child(
                                    div()
                                        .id("confirm-reject")
                                        .cursor_pointer()
                                        .text_color(status.error)
                                        .on_click(cx.listener(|pane, _, window, cx| {
                                            pane.confirm_reject(&ConfirmReject, window, cx)
                                        }))
                                        .child("Reject and tell the thread (Enter)"),
                                )
                                .child(
                                    div()
                                        .id("cancel-reject")
                                        .cursor_pointer()
                                        .text_color(colors.text_muted)
                                        .on_click(cx.listener(|pane, _, window, cx| {
                                            pane.cancel(&Cancel, window, cx)
                                        }))
                                        .child("Cancel (Esc)"),
                                ),
                        ),
                )
            })
            .into_any_element()
    }
}

fn hunks(review: &Review) -> impl Iterator<Item = &Hunk> {
    review.files.iter().flat_map(|file| &file.hunks)
}

fn rejectable(hunk: &Hunk) -> bool {
    matches!(hunk.state, HunkState::Pending | HunkState::Accepted)
}

impl Render for ReviewPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let (muted, accent, panel, text, border) = (
            colors.text_muted,
            colors.text_accent,
            colors.panel_background,
            colors.text,
            colors.border,
        );
        let mut list = div()
            .id("review")
            .key_context("ReviewList")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::accept))
            .on_action(cx.listener(Self::reject))
            .flex()
            .flex_col()
            .flex_grow(1.)
            .min_h_0()
            .overflow_y_scroll();
        let summary: SharedString = match (&self.thread, &self.review) {
            (None, _) => "No thread open".into(),
            (Some(_), None) => "Reading the thread's changes".into(),
            (Some(_), Some(review)) if review.base.is_none() => {
                "The thread has not changed any file".into()
            }
            (Some(_), Some(review)) => {
                let all: Vec<&Hunk> = hunks(review).collect();
                let decided = all
                    .iter()
                    .filter(|h| matches!(h.state, HunkState::Accepted | HunkState::Rejected))
                    .count();
                format!(
                    "Against the thread's first snapshot, not git HEAD · {decided}/{} hunks",
                    all.len()
                )
                .into()
            }
        };
        if let Some(review) = self.review.clone() {
            let mut index = 0;
            for file in &review.files {
                list = list.child(
                    div()
                        .flex()
                        .gap_2()
                        .px_2()
                        .pt_2()
                        .text_sm()
                        .child(file.path.clone())
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(format!("+{} −{}", file.added, file.removed)),
                        )
                        .when(file.binary, |el| {
                            el.child(div().text_xs().text_color(muted).child("binary"))
                        }),
                );
                for hunk in &file.hunks {
                    list = list.child(self.render_hunk(index, hunk, cx));
                    index += 1;
                }
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(panel)
            .text_color(text)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(border)
                    .text_xs()
                    .child(div().text_color(muted).child(summary))
                    .child(
                        div()
                            .id("refresh-review")
                            .cursor_pointer()
                            .text_color(accent)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(ReviewEvent::Refresh)))
                            .child("Refresh"),
                    ),
            )
            .child(list)
    }
}
