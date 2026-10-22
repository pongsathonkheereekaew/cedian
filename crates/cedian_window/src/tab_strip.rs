//! The project tabs in the title bar, drawn like Chrome's (Interface, Layout): a darker strip
//! with one tab per open project, each with its working count and its badge, a close button
//! while more than one is open, and `+` to open another project. A tab drags to a new place.

use gpui::{
    App, AppContext as _, Context, Div, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, prelude::FluentBuilder as _, px,
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

/// A tab being dragged: where it was, and its name for the label under the pointer.
#[derive(Clone)]
pub struct TabDrag {
    pub from: usize,
    name: SharedString,
}

impl Render for TabDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div()
            .px_3()
            .py_1()
            .rounded(px(8.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .text_color(colors.text)
            .text_size(px(13.))
            .child(self.name.clone())
    }
}

/// What dropping a dragged tab on tab `n` does.
pub type OnDrop = Box<dyn Fn(&TabDrag, &mut Window, &mut App) + 'static>;

/// The strip, without the window-moving listeners the root view adds to it.
pub fn strip(
    tabs: &[ProjectTab],
    active: usize,
    cx: &App,
    click: impl Fn(TabClick) -> OnClick,
    dropped: impl Fn(usize) -> OnDrop,
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
                .on_drag(
                    TabDrag {
                        from: n,
                        name: tab.name.clone(),
                    },
                    |drag, _, _, cx| cx.new(|_| drag.clone()),
                )
                .drag_over::<TabDrag>(move |style, _, _, cx| {
                    style.bg(cx.theme().colors().element_hover)
                })
                .on_drop(dropped(n))
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

/// The order after the tab at `from` is dropped on the tab at `to`: it takes that place and
/// the tabs between shift over by one.
pub fn moved<T>(items: &mut Vec<T>, from: usize, to: usize) {
    if from < items.len() && to < items.len() && from != to {
        let item = items.remove(from);
        items.insert(to, item);
    }
}

/// Where the tab at `active` is once the tab at `from` moved to `to`.
pub fn follow(active: usize, from: usize, to: usize) -> usize {
    if active == from {
        to
    } else if from < active && active <= to {
        active - 1
    } else if to <= active && active < from {
        active + 1
    } else {
        active
    }
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

    #[test]
    fn a_dropped_tab_takes_the_place_and_the_front_tab_follows() {
        for (from, to) in [(0, 2), (2, 0), (1, 3), (3, 1), (1, 1)] {
            let mut tabs = vec!['a', 'b', 'c', 'd'];
            moved(&mut tabs, from, to);
            for active in 0..4 {
                let before = ['a', 'b', 'c', 'd'][active];
                assert_eq!(
                    tabs[follow(active, from, to)],
                    before,
                    "{from}->{to} {active}"
                );
            }
        }
        let mut tabs = vec!['a', 'b'];
        moved(&mut tabs, 0, 5);
        assert_eq!(tabs, ['a', 'b']);
    }
}
