use gpui::{App, IntoElement, Rems, RenderOnce, Styled, Window, img};
use theme::{ActiveTheme, Appearance};

/// cedian's mark in its brand colors, the light or dark artwork by the theme.
#[derive(IntoElement)]
pub struct CedianMark {
    size: Rems,
}

impl CedianMark {
    pub fn new(size: Rems) -> Self {
        Self { size }
    }
}

impl RenderOnce for CedianMark {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let path = match cx.theme().appearance() {
            Appearance::Light => "images/cedian_mark_light.svg",
            Appearance::Dark => "images/cedian_mark_dark.svg",
        };
        img(path).flex_none().size(self.size)
    }
}
