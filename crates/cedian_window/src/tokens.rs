//! The Warm & Lime tokens Zed's theme has no key for (Interface, Color; the tints join them
//! with the state rows), and the faces the
//! window draws text in (Interface, Type). The rest come from the cedian themes.

use std::sync::Arc;

use gpui::{App, Font, FontFallbacks, FontFeatures, FontWeight, Hsla};
use theme::{ActiveTheme as _, Appearance};

pub struct Tokens {
    /// The tab strip in the title bar, darker than the app.
    pub frame: Hsla,
    pub user_bubble: Hsla,
    /// Code blocks and tool output.
    pub code: Hsla,
    /// Search, chips and inline code.
    pub field: Hsla,
    /// Diff additions: green, so lime stays for what is primary or verified.
    pub diff_add: Hsla,
    /// The primary button (and the send button): its fill and its text.
    pub primary: Hsla,
    pub on_primary: Hsla,
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
            frame: hex(0x151412, 0xEAEAE6),
            user_bubble: hex(0x32302C, 0xEFEFEB),
            code: hex(0x1E1D1B, 0xF7F7F5),
            field: hex(0x262522, 0xECECE8),
            diff_add: hex(0x7BD88F, 0x2F7D3F),
            primary: hex(0xC6F432, 0x1A1C1E),
            on_primary: hex(0x151412, 0xFFFFFF),
        }
    }
}

pub const UI_FAMILY: &str = "Inter";
pub const ANSWER_FAMILY: &str = "Source Serif 4";
pub const CODE_FAMILY: &str = "Lilex";
/// The bundled Thai faces, used where the system has no fixed Thai face (Linux). On macOS
/// Thai is set in the system's face (Thonburi) through Core Text's fallback, as in Claude,
/// so no Thai face is named there (Interface, Type).
pub const THAI_SANS: &str = "Noto Sans Thai";
pub const THAI_SERIF: &str = "Noto Serif Thai";

/// The Thai fallback beside the sans and mono faces.
pub fn thai_sans() -> Vec<String> {
    if cfg!(target_os = "macos") {
        Vec::new()
    } else {
        vec![THAI_SANS.to_owned()]
    }
}

/// The Thai fallback beside the serif.
pub fn thai_serif() -> Vec<String> {
    if cfg!(target_os = "macos") {
        Vec::new()
    } else {
        vec![THAI_SERIF.to_owned()]
    }
}

fn fallbacks(fonts: Vec<String>) -> Option<FontFallbacks> {
    (!fonts.is_empty()).then(|| FontFallbacks::from_fonts(fonts))
}

/// The serif the agent's answers are set in.
pub fn answer_font() -> Font {
    Font {
        family: ANSWER_FAMILY.into(),
        fallbacks: fallbacks(thai_serif()),
        weight: FontWeight::NORMAL,
        ..Font::default()
    }
}

/// Code, paths, branches and commands.
pub fn code_font() -> Font {
    Font {
        family: CODE_FAMILY.into(),
        fallbacks: fallbacks(thai_sans()),
        ..Font::default()
    }
}

/// Numbers in interface text (costs, counts, times): Inter with its tabular digits, so they
/// line up without the mono face.
pub fn numeric_font() -> Font {
    Font {
        family: UI_FAMILY.into(),
        features: FontFeatures(Arc::new(vec![("tnum".to_owned(), 1)])),
        fallbacks: fallbacks(thai_sans()),
        ..Font::default()
    }
}
