//! The `desktop` settings the window applies: theme and font sizes. cedia's settings file is
//! the only place they live; the daemon reads and writes it, and the window applies what it
//! is sent.

use gpui::{App, BorrowAppContext as _};
use serde_json::Value;
use settings::SettingsStore;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub const ALL: [(ThemeMode, &'static str, &'static str); 3] = [
        (ThemeMode::System, "system", "Follow the system"),
        (ThemeMode::Light, "light", "Light"),
        (ThemeMode::Dark, "dark", "Dark"),
    ];
}

#[derive(Clone, Debug, PartialEq)]
pub struct Desktop {
    pub theme: ThemeMode,
    pub ui_font_size: f32,
    pub code_font_size: f32,
}

pub const UI_FONT_SIZES: std::ops::RangeInclusive<f32> = 12.0..=24.0;

impl Desktop {
    /// Reads what it knows from `desktop`; a missing or wrong value takes its default.
    pub fn from_value(desktop: &Value) -> Self {
        let theme = match desktop.get("theme").and_then(Value::as_str) {
            Some("light") => ThemeMode::Light,
            Some("dark") => ThemeMode::Dark,
            _ => ThemeMode::System,
        };
        let size = |key: &str, default: f32| {
            desktop
                .get(key)
                .and_then(Value::as_f64)
                .map(|n| (n as f32).clamp(*UI_FONT_SIZES.start(), *UI_FONT_SIZES.end()))
                .unwrap_or(default)
        };
        Self {
            theme,
            ui_font_size: size("ui_font_size", 16.0),
            code_font_size: size("code_font_size", 14.0),
        }
    }

    /// Puts the settings into Zed's settings store, in memory: the theme and fonts follow, and
    /// Zed's own settings file is never read or written.
    pub fn apply(&self, cx: &mut App) {
        let mode = match self.theme {
            ThemeMode::System => "system",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        };
        let content = serde_json::json!({
            "theme": {"mode": mode, "light": "One Light", "dark": "One Dark"},
            "ui_font_size": self.ui_font_size,
            "buffer_font_size": self.code_font_size,
        })
        .to_string();
        cx.update_global::<SettingsStore, _>(|store, cx| {
            let result = store.set_user_settings(&content, cx);
            if !matches!(result.parse_status, settings::ParseStatus::Success) {
                log::warn!("desktop settings: {:?}", result.parse_status);
            }
        });
    }
}
