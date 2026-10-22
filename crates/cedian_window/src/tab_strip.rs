//! The project tabs in the title bar, drawn like Chrome's (Interface, Layout): a darker strip
//! with one tab per open project. One project per window for now, so the strip holds one tab;
//! `+`, close and reordering come with several projects (phase C).

use gpui::{
    App, Div, ParentElement as _, Pixels, SharedString, Styled as _, div,
    prelude::FluentBuilder as _, px,
};
use theme::ActiveTheme as _;

use crate::{thread_list, tokens::Tokens};

pub const STRIP_HEIGHT: Pixels = px(42.);
const TAB_HEIGHT: Pixels = px(34.);
const TAB_WIDTH: Pixels = px(236.);
/// Room for the traffic lights, which macOS draws over the strip.
const TRAFFIC_LIGHTS: Pixels = px(80.);

pub struct ProjectTab {
    pub name: SharedString,
    pub working: usize,
    pub waiting: usize,
    pub failed: bool,
}

/// The strip, without the window-moving listeners the root view adds to it.
pub fn strip(tab: &ProjectTab, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let tokens = Tokens::of(cx);
    let lead = if cfg!(target_os = "macos") {
        TRAFFIC_LIGHTS
    } else {
        px(8.)
    };
    div()
        .flex_none()
        .h(STRIP_HEIGHT)
        .w_full()
        .flex()
        .items_end()
        .pl(lead)
        .bg(tokens.frame)
        .child(
            div()
                .h(TAB_HEIGHT)
                .w(TAB_WIDTH)
                .min_w(px(120.))
                .flex_shrink(1.)
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .rounded_t(px(10.))
                .bg(colors.panel_background)
                .text_color(colors.text)
                .text_size(px(13.))
                .child(
                    div()
                        .flex_grow(1.)
                        .min_w_0()
                        .truncate()
                        .child(tab.name.clone()),
                )
                .when(tab.working > 0, |row| {
                    row.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_size(px(12.))
                            .text_color(status.info)
                            .child(div().size(px(6.)).rounded_full().bg(status.info))
                            .child(tab.working.to_string()),
                    )
                })
                .when(tab.waiting > 0, |row| {
                    row.child(thread_list::badge(tab.waiting, tab.failed, cx))
                }),
        )
}
