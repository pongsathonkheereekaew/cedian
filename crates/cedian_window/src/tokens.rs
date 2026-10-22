//! The Warm & Lime tokens Zed's theme has no key for (Interface, Color; the tints join them
//! with the state rows), and the faces the
//! window draws text in (Interface, Type). The rest come from the cedian themes.

use gpui::{App, Font, FontFallbacks, FontWeight, Hsla};
use theme::{ActiveTheme as _, Appearance};

pub struct Tokens {
    /// The tab strip in the title bar, darker than the app.
    pub frame: Hsla,
    pub user_bubble: Hsla,
    /// Code blocks and tool output.
    pub code: Hsla,
    /// Search, chips and inline code.
    pub field: Hsla,
}

impl Tokens {
    pub fn of(cx: &App) -> Self {
        let hex = |dark: u32, light: u32| -> Hsla {
            let value = match cx.theme().appearance() {
                Appearance::Dark => dark,
                Appearance::Light => light,
            };
            gpui::rgb(value).into()
        };
        Self {
            frame: hex(0x151412, 0xE9E6DF),
            user_bubble: hex(0x32302C, 0xF0EDE6),
            code: hex(0x1E1D1B, 0xF7F5F0),
            field: hex(0x262522, 0xE9E6DF),
        }
    }
}

pub const UI_FAMILY: &str = "Inter";
pub const ANSWER_FAMILY: &str = "Source Serif 4";
pub const CODE_FAMILY: &str = "Lilex";
pub const THAI_SANS: &str = "Noto Sans Thai";
pub const THAI_SERIF: &str = "Noto Serif Thai";

/// The serif the agent's answers are set in, with its Thai companion.
pub fn answer_font() -> Font {
    Font {
        family: ANSWER_FAMILY.into(),
        fallbacks: Some(FontFallbacks::from_fonts(vec![THAI_SERIF.to_owned()])),
        weight: FontWeight::NORMAL,
        ..Font::default()
    }
}

/// Code, tool output and the terminal, with Thai in the sans.
pub fn code_font() -> Font {
    Font {
        family: CODE_FAMILY.into(),
        fallbacks: Some(FontFallbacks::from_fonts(vec![THAI_SANS.to_owned()])),
        ..Font::default()
    }
}
