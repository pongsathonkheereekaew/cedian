//! A menu of rows over the window: a block's ⋯, + Tool, the layouts and the run's choices.
//! Each row has a label, a key, and may be off with a word saying why; a choice row also has
//! a check and a line under its label.

use gpui::{
    App, Div, InteractiveElement as _, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px, svg,
};
use theme::ActiveTheme as _;

use crate::{on_click::OnClick, type_scale::TypeScale as _};

pub enum Row {
    Item {
        icon: Option<&'static str>,
        label: SharedString,
        /// The key, or a word such as เปิดอยู่.
        note: Option<SharedString>,
        on_click: Option<OnClick>,
    },
    /// One of a set where one is chosen: the model, the permission mode.
    Choice {
        chosen: bool,
        label: SharedString,
        note: Option<SharedString>,
        /// What it does, or why it is off, wrapped under the label.
        detail: Option<SharedString>,
        on_click: Option<OnClick>,
    },
    Label(SharedString),
    Separator,
}

impl Row {
    pub fn item(label: impl Into<SharedString>, note: Option<String>, on_click: OnClick) -> Self {
        Row::Item {
            icon: None,
            label: label.into(),
            note: note.map(Into::into),
            on_click: Some(on_click),
        }
    }
}

/// The widest a menu grows; a choice's detail wraps inside it.
const MAX_WIDTH: f32 = 340.;

/// The menu's card; the caller places it.
pub fn menu(id: &'static str, rows: Vec<Row>, cx: &App) -> Stateful<Div> {
    let colors = cx.theme().colors();
    let mut card = div()
        .id(id)
        .min_w(px(220.))
        .max_w(px(MAX_WIDTH))
        .max_h(px(520.))
        .overflow_y_scroll()
        .p(px(5.))
        .flex()
        .flex_col()
        .rounded(px(12.))
        .border_1()
        .border_color(colors.border)
        .bg(colors.elevated_surface_background)
        .shadow_lg()
        .text_ui()
        .text_color(colors.text);
    for (n, row) in rows.into_iter().enumerate() {
        card = match row {
            Row::Separator => card.child(div().h(px(1.)).mx(px(6.)).my_1().bg(colors.border)),
            Row::Label(label) => card.child(
                div()
                    .px(px(9.))
                    .pt(px(6.))
                    .pb(px(3.))
                    .text_label()
                    .text_color(colors.text_placeholder)
                    .child(label),
            ),
            Row::Choice {
                chosen,
                label,
                note,
                detail,
                on_click,
            } => {
                let on = on_click.is_some();
                card.child(
                    div()
                        .id(("menu-row", n))
                        // In a card that scrolls, a row keeps its height.
                        .flex_none()
                        .min_h(px(32.))
                        .px(px(9.))
                        .py(px(6.))
                        .flex()
                        .items_start()
                        .gap(px(8.))
                        .rounded(px(7.))
                        .map(|r| {
                            if on {
                                r.cursor_pointer().hover(|r| r.bg(colors.element_selected))
                            } else {
                                r.text_color(colors.text_placeholder)
                            }
                        })
                        .child(div().flex_none().w(px(14.)).pt(px(3.)).when(chosen, |c| {
                            c.child(
                                svg()
                                    .path("icons/check.svg")
                                    .size(px(14.))
                                    .text_color(colors.text_muted),
                            )
                        }))
                        .child(
                            div()
                                .flex_grow(1.)
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(1.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .child(
                                            div().flex_grow(1.).min_w_0().truncate().child(label),
                                        )
                                        .when_some(note, |r, note| {
                                            r.child(
                                                div()
                                                    .flex_none()
                                                    .text_label()
                                                    .text_color(colors.text_placeholder)
                                                    .child(note),
                                            )
                                        }),
                                )
                                .when_some(detail, |r, detail| {
                                    r.child(
                                        div()
                                            .text_meta()
                                            .text_color(colors.text_placeholder)
                                            .child(detail),
                                    )
                                }),
                        )
                        .when_some(on_click, |r, f| r.on_click(f)),
                )
            }
            Row::Item {
                icon,
                label,
                note,
                on_click,
            } => {
                let on = on_click.is_some();
                card.child(
                    div()
                        .id(("menu-row", n))
                        .h(px(32.))
                        .px(px(9.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .rounded(px(7.))
                        .map(|r| {
                            if on {
                                r.cursor_pointer().hover(|r| r.bg(colors.element_selected))
                            } else {
                                r.text_color(colors.text_placeholder)
                            }
                        })
                        .when_some(icon, |r, icon| {
                            r.child(svg().path(icon).size(px(14.)).text_color(colors.text_muted))
                        })
                        .child(div().flex_grow(1.).min_w_0().truncate().child(label))
                        .when_some(note, |r, note| {
                            r.child(
                                div()
                                    .flex_none()
                                    .pl_3()
                                    .text_label()
                                    .text_color(colors.text_placeholder)
                                    .child(note),
                            )
                        })
                        .when_some(on_click, |r, f| r.on_click(f)),
                )
            }
        };
    }
    card
}
