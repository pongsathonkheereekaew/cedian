//! Moving a block with the mouse (Interface, Blocks): its header drags a card; over another
//! block the half it would take is tinted with `accent soft`, outlined, and named in a pill
//! (แบ่งขวา · Terminal); the middle makes it a tab there.

use cedian_layout::{BlockId, Side};
use gpui::{
    AnyElement, App, Bounds, Context, FontWeight, IntoElement, ParentElement as _, Pixels, Point,
    Render, SharedString, Styled as _, Window, div, px, relative, svg,
};
use theme::ActiveTheme as _;

use crate::type_scale::TypeScale as _;

/// A block being dragged by its header.
#[derive(Clone)]
pub struct BlockDrag {
    pub id: BlockId,
    pub label: SharedString,
    pub icon: &'static str,
}

impl Render for BlockDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div()
            .w(px(260.))
            .rounded(px(12.))
            .border_1()
            .border_color(colors.border_selected)
            .bg(colors.editor_background)
            .shadow_lg()
            .overflow_hidden()
            .opacity(0.96)
            .child(
                div()
                    .h(px(34.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(colors.border)
                    .text_ui()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors.text)
                    .child(
                        svg()
                            .path(self.icon)
                            .size(px(14.))
                            .text_color(colors.text_muted),
                    )
                    .child(div().min_w_0().truncate().child(self.label.clone())),
            )
            .child(div().h(px(54.)).bg(colors.panel_background))
    }
}

/// A divider being dragged: the split at `path` and the divider after child `divider`.
#[derive(Clone)]
pub struct SplitDrag {
    pub path: Vec<usize>,
    pub divider: usize,
}

impl Render for SplitDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // The divider itself moves; nothing follows the pointer.
        div()
    }
}

/// Which part of a block the pointer is over: the outer quarter on each side splits there,
/// the middle makes a tab.
pub fn zone(bounds: Bounds<Pixels>, at: Point<Pixels>) -> Side {
    let x = f32::from(at.x - bounds.left()) / f32::from(bounds.size.width).max(1.);
    let y = f32::from(at.y - bounds.top()) / f32::from(bounds.size.height).max(1.);
    let edges = [
        (x, Side::Left),
        (1. - x, Side::Right),
        (y, Side::Up),
        (1. - y, Side::Down),
    ];
    let (near, side) = edges
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap_or((1., Side::Center));
    if near < 0.25 { side } else { Side::Center }
}

pub fn side_word(side: Side) -> &'static str {
    match side {
        Side::Left => "แบ่งซ้าย",
        Side::Right => "แบ่งขวา",
        Side::Up => "แบ่งบน",
        Side::Down => "แบ่งล่าง",
        Side::Center => "เป็นแท็บ",
    }
}

/// The tint over the part of a block a drop would take, with its pill.
pub fn drop_zone(side: Side, label: &str, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    // Neutral: in Warm & Lime the accent is lime, which is kept for the person.
    let tint = colors.text;
    let zone = div()
        .absolute()
        .rounded(px(10.))
        .border_2()
        .border_color(tint.opacity(0.35))
        .bg(tint.opacity(0.07))
        .flex()
        .justify_center()
        .pt(px(14.));
    let zone = match side {
        Side::Left => zone.top_0().bottom_0().left_0().w(relative(0.5)),
        Side::Right => zone.top_0().bottom_0().right_0().w(relative(0.5)),
        Side::Up => zone.left_0().right_0().top_0().h(relative(0.5)),
        Side::Down => zone.left_0().right_0().bottom_0().h(relative(0.5)),
        Side::Center => zone.top_0().bottom_0().left_0().right_0(),
    };
    zone.child(
        div()
            .h(px(26.))
            .px_3()
            .flex()
            .items_center()
            .rounded(px(13.))
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .shadow_md()
            .text_ui()
            .font_weight(FontWeight::MEDIUM)
            .text_color(colors.text)
            .child(format!("{} · {label}", side_word(side))),
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    #[test]
    fn the_outer_quarter_splits_and_the_middle_makes_a_tab() {
        let b = Bounds::new(point(px(100.), px(100.)), size(px(400.), px(200.)));
        assert_eq!(zone(b, point(px(120.), px(200.))), Side::Left);
        assert_eq!(zone(b, point(px(480.), px(200.))), Side::Right);
        assert_eq!(zone(b, point(px(300.), px(110.))), Side::Up);
        assert_eq!(zone(b, point(px(300.), px(290.))), Side::Down);
        assert_eq!(zone(b, point(px(300.), px(200.))), Side::Center);
    }
}
