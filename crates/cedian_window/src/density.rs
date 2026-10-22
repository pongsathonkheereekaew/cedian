//! How tightly the window packs its rows (Interface, Settings window: Appearance), from
//! `desktop.density`. Comfortable is the default; compact trims the space between blocks.

use gpui::{Pixels, px};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

impl Density {
    /// Each density, its `desktop.density` value and its word in Settings.
    pub const ALL: [(Density, &'static str, &'static str); 2] = [
        (Density::Comfortable, "comfortable", "สบายตา"),
        (Density::Compact, "compact", "กะทัดรัด"),
    ];

    pub fn from_key(key: Option<&str>) -> Self {
        Self::ALL
            .iter()
            .find(|(_, k, _)| Some(*k) == key)
            .map_or_else(Self::default, |(density, _, _)| *density)
    }

    /// The space above and below a block of the agent pane.
    pub fn block_gap(self) -> Pixels {
        match self {
            Density::Comfortable => px(6.),
            Density::Compact => px(2.),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_is_comfortable() {
        assert_eq!(Density::from_key(Some("compact")), Density::Compact);
        assert_eq!(Density::from_key(Some("roomy")), Density::Comfortable);
        assert_eq!(Density::from_key(None), Density::Comfortable);
    }
}
