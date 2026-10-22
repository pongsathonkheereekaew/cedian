//! The sidebar's last line (Interface, Polish: it took the status bar's place): whether the
//! daemon answers and the cedia it runs, how many runs work and how many threads need you
//! across the tabs, and the way to Settings. Today's cost joins it once the daemon reports it.

use crate::type_scale::TypeScale as _;
use gpui::{
    App, Div, InteractiveElement as _, ParentElement as _, SharedString, Stateful, Styled as _,
    div, prelude::FluentBuilder as _, px,
};
use theme::ActiveTheme as _;

pub const FOOT_HEIGHT: f32 = 32.;

pub struct StatusLine {
    /// The daemon's state in words, and whether it answers.
    pub daemon: SharedString,
    pub connected: bool,
    pub cedia: Option<SharedString>,
    pub working: usize,
    pub waiting: usize,
}

/// The line, with `settings` as its last item.
pub fn foot(line: StatusLine, settings: Stateful<Div>, cx: &App) -> Div {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let sep = || div().text_color(colors.border).child("·");
    div()
        .h(px(FOOT_HEIGHT))
        .flex_none()
        .px_4()
        .flex()
        .items_center()
        .gap_2()
        .text_meta()
        .text_color(colors.text_muted)
        .child(
            div()
                .flex_none()
                .size(px(6.))
                .rounded_full()
                .bg(if line.connected {
                    crate::tokens::Tokens::of(cx).diff_add
                } else {
                    status.error
                }),
        )
        .child(div().flex_none().child(line.daemon))
        .when_some(line.cedia, |bar, cedia| {
            bar.child(sep())
                .child(div().min_w_0().truncate().child(cedia))
        })
        .when(line.working > 0, |bar| {
            bar.child(sep())
                .child(div().flex_none().child(format!("{} กำลังทำ", line.working)))
        })
        .when(line.waiting > 0, |bar| {
            bar.child(sep()).child(
                div()
                    .flex_none()
                    .text_color(colors.text)
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
