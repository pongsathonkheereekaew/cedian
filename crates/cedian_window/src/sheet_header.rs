//! The 48 px header of the main sheet (Interface, Polish): the open thread's state dot, title
//! and cost, then the tools as icon toggles, then Stop while a run works. It replaced the
//! toolbar row and the rail.

use cedian_wire::Status;
use gpui::{
    App, Div, FontWeight, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::{
    agent_pane::STOP_KEY, thread_list, tokens, tool_rail::OnClick, type_scale::TypeScale as _,
};

pub const HEADER_HEIGHT: f32 = 48.;

pub struct HeaderLine {
    pub title: SharedString,
    pub status: Option<Status>,
    pub cost: Option<f64>,
}

/// The header, with `toggles` (the tools) before Stop.
pub fn header(line: HeaderLine, toggles: Div, stop: OnClick, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let working = line.status == Some(Status::Working);
    div()
        .h(px(HEADER_HEIGHT))
        .flex_none()
        .pl_4()
        .pr_2()
        .flex()
        .items_center()
        .gap_2p5()
        .border_b_1()
        .border_color(colors.border)
        .children(line.status.map(|status| thread_list::dot(status, cx)))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_pane_title()
                .font_weight(FontWeight::SEMIBOLD)
                .child(line.title),
        )
        .children(line.status.map(|status| {
            div()
                .flex_none()
                .text_meta()
                .text_color(colors.text_muted)
                .child(thread_list::state_word(status))
        }))
        .children(line.cost.map(|cost| {
            div()
                .flex_none()
                .font(tokens::code_font())
                .text_meta()
                .text_color(colors.text_muted)
                .child(format!("${cost:.4}"))
        }))
        .child(div().flex_grow(1.))
        .child(toggles)
        .when(working, |row| {
            row.child(
                div()
                    .id("stop")
                    .flex_none()
                    .ml_1()
                    .h(px(30.))
                    .px_2p5()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors.border_selected)
                    .hover(|b| b.bg(colors.element_hover))
                    .cursor_pointer()
                    .text_ui()
                    .child(
                        svg()
                            .path("icons/stop.svg")
                            .size(px(12.))
                            .text_color(colors.text),
                    )
                    .child("หยุด")
                    .child(
                        div()
                            .text_label()
                            .text_color(colors.text_muted)
                            .child(STOP_KEY),
                    )
                    .on_click(stop),
            )
        })
}
