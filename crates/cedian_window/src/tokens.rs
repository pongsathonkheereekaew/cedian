//! The Ink & Lime tokens Zed's theme has no key for (Interface, Color; `frame` and the tints
//! join them with the tab strip and the state rows), and the faces the
//! window draws text in (Interface, Type). The rest come from the cedian themes.

use gpui::{App, Font, FontFallbacks, FontWeight, Hsla};
use theme::{ActiveTheme as _, Appearance};

pub struct Tokens {
    pub user_bubble: Hsla,
    /// Code blocks and tool output.
    pub code: Hsla,
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
            user_bubble: hex(0x1A1F26, 0xF0F2ED),
            code: hex(0x0D1013, 0xFCFCFA),
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
