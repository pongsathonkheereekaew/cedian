//! Settings as its own window (`⌘,`), as on any Mac app. It shows the main window's
//! `SettingsView`, so the daemon's answers reach it through the one connection.

use gpui::{
    App, AppContext as _, Bounds, Context, Entity, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    TitlebarOptions, Window, WindowBounds, WindowControlArea, WindowHandle, WindowOptions, div,
    point, prelude::FluentBuilder as _, px, size,
};
use settings::Settings as _;
use theme::ActiveTheme as _;

use crate::type_scale::TypeScale as _;
use crate::{settings_view::SettingsView, tokens::Tokens};

pub struct SettingsWindow {
    view: Entity<SettingsView>,
    /// A press on the title bar that moves the window once the mouse moves.
    should_move: bool,
}

const TITLE_BAR: gpui::Pixels = px(32.);

impl SettingsWindow {
    /// Brings the open settings window forward, or opens one.
    pub fn show(
        view: Entity<SettingsView>,
        open: Option<WindowHandle<SettingsWindow>>,
        cx: &mut App,
    ) -> Option<WindowHandle<SettingsWindow>> {
        if let Some(handle) = open
            && handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            return Some(handle);
        }
        let options = WindowOptions {
            // A dark bar of the app's own, like the main window's strip, instead of the
            // system's light title bar.
            titlebar: Some(TitlebarOptions {
                title: Some("Settings".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.), px(10.))),
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(920.), px(600.)),
                cx,
            ))),
            window_min_size: Some(size(px(720.), px(460.))),
            ..Default::default()
        };
        match cx.open_window(options, |_, cx| {
            cx.new(|_| SettingsWindow {
                view,
                should_move: false,
            })
        }) {
            Ok(handle) => Some(handle),
            Err(error) => {
                log::error!("opening the settings window: {error:#}");
                None
            }
        }
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_settings::ThemeSettings::get_global(cx);
        window.set_rem_size(theme.ui_font_size(cx));
        let colors = cx.theme().colors();
        let title_bar = div()
            .id("settings-title-bar")
            .flex_none()
            .h(TITLE_BAR)
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(Tokens::of(cx).frame)
            .text_ui()
            .text_color(colors.text_muted)
            .child("Settings")
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.should_move = true),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.should_move = false),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if std::mem::take(&mut this.should_move) {
                    window.start_window_move();
                }
            }))
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            });
        div()
            .size_full()
            .flex()
            .flex_col()
            .font(theme.ui_font.clone())
            .bg(colors.background)
            .text_color(colors.text)
            .when(cfg!(target_os = "macos"), |window| window.child(title_bar))
            .child(div().flex_grow(1.).min_h_0().child(self.view.clone()))
    }
}
