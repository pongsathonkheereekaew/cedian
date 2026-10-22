//! The status bar along the window's foot (Interface, Layout: status bar, 26 px): whether the
//! daemon answers, the cedia it runs, how many runs work and how many threads need you, and
//! the way to Settings.

use gpui::{
    App, Div, InteractiveElement as _, ParentElement as _, SharedString, Stateful, Styled as _,
    div, prelude::FluentBuilder as _, px,
};
use theme::ActiveTheme as _;

pub const STATUS_HEIGHT: f32 = 26.;

pub struct StatusLine {
    /// The daemon's state in words, and whether it answers.
    pub daemon: SharedString,
    pub connected: bool,
    pub cedia: Option<SharedString>,
    pub working: usize,
    pub waiting: usize,
}

/// The bar, with `settings` as its last item.
pub fn bar(line: StatusLine, settings: Stateful<Div>, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let sep = || div().text_color(colors.border).child("·");
    div()
        .h(px(STATUS_HEIGHT))
        .flex_none()
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .border_t_1()
        .border_color(colors.border)
        .bg(colors.panel_background)
        .text_size(px(12.))
        .text_color(colors.text_muted)
        .child(div().size(px(6.)).rounded_full().bg(if line.connected {
            status.success
        } else {
            status.error
        }))
        .child(line.daemon)
        .when_some(line.cedia, |bar, cedia| bar.child(sep()).child(cedia))
        .child(sep())
        .child(format!("{} กำลังทำ", line.working))
        .when(line.waiting > 0, |bar| {
            bar.child(sep()).child(
                div()
                    .text_color(colors.text_accent)
                    .child(format!("{} ต้องดู", line.waiting)),
            )
        })
        .child(div().flex_grow(1.))
        .child(
            settings
                .cursor_pointer()
                .hover(|s| s.text_color(colors.text)),
        )
}
