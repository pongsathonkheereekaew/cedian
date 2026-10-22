//! The agent pane: a thread's blocks, drawn from cedia's stream as it arrives.

use cedian_agent::{Block, ToolCall, ToolState, Transcript};
use cedian_protocol::{EffectStatus, Line};
use gpui::{
    AnyElement, App, Context, Hsla, IntoElement, ListAlignment, ListState, ParentElement as _,
    Render, SharedString, Styled as _, Window, div, list, prelude::FluentBuilder as _, px,
};
use theme::ActiveTheme as _;

use crate::{line_breaks::wrapped, tokens};

/// Lines of tool output shown under a call before it is cut.
const OUTPUT_LINES: usize = 6;

pub struct AgentPane {
    title: SharedString,
    transcript: Transcript,
    list: ListState,
}

impl AgentPane {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            transcript: Transcript::default(),
            list: ListState::new(0, ListAlignment::Bottom, px(512.)),
        }
    }

    /// Empties the pane for another thread.
    pub fn reset(&mut self, title: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.title = title.into();
        self.transcript = Transcript::default();
        self.list.reset(0);
        cx.notify();
    }

    pub fn set_title(&mut self, title: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.title = title.into();
        cx.notify();
    }

    pub fn apply(&mut self, line: &Line, cx: &mut Context<Self>) {
        self.transcript.apply(line);
        let Some(from) = self.transcript.take_changed_from() else {
            return;
        };
        let old = self.list.item_count();
        let from = from.min(old);
        self.list.splice(
            from..old,
            self.transcript.blocks().len().saturating_sub(from),
        );
        cx.notify();
    }

    fn render_block(&self, index: usize, cx: &App) -> AnyElement {
        let colors = cx.theme().colors();
        let status = cx.theme().status();
        let tokens = tokens::Tokens::of(cx);
        let Some(block) = self.transcript.blocks().get(index) else {
            return div().into_any_element();
        };
        let body = match block {
            Block::Session { model, role } => div()
                .text_xs()
                .text_color(colors.text_muted)
                .child(format!("{role} session on {model}")),
            Block::User { text } => div()
                .p_2()
                .rounded_md()
                .bg(tokens.user_bubble)
                .child(wrapped(text)),
            Block::Text { text, .. } => div().font(tokens::answer_font()).child(wrapped(text)),
            Block::Thinking { text, .. } => div()
                .italic()
                .text_color(colors.text_muted)
                .child(wrapped(text)),
            Block::Tool(call) => {
                let color = tool_color(call, status.success, status.error, colors.text_muted);
                tool_block(
                    call,
                    color,
                    colors.text_muted,
                    colors.border_variant,
                    tokens.code,
                )
            }
            Block::Result {
                subtype,
                text,
                cost_usd,
                turns,
                evidence,
            } => {
                let verified = block.verified();
                let mut card = div()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(if verified {
                        status.success
                    } else {
                        colors.border
                    })
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(format!("{subtype}"))
                            .when(verified, |row| {
                                row.child(div().text_color(status.success).child("verified"))
                            })
                            .child(
                                div()
                                    .text_color(colors.text_muted)
                                    .child(format!("{turns} turns, ${cost_usd:.4}")),
                            ),
                    )
                    .child(wrapped(text));
                if let Some(evidence) = evidence {
                    for path in &evidence.changed {
                        card = card.child(
                            div()
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child(format!("changed {path}")),
                        );
                    }
                    for check in &evidence.checks {
                        let passed = check.exit_code == 0.;
                        card = card.child(
                            div()
                                .text_xs()
                                .text_color(if passed { status.success } else { status.error })
                                .child(format!(
                                    "check `{}` exit {}{}",
                                    check.command,
                                    check.exit_code,
                                    self.check_origin(check.thread.as_deref())
                                )),
                        );
                    }
                    for path in evidence.unverified.iter().chain(&evidence.flags) {
                        card = card.child(
                            div()
                                .text_xs()
                                .text_color(status.warning)
                                .child(format!("unverified {path}")),
                        );
                    }
                }
                card
            }
        };
        div().px_3().py_1().child(body).into_any_element()
    }
}

impl AgentPane {
    /// Names the sub-thread a check ran in; a check of this thread needs no label.
    fn check_origin(&self, thread: Option<&str>) -> String {
        match thread {
            Some(thread) if Some(thread) != self.transcript.thread() => {
                format!(" in sub-thread {thread}")
            }
            _ => String::new(),
        }
    }
}

fn tool_color(call: &ToolCall, ok: Hsla, bad: Hsla, idle: Hsla) -> Hsla {
    match call.state {
        ToolState::Done(EffectStatus::Completed) => ok,
        ToolState::Done(_) => bad,
        ToolState::Requested | ToolState::Running => idle,
    }
}

fn tool_block(call: &ToolCall, color: Hsla, muted: Hsla, border: Hsla, code: Hsla) -> gpui::Div {
    let mut block = div()
        .pl_2()
        .border_l_2()
        .border_color(border)
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .gap_2()
                .child(div().text_color(color).child(call.name.clone()))
                .child(div().text_color(muted).child(input_summary(call))),
        );
    if let Some(permission) = call.permission.as_ref().filter(|p| !p.allowed) {
        let reason = permission.reason.clone().unwrap_or_default();
        block = block.child(
            div()
                .text_xs()
                .text_color(color)
                .child(format!("denied by {}: {reason}", permission.rule)),
        );
    }
    if let Some(output) = &call.output {
        let mut lines = output.lines();
        let shown: Vec<&str> = lines.by_ref().take(OUTPUT_LINES).collect();
        let more = lines.count();
        block = block.child(
            div()
                .text_xs()
                .font(tokens::code_font())
                .bg(code)
                .px_1()
                .rounded_sm()
                .text_color(muted)
                .children(shown.into_iter().map(|line| div().child(line.to_owned())))
                .when(more > 0, |d| d.child(format!("… {more} more lines"))),
        );
    }
    block
}

fn input_summary(call: &ToolCall) -> String {
    for key in ["command", "path", "pattern", "url", "task"] {
        if let Some(value) = call.input.get(key).and_then(|v| v.as_str()) {
            return value.lines().next().unwrap_or_default().to_owned();
        }
    }
    serde_json::Value::Object(call.input.clone()).to_string()
}

impl Render for AgentPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let pane = cx.entity().downgrade();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.panel_background)
            .text_color(colors.text)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(self.title.clone()),
            )
            .child(
                list(self.list.clone(), move |index, _window, cx| {
                    match pane.upgrade() {
                        Some(pane) => pane.read(cx).render_block(index, cx),
                        None => div().into_any_element(),
                    }
                })
                .flex_grow(1.)
                .py_2(),
            )
    }
}
