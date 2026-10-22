//! Tooltips on ellipsized text (Interface): a label cut short with … shows its whole text on
//! hover, after GPUI's delay; one that fits shows nothing. Whether it was cut is measured where
//! it is drawn, with the label's own font, so nothing guesses from its length.

use std::{cell::Cell, rc::Rc};

use gpui::{
    AppContext as _, Context, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, canvas,
    div, prelude::FluentBuilder as _, px,
};
use theme::ActiveTheme as _;

use crate::type_scale::TypeScale as _;

/// The longest a tooltip gets before it wraps.
const MAX_WIDTH: f32 = 360.;

/// `text` on one line, ellipsized when it does not fit, with its whole text on hover then.
/// The caller sets its size and colour; it takes the font of where it is drawn.
pub fn cut(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<gpui::Div> {
    let text = text.into();
    let was_cut = Rc::new(Cell::new(false));
    let measured = was_cut.clone();
    let shown = text.clone();
    div()
        .id(id)
        .relative()
        .min_w_0()
        .truncate()
        .child(text.clone())
        .child(
            canvas(
                move |bounds, window, _| {
                    let style = window.text_style();
                    let size = style.font_size.to_pixels(window.rem_size());
                    let run = style.to_run(shown.len());
                    let width = window
                        .text_system()
                        .shape_line(shown.clone(), size, &[run], None)
                        .width;
                    // Half a pixel for rounding.
                    measured.set(width > bounds.size.width + px(0.5));
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .tooltip(move |_, cx| {
            let text = was_cut.get().then(|| text.clone());
            cx.new(|_| Tooltip { text }).into()
        })
}

/// What a tooltip shows: the whole text, or nothing for a label that fit.
pub struct Tooltip {
    text: Option<SharedString>,
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div().when_some(self.text.clone(), |tip, text| {
            tip.debug_selector(|| "tooltip".into())
                .max_w(px(MAX_WIDTH))
                .px_2()
                .py_1()
                .rounded(px(6.))
                .border_1()
                .border_color(colors.border)
                .bg(colors.elevated_surface_background)
                .text_meta()
                .text_color(colors.text)
                .child(text)
        })
    }
}
