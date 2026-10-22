//! The overview (Interface, Blocks: `⌃⌥↑`): every layout of the project as a miniature of its
//! tree, the current one ringed, บันทึก layout นี้, and the parked blocks under them.

use cedian_layout::{BlockId, Dir, Node, Workspace};
use gpui::{
    AnyElement, App, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::{
    block_header::icon, block_keys, on_click::OnClick, tokens::Tokens, type_scale::TypeScale as _,
};

pub struct OverviewActions {
    pub layout: Box<dyn Fn(usize) -> OnClick>,
    pub unpark: Box<dyn Fn(BlockId) -> OnClick>,
    pub save: OnClick,
    pub close: OnClick,
}

pub fn overview(ws: &Workspace, actions: OverviewActions, cx: &App) -> Stateful<Div> {
    let colors = cx.theme().colors();
    let mut layouts = div().flex().flex_wrap().justify_center().gap(px(18.));
    for (n, layout) in ws.layouts.iter().enumerate() {
        let current = n == ws.current;
        layouts = layouts.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .id(("overview-layout", n))
                        .w(px(250.))
                        .h(px(158.))
                        .p(px(5.))
                        .flex()
                        .rounded(px(12.))
                        .bg(colors.background)
                        .border_2()
                        .border_color(if current { colors.text } else { colors.border })
                        .cursor_pointer()
                        .hover(|m| m.border_color(colors.text_muted))
                        .on_click((actions.layout)(n))
                        .children(layout.tree.as_ref().map(|tree| mini(tree, ws, cx))),
                )
                .child(
                    div()
                        .flex()
                        .gap_1p5()
                        .text_ui()
                        .text_color(colors.text)
                        .child(layout.name.clone())
                        .when(n < 9, |name| {
                            name.child(
                                div()
                                    .text_label()
                                    .text_color(colors.text_placeholder)
                                    .child(block_keys::shown(&format!(
                                        "{}-{}",
                                        block_keys::LAYOUT,
                                        n + 1
                                    ))),
                            )
                        }),
                ),
        );
    }
    layouts = layouts.child(
        div()
            .id("overview-save")
            .w(px(150.))
            .h(px(158.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1p5()
            .rounded(px(12.))
            .border_1()
            .border_dashed()
            .border_color(colors.border_selected)
            .text_ui()
            .text_color(colors.text_muted)
            .cursor_pointer()
            .hover(|b| b.text_color(colors.text))
            .on_click(actions.save)
            .child(
                svg()
                    .path("icons/plus.svg")
                    .size(px(15.))
                    .text_color(colors.text_muted),
            )
            .child("บันทึก layout นี้"),
    );
    let parked = (!ws.parked.is_empty()).then(|| {
        let mut row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_center()
            .gap_2()
            .py_2()
            .pl_3()
            .pr_2()
            .rounded(px(12.))
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_meta()
            .text_color(colors.text_placeholder)
            .child("พักไว้");
        for id in &ws.parked {
            let kind = ws.kind(*id);
            row = row.child(
                div()
                    .id(SharedString::from(format!("unpark-{id}")))
                    .h(px(28.))
                    .px_2p5()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .rounded(px(8.))
                    .bg(Tokens::of(cx).field)
                    .text_ui()
                    .text_color(colors.text)
                    .cursor_pointer()
                    .on_click((actions.unpark)(*id))
                    .children(kind.map(|k| {
                        svg()
                            .path(icon(k))
                            .size(px(13.))
                            .text_color(colors.text_muted)
                    }))
                    .child(kind.map_or("Block", |k| k.name())),
            );
        }
        row
    });
    div()
        .id("overview")
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(26.))
        .p_6()
        .bg(Tokens::of(cx).frame)
        .on_click(actions.close)
        .child(
            div()
                .text_ui()
                .text_color(colors.text_muted)
                .child("เลือก layout · Esc กลับ"),
        )
        .child(layouts)
        .children(parked)
}

/// A layout's tree drawn small: each leaf names the kind it shows.
fn mini(node: &Node, ws: &Workspace, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    match node {
        Node::Leaf { tabs, active } => {
            let kind = tabs.get(*active).and_then(|id| ws.kind(*id));
            div()
                .flex_basis(px(0.))
                .flex_grow(1.)
                .min_w_0()
                .min_h_0()
                .flex()
                .gap_1()
                .px(px(6.))
                .py(px(5.))
                .rounded(px(5.))
                .bg(colors.editor_background)
                .border_1()
                .border_color(colors.border)
                .overflow_hidden()
                .text_size(px(9.5))
                .text_color(colors.text_placeholder)
                .font_weight(FontWeight::MEDIUM)
                .child(kind.map_or("", |k| k.name()))
                .when(tabs.len() > 1, |l| l.child(format!("+{}", tabs.len() - 1)))
                .into_any_element()
        }
        Node::Split { dir, sizes, kids } => {
            let mut split = div()
                .flex_basis(px(0.))
                .flex_grow(1.)
                .min_w_0()
                .min_h_0()
                .flex()
                .gap(px(4.));
            if *dir == Dir::Col {
                split = split.flex_col();
            }
            for (size, kid) in sizes.iter().zip(kids) {
                split = split.child(
                    div()
                        .flex_basis(px(0.))
                        .flex_grow(*size)
                        .min_w_0()
                        .min_h_0()
                        .flex()
                        .child(mini(kid, ws, cx)),
                );
            }
            split.into_any_element()
        }
    }
}
