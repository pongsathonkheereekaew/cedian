//! The project tabs in the title bar, drawn like Chrome's (Interface, Layout): a darker strip
//! with one tab per open project, each with its working count and its badge, a close button
//! while more than one is open, and `+` to open another project.

use gpui::{
    App, Div, InteractiveElement as _, MouseButton, ParentElement as _, Pixels, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};
use theme::ActiveTheme as _;

use crate::{thread_list, tokens::Tokens, tool_rail::OnClick};

gpui::actions!(tabs, [NewTab, CloseTab, NextTab, PreviousTab]);

/// `⌘1`–`⌘9`: the tab [`nth`] names.
#[derive(Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui::Action)]
#[action(namespace = tabs)]
pub struct ActivateTab(pub usize);

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

/// What a press on the strip does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TabClick {
    Activate(usize),
    Close(usize),
    New,
}

/// The strip, without the window-moving listeners the root view adds to it.
pub fn strip(
    tabs: &[ProjectTab],
    active: usize,
    cx: &App,
    click: impl Fn(TabClick) -> OnClick,
) -> Div {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let tokens = Tokens::of(cx);
    let lead = if cfg!(target_os = "macos") {
        TRAFFIC_LIGHTS
    } else {
        px(8.)
    };
    let closable = tabs.len() > 1;
    let mut row = div()
        .flex_none()
        .h(STRIP_HEIGHT)
        .w_full()
        .flex()
        .items_end()
        .gap_1()
        .pl(lead)
        .pr_2()
        .bg(tokens.frame);
    for (n, tab) in tabs.iter().enumerate() {
        let selected = n == active;
        row = row.child(
            div()
                .id(("tab", n))
                .h(TAB_HEIGHT)
                .w(TAB_WIDTH)
                .min_w(px(96.))
                .flex_shrink(1.)
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .rounded_t(px(10.))
                .text_size(px(13.))
                .map(|t| {
                    if selected {
                        t.bg(colors.panel_background).text_color(colors.text)
                    } else {
                        t.cursor_pointer()
                            .text_color(colors.text_muted)
                            .hover(|t| t.bg(colors.element_hover))
                    }
                })
                // A press on a tab picks it; it does not start moving the window.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(click(TabClick::Activate(n)))
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
                })
                .when(closable, |row| {
                    row.child(
                        div()
                            .id(("close-tab", n))
                            .px_1()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_color(colors.text_muted)
                            .hover(|c| c.bg(colors.element_hover).text_color(colors.text))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(click(TabClick::Close(n)))
                            .child("×"),
                    )
                }),
        );
    }
    row.child(
        div()
            .id("new-tab")
            .mb(px(4.))
            .size(px(26.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .cursor_pointer()
            .text_size(px(16.))
            .text_color(colors.text_muted)
            .hover(|b| b.bg(colors.element_hover).text_color(colors.text))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(click(TabClick::New))
            .child("+"),
    )
}

/// The tab `⌃Tab` (`step` 1) or `⌃⇧Tab` (`step` -1) moves to, wrapping round.
pub fn step(active: usize, count: usize, step: isize) -> usize {
    if count == 0 {
        return 0;
    }
    (active as isize + step).rem_euclid(count as isize) as usize
}

/// The tab `⌘n` picks: the nth, or the last for `⌘9`, as in Chrome.
pub fn nth(n: usize, count: usize) -> Option<usize> {
    match n {
        9 => count.checked_sub(1),
        1..=8 if n <= count => Some(n - 1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_wrap_round_and_nine_is_the_last() {
        assert_eq!(step(0, 3, -1), 2);
        assert_eq!(step(2, 3, 1), 0);
        assert_eq!(step(1, 3, 1), 2);
        assert_eq!(nth(1, 3), Some(0));
        assert_eq!(nth(4, 3), None);
        assert_eq!(nth(9, 3), Some(2));
        assert_eq!(nth(9, 0), None);
    }
}
