//! How much of a thread the agent pane shows (Interface, Agent pane): the answers only, the
//! thinking too, or every tool call as its own card. `desktop.view_mode` keeps the choice.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    /// Answers, with each run of tool calls folded into one chip; thinking hidden.
    #[default]
    Answers,
    /// Answers and thinking, tools still folded.
    Thinking,
    /// Every block, each tool call a card.
    All,
}

impl ViewMode {
    /// Each mode, its `desktop.view_mode` value and the word on its chip.
    pub const ALL: [(ViewMode, &'static str, &'static str); 3] = [
        (ViewMode::Answers, "answers", "คำตอบ"),
        (ViewMode::Thinking, "thinking", "+ ความคิด"),
        (ViewMode::All, "all", "ทุก tool"),
    ];

    /// Reads `desktop.view_mode`; anything else is the default.
    pub fn from_key(key: Option<&str>) -> Self {
        Self::ALL
            .iter()
            .find(|(_, k, _)| Some(*k) == key)
            .map_or_else(Self::default, |(mode, _, _)| *mode)
    }

    pub fn key(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(mode, _, _)| *mode == self)
            .map_or("answers", |(_, key, _)| key)
    }

    pub fn shows_thinking(self) -> bool {
        self != ViewMode::Answers
    }

    /// Tool calls fold into one chip per run.
    pub fn folds_tools(self) -> bool {
        self != ViewMode::All
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_and_unknown_is_answers() {
        for (mode, key, _) in ViewMode::ALL {
            assert_eq!(ViewMode::from_key(Some(key)), mode);
            assert_eq!(mode.key(), key);
        }
        assert_eq!(ViewMode::from_key(Some("everything")), ViewMode::Answers);
        assert_eq!(ViewMode::from_key(None), ViewMode::Answers);
    }
}
