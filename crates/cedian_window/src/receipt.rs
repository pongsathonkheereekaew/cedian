//! A run's receipt (Interface, Agent pane): a card with an `accent` border; a header with
//! `verified` or a dashed `unverified n`, the verdict, and turns, time and cost; the checks
//! with the thread that ran each, and the files changed; then the next step and 👍/👎.

use cedian_protocol::{Evidence, Rating, ResultSubtype};
use gpui::{
    App, Div, FontWeight, InteractiveElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::type_scale::TypeScale as _;
use crate::{
    agent_pane::{AgentEvent, Emit},
    line_breaks::wrapped,
    tokens, tool_card,
};

pub struct Receipt<'a> {
    pub subtype: ResultSubtype,
    /// The run's last words, when the answer above does not already say them.
    pub text: Option<&'a str>,
    pub cost_usd: f64,
    pub turns: u32,
    pub wall_ms: Option<f64>,
    pub evidence: Option<&'a Evidence>,
    pub verified: bool,
    /// This thread's id, so a check another thread ran is named.
    pub thread: Option<&'a str>,
    /// Set on the thread's latest receipt, which carries the rating and the next step.
    pub actions: Option<Actions>,
}

pub struct Actions {
    pub rating: Option<Rating>,
    pub emit: Emit,
}

impl Receipt<'_> {
    pub fn render(self, cx: &App) -> Div {
        let colors = cx.theme().colors();
        let status = cx.theme().status();
        let unverified = self
            .evidence
            .map_or(0, |e| e.unverified.len() + e.flags.len());
        let badge = if self.verified {
            div()
                .px(px(6.))
                .rounded(px(4.))
                .bg(colors.text_accent)
                .text_color(colors.background)
                .child("verified")
        } else {
            div()
                .px(px(6.))
                .rounded(px(4.))
                .border_1()
                .border_dashed()
                .border_color(status.warning)
                .text_color(status.warning)
                .child(match unverified {
                    0 => "unverified".to_owned(),
                    n => format!("unverified {n}"),
                })
        };
        let mut figures = vec![match self.turns {
            1 => "1 turn".to_owned(),
            n => format!("{n} turns"),
        }];
        if let Some(ms) = self.wall_ms {
            figures.push(tool_card::duration(ms as i64));
        }
        figures.push(format!("${:.4}", self.cost_usd));
        let header = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                badge
                    .flex_none()
                    .text_label()
                    .font_weight(FontWeight::SEMIBOLD),
            )
            .child(
                div()
                    .flex_grow(1.)
                    .font_weight(FontWeight::MEDIUM)
                    .child(verdict(self.subtype)),
            )
            .child(
                div()
                    .flex_none()
                    .font(tokens::code_font())
                    .text_meta()
                    .text_color(colors.text_muted)
                    .child(figures.join(" · ")),
            );
        let mut card = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded(px(12.))
            .border_1()
            .border_color(colors.text_accent)
            .bg(colors.editor_background)
            .text_ui()
            .child(header);
        if let Some(text) = self.text {
            card = card.child(div().text_message().child(wrapped(text)));
        }
        if let Some(evidence) = self.evidence {
            let mut lines = div().flex().flex_col().gap_1().text_meta();
            for check in &evidence.checks {
                let passed = check.exit_code == 0.;
                let origin = match check.thread.as_deref() {
                    Some(thread) if Some(thread) != self.thread => format!(" · ใน {thread}"),
                    _ => String::new(),
                };
                lines = lines.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_none()
                                .text_color(if passed { status.success } else { status.error })
                                .child(if passed { "✓" } else { "✕" }),
                        )
                        .child(div().font(tokens::code_font()).child(format!(
                            "{} · exit {}{origin}",
                            check.command, check.exit_code
                        ))),
                );
            }
            for path in &evidence.changed {
                lines = lines.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_color(colors.text_muted)
                        .child(
                            svg()
                                .path("icons/file.svg")
                                .size(px(12.))
                                .flex_none()
                                .text_color(colors.text_muted),
                        )
                        .child(div().font(tokens::code_font()).child(path.clone())),
                );
            }
            for path in evidence.unverified.iter().chain(&evidence.flags) {
                lines = lines.child(
                    div()
                        .text_color(status.warning)
                        .child(format!("ยังไม่ได้ตรวจ: {path}")),
                );
            }
            card = card.child(lines);
        }
        if let Some(actions) = self.actions {
            card = card.child(actions.render(evidence_changed(self.evidence), cx));
        }
        card
    }
}

fn evidence_changed(evidence: Option<&Evidence>) -> bool {
    evidence.is_some_and(|e| !e.changed.is_empty())
}

impl Actions {
    fn render(self, changed: bool, cx: &App) -> Div {
        let colors = cx.theme().colors();
        let button = |id: &'static str| {
            div()
                .id(id)
                .h(px(28.))
                .px_3()
                .flex()
                .items_center()
                .gap_1()
                .rounded(px(7.))
                .border_1()
                .border_color(colors.border)
                .bg(colors.element_background)
                .text_ui()
                .cursor_pointer()
                .hover(|b| b.bg(colors.element_hover))
        };
        let thumb = |id: &'static str, rating: Rating, icon: &'static str| {
            let on = self.rating == Some(rating);
            let emit = self.emit.clone();
            button(id)
                .when(on, |b| b.border_color(colors.text_accent))
                .child(svg().path(icon).size(px(14.)).text_color(if on {
                    colors.text_accent
                } else {
                    colors.text_muted
                }))
                .on_click(move |_, _, cx| emit(AgentEvent::Rate(rating), cx))
        };
        let review = self.emit.clone();
        div()
            .flex()
            .gap_2()
            .pt_1()
            .when(changed, |row| {
                row.child(
                    button("receipt-review")
                        .child("รีวิว diff")
                        .on_click(move |_, _, cx| review(AgentEvent::Review, cx)),
                )
            })
            .child(div().flex_grow(1.))
            .child(thumb("rate-up", Rating::Up, "icons/thumbs_up.svg"))
            .child(thumb("rate-down", Rating::Down, "icons/thumbs_down.svg"))
    }
}

pub fn verdict(subtype: ResultSubtype) -> &'static str {
    match subtype {
        ResultSubtype::Success => "เสร็จ",
        ResultSubtype::ErrorMaxTurns => "ครบจำนวน turn ที่ให้",
        ResultSubtype::ErrorMaxBudgetUsd => "ครบงบที่ให้",
        ResultSubtype::ErrorDuringExecution => "ผิดพลาดระหว่างทำ",
        ResultSubtype::Interrupted => "ขาดตอน",
        ResultSubtype::ErrorUnverifiedClaim => "บอกว่าเสร็จแต่ตรวจไม่ผ่าน",
        ResultSubtype::Handoff => "ส่งต่อให้ session ใหม่",
        ResultSubtype::Asked => "ถามคุณ",
    }
}
