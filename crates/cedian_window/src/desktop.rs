//! The `desktop` settings the window applies: theme and font sizes. cedia's settings file is
//! the only place they live; the daemon reads and writes it, and the window applies what it
//! is sent.

use gpui::{App, BorrowAppContext as _};
use serde_json::Value;
use settings::SettingsStore;

use crate::{density::Density, tokens, view_mode::ViewMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub const ALL: [(ThemeMode, &'static str, &'static str); 3] = [
        (ThemeMode::Dark, "dark", "มืด"),
        (ThemeMode::Light, "light", "สว่าง"),
        (ThemeMode::System, "system", "ตามระบบ"),
    ];
}

#[derive(Clone, Debug, PartialEq)]
pub struct Desktop {
    pub theme: ThemeMode,
    pub ui_font_size: f32,
    pub code_font_size: f32,
    pub view_mode: ViewMode,
    pub density: Density,
}

pub const UI_FONT_SIZES: std::ops::RangeInclusive<f32> = 12.0..=24.0;

impl Desktop {
    /// Reads what it knows from `desktop`; a missing or wrong value takes its default.
    pub fn from_value(desktop: &Value) -> Self {
        let theme = match desktop.get("theme").and_then(Value::as_str) {
            Some("light") => ThemeMode::Light,
            Some("system") => ThemeMode::System,
            // Dark until the person picks another: the owner's default.
            _ => ThemeMode::Dark,
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
            view_mode: ViewMode::from_key(desktop.get("view_mode").and_then(Value::as_str)),
            density: Density::from_key(desktop.get("density").and_then(Value::as_str)),
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
            "theme": {"mode": mode, "light": "cedian Light", "dark": "cedian Dark"},
            "ui_font_family": tokens::UI_FAMILY,
            "ui_font_fallbacks": [tokens::THAI_SANS],
            "ui_font_size": self.ui_font_size,
            "buffer_font_family": tokens::CODE_FAMILY,
            "buffer_font_fallbacks": [tokens::THAI_SANS],
            "buffer_font_size": self.code_font_size,
        })
        .to_string();
        cx.update_global::<SettingsStore, _>(|store, cx| {
            let result = store.set_user_settings(&content, cx);
            if !matches!(result.parse_status, settings::ParseStatus::Success) {
                log::warn!("desktop settings: {:?}", result.parse_status);
            }
        });
        // theme_settings reloads the theme when the store changes, but its observer only
        // starts on the next cycle, which the settings applied at launch come before.
        theme_settings::reload_theme(cx);
    }
}

/// `desktop` as the file holds it now, read once at launch so the first frame already has the
/// theme and the layout; afterwards the daemon sends it. A missing or bad file reads as empty.
pub fn read_at_launch() -> Value {
    let path = cedian_index::cedia_home().join("settings.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!("{}: {error}", path.display());
            }
            return Value::Object(Default::default());
        }
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(mut file) => file
            .get_mut("desktop")
            .map(Value::take)
            .filter(Value::is_object)
            .unwrap_or_else(|| Value::Object(Default::default())),
        Err(error) => {
            log::warn!("{}: {error}", path.display());
            Value::Object(Default::default())
        }
    }
}
