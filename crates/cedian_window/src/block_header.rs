//! A block's 36 px header (Interface, Blocks): the kind's icon, the tabs (a thread's tab with
//! its state dot), a short meta in the code face, then Zoom. A block that holds one tab and has
//! controls of its own (Browser, Review) puts them in place of the icon, the tab and the meta,
//! as Tern does. The grip shows in the middle of the focused block's header, and on hover on the
//! others; it opens the block's menu, and the header drags from anywhere a control is not.

use cedian_layout::{BlockId, Kind};
use cedian_wire::Status;
use gpui::{
    AnyElement, App, Div, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::{on_click::OnClick, thread_list, tokens, type_scale::TypeScale as _};

pub const HEADER_HEIGHT: f32 = 36.;
/// The grip's height, from the header's top: the block's menu opens under it.
pub const GRIP_HEIGHT: f32 = 11.;

pub struct Tab {
    pub id: BlockId,
    pub label: SharedString,
    /// A thread's state, drawn as its dot.
    pub status: Option<Status>,
    /// Lime: the project chat's dot when it waits on the person.
    pub calls: bool,
}

pub struct HeaderSpec {
    pub id: BlockId,
    pub kind: Kind,
    pub tabs: Vec<Tab>,
    pub active: BlockId,
    pub meta: Option<SharedString>,
    pub focused: bool,
    pub zoomed: bool,
    /// Under 360 px the meta goes.
    pub narrow: bool,
    /// The most tabs that fit; the rest wait behind a `+n` tab.
    pub max_tabs: usize,
    /// The block's own controls, drawn in place of the icon, the tabs and the meta.
    pub bar: Option<AnyElement>,
}

pub struct HeaderActions {
    pub tab: Box<dyn Fn(BlockId) -> OnClick>,
    pub zoom: OnClick,
    pub menu: OnClick,
}

pub fn icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Threads => "icons/menu.svg",
        Kind::Agent => "icons/thread.svg",
        Kind::Chat => "icons/chat.svg",
        Kind::Terminal => "icons/terminal.svg",
        Kind::Browser => "icons/public.svg",
        Kind::Review => "icons/file_diff.svg",
        Kind::Editor => "icons/file_code.svg",
        Kind::Files => "icons/file_tree.svg",
        Kind::Git => "icons/git_branch.svg",
        Kind::Outputs => "icons/box_open.svg",
        Kind::Memory => "icons/book.svg",
        Kind::Skills => "icons/sparkle.svg",
        Kind::Usage => "icons/signal_high.svg",
        Kind::Background => "icons/clock.svg",
        Kind::Session => "icons/info.svg",
    }
}

/// The header, `grip` being where a drag starts (the caller adds the drag to it).
pub fn header(spec: HeaderSpec, actions: HeaderActions, cx: &App) -> Stateful<Div> {
    let colors = cx.theme().colors();
    let button = |id: SharedString, icon: &'static str| {
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
            // A press on a button is not the start of a drag.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(svg().path(icon).size(px(14.)).text_color(colors.text_muted))
    };
    let meta = spec.meta.clone().filter(|_| !spec.narrow);
    let (shown, behind) = fit(&spec.tabs, spec.active, spec.max_tabs);
    let mut tabs = div()
        .flex_grow(1.)
        .min_w_0()
        .flex()
        .items_center()
        .gap_0p5()
        .overflow_hidden();
    for tab in shown {
        let on = tab.id == spec.active;
        tabs = tabs.child(
            div()
                .id(SharedString::from(format!("block-tab-{}", tab.id)))
                .h(px(26.))
                .min_w_0()
                .max_w(px(230.))
                .px(px(9.))
                .flex()
                .items_center()
                .gap(px(7.))
                .rounded(px(7.))
                .text_ui()
                .map(|t| {
                    if on {
                        t.bg(colors.element_selected)
                            .text_color(colors.text)
                            .font_weight(FontWeight::MEDIUM)
                    } else {
                        t.text_color(colors.text_muted)
                            .cursor_pointer()
                            .hover(|t| t.text_color(colors.text))
                    }
                })
                .when_some(tab.status, |t, status| {
                    t.child(thread_list::dot(status, cx))
                })
                .when(tab.calls, |t| {
                    t.child(
                        div()
                            .flex_none()
                            .size(px(7.))
                            .rounded_full()
                            .bg(tokens::Tokens::of(cx).primary),
                    )
                })
                .child(crate::tooltip::cut("label", tab.label.clone()))
                .when(!on, |t| t.on_click((actions.tab)(tab.id))),
        );
    }
    if let Some(first) = behind.first() {
        tabs = tabs.child(
            div()
                .id(SharedString::from(format!("block-more-{}", spec.id)))
                .flex_none()
                .h(px(26.))
                .px(px(7.))
                .flex()
                .items_center()
                .rounded(px(7.))
                .cursor_pointer()
                .font(tokens::numeric_font())
                .text_meta()
                .text_color(colors.text_muted)
                .hover(|t| t.bg(colors.element_hover).text_color(colors.text))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click((actions.tab)(first.id))
                .child(format!("+{}", behind.len())),
        );
    }
    div()
        .id(SharedString::from(format!("block-header-{}", spec.id)))
        .group("block-header")
        .relative()
        .h(px(HEADER_HEIGHT))
        .flex_none()
        .pl_2()
        .pr(px(6.))
        .flex()
        .items_center()
        .gap_1()
        .cursor_grab()
        .map(|h| match spec.bar {
            Some(bar) => h.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    // A press on a control is not the start of a drag.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(bar),
            ),
            None => h
                .child(
                    div()
                        .flex_none()
                        .size(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            svg()
                                .path(icon(spec.kind))
                                .size(px(14.))
                                .text_color(colors.text_muted),
                        ),
                )
                .child(tabs)
                .when_some(meta, |h, meta| {
                    h.child(
                        div()
                            .flex_none()
                            .px_1()
                            .font(tokens::numeric_font())
                            .text_meta()
                            .text_color(colors.text_placeholder)
                            .child(meta),
                    )
                }),
        })
        .child(
            button(
                SharedString::from(format!("block-zoom-{}", spec.id)),
                if spec.zoomed {
                    "icons/minimize.svg"
                } else {
                    "icons/maximize.svg"
                },
            )
            .on_click(actions.zoom),
        )
        .child(grip(spec.id, spec.focused, actions.menu, cx))
}

/// The tabs that fit, in order and always with the open one, and the ones behind `+n`.
fn fit(tabs: &[Tab], active: BlockId, max: usize) -> (Vec<&Tab>, Vec<&Tab>) {
    let max = max.max(1);
    if tabs.len() <= max {
        return (tabs.iter().collect(), Vec::new());
    }
    let open = tabs.iter().position(|t| t.id == active).unwrap_or(0);
    // The open tab and the ones before it, then after it, up to `max`.
    let start = open.saturating_sub(max - 1).min(tabs.len() - max);
    let shown: Vec<&Tab> = tabs[start..start + max].iter().collect();
    let behind = tabs
        .iter()
        .enumerate()
        .filter(|(n, _)| *n < start || *n >= start + max)
        .map(|(_, t)| t)
        .collect();
    (shown, behind)
}

/// The ⠿ in the middle of the header: always on the focused block, on hover elsewhere. A
/// click opens the block's menu; a drag from it moves the block like the rest of the header.
fn grip(id: BlockId, focused: bool, menu: OnClick, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let dots = (0..2).fold(div().flex().flex_col().gap(px(2.)), |col, _| {
        col.child((0..3).fold(div().flex().gap(px(4.)), |row, _| {
            row.child(
                div()
                    .size(px(2.))
                    .rounded_full()
                    .bg(colors.text_placeholder),
            )
        }))
    });
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            div()
                .id(SharedString::from(format!("block-menu-{id}")))
                .debug_selector(move || format!("block-grip-{id}"))
                .w(px(36.))
                .h(px(GRIP_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded_b(px(5.))
                .cursor_pointer()
                .opacity(if focused { 0.75 } else { 0. })
                .group_hover("block-header", |g| g.opacity(0.75))
                .hover(|g| g.opacity(1.).bg(colors.element_hover))
                .on_click(menu)
                .child(dots),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(id: BlockId) -> Tab {
        Tab {
            id,
            label: id.to_string().into(),
            status: None,
            calls: false,
        }
    }

    #[test]
    fn the_open_tab_always_shows_and_the_rest_wait_behind_more() {
        let tabs: Vec<Tab> = (1..=5).map(tab).collect();
        let ids = |v: Vec<&Tab>| v.iter().map(|t| t.id).collect::<Vec<_>>();
        let (shown, behind) = fit(&tabs, 1, 2);
        assert_eq!(ids(shown), [1, 2]);
        assert_eq!(ids(behind), [3, 4, 5]);
        let (shown, behind) = fit(&tabs, 5, 2);
        assert_eq!(ids(shown), [4, 5]);
        assert_eq!(ids(behind), [1, 2, 3]);
        let (shown, behind) = fit(&tabs, 3, 1);
        assert_eq!(ids(shown), [3]);
        assert_eq!(behind.len(), 4);
        let (shown, behind) = fit(&tabs, 3, 9);
        assert_eq!(shown.len(), 5);
        assert!(behind.is_empty());
    }
}
