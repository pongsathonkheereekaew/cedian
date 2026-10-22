//! The settings window's content (`⌘,`, Interface, Settings window): sections down the left,
//! the open section's controls, and on the right the file as it is written, with the line
//! the last change touched marked. There is no Save button: each change goes to the daemon,
//! which writes the `desktop` key of cedia's settings file, and every window applies it.

use std::path::PathBuf;

use gpui::{
    AnyElement, Context, Div, EventEmitter, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _, px,
};
use serde_json::Value;
use theme::ActiveTheme as _;

use crate::{
    density::Density,
    desktop::{Desktop, ThemeMode, UI_FONT_SIZES},
    tokens,
    view_mode::ViewMode,
};

pub enum SettingsEvent {
    Set { key: &'static str, value: Value },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Section {
    #[default]
    Appearance,
    Bots,
    Models,
}

impl Section {
    /// Each section, its word, and whether its keys are cedia's.
    const ALL: [(Section, &'static str, bool); 3] = [
        (Section::Appearance, "รูปลักษณ์", false),
        (Section::Bots, "Bots", true),
        (Section::Models, "Models", true),
    ];
}

#[derive(Default)]
pub struct SettingsView {
    path: Option<PathBuf>,
    desktop: Value,
    section: Section,
    /// The key the last change set, marked in the file column.
    changed: Option<&'static str>,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl SettingsView {
    pub fn set(&mut self, path: PathBuf, desktop: Value, cx: &mut Context<Self>) {
        self.path = Some(path);
        self.desktop = desktop;
        cx.notify();
    }

    fn change(&mut self, key: &'static str, value: Value, cx: &mut Context<Self>) {
        self.changed = Some(key);
        cx.emit(SettingsEvent::Set { key, value });
        cx.notify();
    }

    /// A row of choices, the current one selected.
    fn choices<T: PartialEq + Copy + 'static>(
        &self,
        key: &'static str,
        current: T,
        all: &[(T, &'static str, &'static str)],
        cx: &mut Context<Self>,
    ) -> Div {
        let colors = cx.theme().colors();
        let mut row = div()
            .flex()
            .h(px(30.))
            .rounded(px(7.))
            .border_1()
            .border_color(colors.border)
            .overflow_hidden();
        for &(choice, value, label) in all {
            let on = choice == current;
            row = row.child(
                div()
                    .id(SharedString::from(format!("{key}-{value}")))
                    .px_3()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_size(px(13.))
                    .when(on, |c| {
                        c.bg(colors.element_selected).text_color(colors.text)
                    })
                    .when(!on, |c| {
                        c.text_color(colors.text_muted)
                            .hover(|c| c.bg(colors.element_hover))
                    })
                    .on_click(cx.listener(move |view, _, _, cx| view.change(key, value.into(), cx)))
                    .child(label),
            );
        }
        row
    }

    fn size_control(&self, key: &'static str, size: f32, cx: &mut Context<Self>) -> Div {
        let colors = cx.theme().colors();
        let step = |id: &'static str, text: &'static str, to: f32, enabled: bool| {
            div()
                .id(SharedString::from(format!("{key}-{id}")))
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .when(enabled, |el| {
                    el.cursor_pointer().hover(|el| el.bg(colors.element_hover))
                })
                .text_color(if enabled {
                    colors.text
                } else {
                    colors.text_disabled
                })
                .child(text)
                .when(enabled, |el| {
                    el.on_click(cx.listener(move |view, _, _, cx| {
                        view.change(key, Value::from(to.round() as i64), cx)
                    }))
                })
        };
        div()
            .flex()
            .items_center()
            .rounded(px(7.))
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
            .child(step(
                "smaller",
                "−",
                size - 1.0,
                size > *UI_FONT_SIZES.start(),
            ))
            .child(
                div()
                    .w(px(44.))
                    .text_center()
                    .font(tokens::code_font())
                    .text_size(px(12.))
                    .child(format!("{size}")),
            )
            .child(step("larger", "+", size + 1.0, size < *UI_FONT_SIZES.end()))
    }

    fn render_appearance(&self, cx: &mut Context<Self>) -> Div {
        let desktop = Desktop::from_value(&self.desktop);
        let theme = self.choices("theme", desktop.theme, &ThemeMode::ALL, cx);
        let density = self.choices("density", desktop.density, &Density::ALL, cx);
        let view_mode = self.choices("view_mode", desktop.view_mode, &ViewMode::ALL, cx);
        let ui_size = self.size_control("ui_font_size", desktop.ui_font_size, cx);
        let code_size = self.size_control("code_font_size", desktop.code_font_size, cx);
        let fonts = format!(
            "{} · {} · {}, กับ {} และ {} สำหรับภาษาไทย",
            tokens::UI_FAMILY,
            tokens::ANSWER_FAMILY,
            tokens::CODE_FAMILY,
            tokens::THAI_SANS,
            tokens::THAI_SERIF
        );
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(row(cx, "ธีม", None, theme.into_any_element()))
            .child(row(
                cx,
                "ขนาดตัวอักษร",
                Some("หน้าจอ"),
                ui_size.into_any_element(),
            ))
            .child(row(
                cx,
                "ขนาดโค้ด",
                Some("editor, tool และ terminal"),
                code_size.into_any_element(),
            ))
            .child(row(cx, "ความแน่น", None, density.into_any_element()))
            .child(row(
                cx,
                "โหมดดูเริ่มต้น",
                Some("ของหน้า agent"),
                view_mode.into_any_element(),
            ))
            .child(row(
                cx,
                "ฟอนต์",
                Some("ฝังมากับแอป"),
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().colors().text_muted)
                    .child(fonts)
                    .into_any_element(),
            ))
    }

    fn render_cedia_section(&self, name: &'static str, cx: &mut Context<Self>) -> Div {
        div()
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(cx.theme().colors().text_muted)
            .child(format!(
                "{name} เป็น key ของ cedia และจะแก้ผ่าน schema ของ cedia ซึ่งยังไม่ได้ต่อเข้ามา \
                 ตอนนี้แก้ได้ใน settings.json โดยตรง"
            ))
    }

    fn nav_item(
        &self,
        section: Section,
        label: &'static str,
        cedia: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = cx.theme().colors();
        let on = self.section == section;
        div()
            .id(label)
            .h(px(30.))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(7.))
            .cursor_pointer()
            .text_size(px(13.))
            .when(on, |item| item.bg(colors.element_selected))
            .hover(|item| item.bg(colors.element_hover))
            .on_click(cx.listener(move |view, _, _, cx| {
                view.section = section;
                cx.notify();
            }))
            .child(div().flex_grow(1.).child(label))
            .when(cedia, |item| {
                item.child(
                    div()
                        .px(px(5.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(colors.border)
                        .font(tokens::code_font())
                        .text_size(px(10.))
                        .text_color(colors.text_muted)
                        .child("cedia"),
                )
            })
    }
}

/// A setting: its name and note on the left, its control on the right.
fn row(
    cx: &Context<SettingsView>,
    label: &'static str,
    note: Option<&'static str>,
    control: AnyElement,
) -> Div {
    let colors = cx.theme().colors();
    div()
        .flex()
        .items_center()
        .gap_4()
        .child(
            div()
                .w(px(140.))
                .flex_none()
                .flex()
                .flex_col()
                .child(div().text_size(px(13.)).child(label))
                .children(note.map(|note| {
                    div()
                        .text_size(px(12.))
                        .text_color(colors.text_muted)
                        .child(note)
                })),
        )
        .child(div().flex_grow(1.).min_w_0().child(control))
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let success = cx.theme().status().success;
        let code = tokens::Tokens::of(cx).code;
        let mut nav = div()
            .w(px(180.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap_px()
            .p_2()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background);
        for (section, label, cedia) in Section::ALL {
            nav = nav.child(self.nav_item(section, label, cedia, cx));
        }
        let (title, body) = match self.section {
            Section::Appearance => ("รูปลักษณ์", self.render_appearance(cx)),
            Section::Bots => ("Bots", self.render_cedia_section("Bots", cx)),
            Section::Models => ("Models", self.render_cedia_section("Models", cx)),
        };
        let form = div()
            .id("settings-body")
            .flex_grow(1.)
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .overflow_y_scroll()
            .child(
                div()
                    .text_size(px(22.))
                    .line_height(px(30.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(body);
        let file = serde_json::to_string_pretty(&serde_json::json!({ "desktop": self.desktop }))
            .unwrap_or_default();
        let marker = self.changed.map(|key| format!("\"{key}\":"));
        let path: SharedString =
            self.path.as_ref().map_or("กำลังอ่าน settings".into(), |p| {
                p.display().to_string().into()
            });
        let lines = file.lines().map(|line| {
            let changed = marker
                .as_deref()
                .is_some_and(|marker| line.trim_start().starts_with(marker));
            div()
                .px_2()
                .whitespace_nowrap()
                .when(changed, |l| {
                    l.bg(success.opacity(0.12))
                        .border_l_2()
                        .border_color(colors.text_accent)
                })
                .child(line.to_owned())
        });
        let file_column = div()
            .id("settings-file")
            .w(px(280.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_l_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .overflow_y_scroll()
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.text_muted)
                    .child("ไฟล์ที่เขียน"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(colors.text_muted)
                    .child(path),
            )
            .child(
                div()
                    .py_1()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(code)
                    .font(tokens::code_font())
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .children(lines),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(colors.text_muted)
                    .child("แต่ละการเปลี่ยนเขียนลงไฟล์ทันที key อื่นของ cedia อยู่เหมือนเดิม"),
            );
        div()
            .size_full()
            .flex()
            .text_color(colors.text)
            .bg(colors.background)
            .child(nav)
            .child(form)
            .child(file_column)
    }
}
