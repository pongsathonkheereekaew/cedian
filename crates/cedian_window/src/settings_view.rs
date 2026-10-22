//! The settings window's content (`⌘,`). There is no Save button: each change goes to the daemon, which writes
//! the `desktop` key of cedia's settings file, and every window applies the result.

use std::path::PathBuf;

use gpui::{
    AnyElement, Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};
use serde_json::Value;
use theme::ActiveTheme as _;

use crate::{
    desktop::{Desktop, ThemeMode, UI_FONT_SIZES},
    tokens,
};

pub enum SettingsEvent {
    Set { key: &'static str, value: Value },
}

#[derive(Default)]
pub struct SettingsView {
    path: Option<PathBuf>,
    desktop: Value,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl SettingsView {
    pub fn set(&mut self, path: PathBuf, desktop: Value, cx: &mut Context<Self>) {
        self.path = Some(path);
        self.desktop = desktop;
        cx.notify();
    }

    fn size_row(
        &self,
        key: &'static str,
        label: &'static str,
        size: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let step = |id: &'static str, text: &'static str, to: f32, enabled: bool| {
            div()
                .id(SharedString::from(format!("{key}-{id}")))
                .px_2()
                .border_1()
                .border_color(colors.border)
                .when(enabled, |el| el.cursor_pointer())
                .text_color(if enabled {
                    colors.text
                } else {
                    colors.text_disabled
                })
                .child(text)
                .when(enabled, |el| {
                    el.on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(SettingsEvent::Set {
                            key,
                            value: Value::from(to.round() as i64),
                        })
                    }))
                })
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().w_40().child(label))
            .child(step(
                "smaller",
                "−",
                size - 1.0,
                size > *UI_FONT_SIZES.start(),
            ))
            .child(div().w_10().text_center().child(format!("{size}")))
            .child(step("larger", "+", size + 1.0, size < *UI_FONT_SIZES.end()))
            .into_any_element()
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let desktop = Desktop::from_value(&self.desktop);
        let colors = cx.theme().colors();
        let (text, muted, border, accent, selected) = (
            colors.text,
            colors.text_muted,
            colors.border,
            colors.text_accent,
            colors.element_selected,
        );
        let mut themes = div().flex().gap_2();
        for (mode, value, label) in ThemeMode::ALL {
            let on = desktop.theme == mode;
            themes = themes.child(
                div()
                    .id(value)
                    .px_3()
                    .py_2()
                    .border_1()
                    .border_color(if on { accent } else { border })
                    .when(on, |el| el.bg(selected))
                    .cursor_pointer()
                    .child(label)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(SettingsEvent::Set {
                            key: "theme",
                            value: value.into(),
                        })
                    })),
            );
        }
        let file = serde_json::to_string_pretty(&serde_json::json!({ "desktop": self.desktop }))
            .unwrap_or_default();
        let path: SharedString = self
            .path
            .as_ref()
            .map_or("reading the settings".into(), |p| {
                p.display().to_string().into()
            });
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_color(text)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .child("Settings"),
            )
            .child(
                div()
                    .id("settings-body")
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_3()
                    .overflow_y_scroll()
                    .child(div().text_sm().text_color(muted).child("Appearance"))
                    .child(themes)
                    .child(self.size_row(
                        "ui_font_size",
                        "Interface text",
                        desktop.ui_font_size,
                        cx,
                    ))
                    .child(self.size_row("code_font_size", "Code text", desktop.code_font_size, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().text_color(muted).child(format!(
                                "Written to {path} at once; cedia's own keys stay as they are"
                            )))
                            .child(
                                div()
                                    .p_2()
                                    .border_1()
                                    .border_color(border)
                                    .font(tokens::code_font())
                                    .text_xs()
                                    .child(file),
                            ),
                    ),
            )
    }
}
