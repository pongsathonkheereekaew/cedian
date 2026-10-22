//! Settings as its own window (`⌘,`), as on any Mac app. It shows the main window's
//! `SettingsView`, so the daemon's answers reach it through the one connection.

use gpui::{
    App, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, div, px, size,
};
use settings::Settings as _;
use theme::ActiveTheme as _;

use crate::settings_view::SettingsView;

pub struct SettingsWindow {
    view: Entity<SettingsView>,
}

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
            titlebar: Some(TitlebarOptions {
                title: Some("Settings".into()),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(760.), px(560.)),
                cx,
            ))),
            window_min_size: Some(size(px(560.), px(400.))),
            ..Default::default()
        };
        match cx.open_window(options, |_, cx| cx.new(|_| SettingsWindow { view })) {
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
        div()
            .size_full()
            .font(theme.ui_font.clone())
            .bg(cx.theme().colors().background)
            .child(self.view.clone())
    }
}
