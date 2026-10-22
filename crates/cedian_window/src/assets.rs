//! cedian's own assets laid over Zed's: the bundled fonts (Interface, Type) and the cedian
//! themes. A path cedian has is served from here; any other goes to Zed's assets.

use std::borrow::Cow;

use anyhow::Context as _;
use gpui::{App, AssetSource, SharedString};

/// Every file under `crates/cedian_window/assets`, embedded so the binary needs no files.
const OWN: &[(&str, &[u8])] = &[
    (
        "fonts/inter/Inter-Bold.ttf",
        include_bytes!("../assets/fonts/inter/Inter-Bold.ttf"),
    ),
    (
        "fonts/inter/Inter-Medium.ttf",
        include_bytes!("../assets/fonts/inter/Inter-Medium.ttf"),
    ),
    (
        "fonts/inter/Inter-Regular.ttf",
        include_bytes!("../assets/fonts/inter/Inter-Regular.ttf"),
    ),
    (
        "fonts/inter/Inter-SemiBold.ttf",
        include_bytes!("../assets/fonts/inter/Inter-SemiBold.ttf"),
    ),
    (
        "fonts/noto-sans-thai/NotoSansThai-Bold.ttf",
        include_bytes!("../assets/fonts/noto-sans-thai/NotoSansThai-Bold.ttf"),
    ),
    (
        "fonts/noto-sans-thai/NotoSansThai-Medium.ttf",
        include_bytes!("../assets/fonts/noto-sans-thai/NotoSansThai-Medium.ttf"),
    ),
    (
        "fonts/noto-sans-thai/NotoSansThai-Regular.ttf",
        include_bytes!("../assets/fonts/noto-sans-thai/NotoSansThai-Regular.ttf"),
    ),
    (
        "fonts/noto-sans-thai/NotoSansThai-SemiBold.ttf",
        include_bytes!("../assets/fonts/noto-sans-thai/NotoSansThai-SemiBold.ttf"),
    ),
    (
        "fonts/noto-serif-thai/NotoSerifThai-Bold.ttf",
        include_bytes!("../assets/fonts/noto-serif-thai/NotoSerifThai-Bold.ttf"),
    ),
    (
        "fonts/noto-serif-thai/NotoSerifThai-Regular.ttf",
        include_bytes!("../assets/fonts/noto-serif-thai/NotoSerifThai-Regular.ttf"),
    ),
    (
        "fonts/noto-serif-thai/NotoSerifThai-SemiBold.ttf",
        include_bytes!("../assets/fonts/noto-serif-thai/NotoSerifThai-SemiBold.ttf"),
    ),
    (
        "fonts/source-serif-4/SourceSerif4-Bold.ttf",
        include_bytes!("../assets/fonts/source-serif-4/SourceSerif4-Bold.ttf"),
    ),
    (
        "fonts/source-serif-4/SourceSerif4-Regular.ttf",
        include_bytes!("../assets/fonts/source-serif-4/SourceSerif4-Regular.ttf"),
    ),
    (
        "fonts/source-serif-4/SourceSerif4-SemiBold.ttf",
        include_bytes!("../assets/fonts/source-serif-4/SourceSerif4-SemiBold.ttf"),
    ),
    (
        "themes/cedian.json",
        include_bytes!("../assets/themes/cedian.json"),
    ),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        match OWN.iter().find(|(own, _)| *own == path) {
            Some((_, bytes)) => Ok(Some(Cow::Borrowed(bytes))),
            None => ::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> = OWN
            .iter()
            .filter(|(own, _)| own.starts_with(path))
            .map(|(own, _)| SharedString::from(*own))
            .collect();
        paths.extend(::assets::Assets.list(path)?);
        Ok(paths)
    }
}

impl Assets {
    /// Gives GPUI's text system every bundled font, cedian's and Zed's (Lilex, IBM Plex), so
    /// the interface never depends on what the system has installed.
    pub fn load_fonts(&self, cx: &App) -> anyhow::Result<()> {
        let mut fonts = Vec::new();
        for path in self.list("fonts/")? {
            if path.ends_with(".ttf") {
                fonts.push(
                    self.load(&path)?
                        .with_context(|| format!("no font at {path}"))?,
                );
            }
        }
        cx.text_system().add_fonts(fonts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cedian_themes_parse_as_zed_themes() {
        let bytes = Assets
            .load("themes/cedian.json")
            .expect("load")
            .expect("bundled");
        let family = theme_settings::deserialize_user_theme(&bytes).expect("a Zed theme family");
        let names: Vec<_> = family.themes.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["cedian Dark", "cedian Light"]);
    }

    #[test]
    fn every_bundled_face_is_listed() {
        let fonts = Assets.list("fonts/").expect("list");
        for face in [
            "fonts/inter/Inter-Regular.ttf",
            "fonts/noto-sans-thai/NotoSansThai-Regular.ttf",
            "fonts/source-serif-4/SourceSerif4-Regular.ttf",
            "fonts/noto-serif-thai/NotoSerifThai-Regular.ttf",
            "fonts/lilex/Lilex-Regular.ttf",
        ] {
            assert!(fonts.iter().any(|f| f == face), "{face} missing");
        }
    }
}
