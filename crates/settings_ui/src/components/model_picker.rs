use std::sync::Arc;

use gpui::{AnyElement, App, Context, DismissEvent, SharedString, Task, Window, px};
use picker::{Picker, PickerDelegate};
use ui::{ListItem, ListItemSpacing, PopoverMenu, prelude::*};

use crate::render_picker_trigger_button;

/// One model the agent can run, as the shared model picker lists it
/// (Settings > Models now, Methodology's pstack roles next).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelChoice {
    /// `provider/model`, the value a role is set to.
    pub key: SharedString,
    pub name: SharedString,
    pub provider: SharedString,
}

/// `choices` grouped by provider, then by name, as the picker lists them.
pub fn grouped(mut choices: Vec<ModelChoice>) -> Vec<ModelChoice> {
    choices.sort_by(|a, b| a.provider.cmp(&b.provider).then(a.name.cmp(&b.name)));
    choices
}

/// The choices whose name, provider or key contain `query`, ignoring case.
pub fn filter(choices: &[ModelChoice], query: &str) -> Vec<usize> {
    let query = query.to_lowercase();
    choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| {
            query.is_empty()
                || [&choice.name, &choice.provider, &choice.key]
                    .iter()
                    .any(|text| text.to_lowercase().contains(&query))
        })
        .map(|(index, _)| index)
        .collect()
}

type ModelPicker = Picker<ModelPickerDelegate>;

struct ModelPickerDelegate {
    choices: Arc<[ModelChoice]>,
    matches: Vec<usize>,
    selected: usize,
    on_pick: Arc<dyn Fn(SharedString, &mut Window, &mut App)>,
}

impl PickerDelegate for ModelPickerDelegate {
    type ListItem = AnyElement;

    fn name() -> &'static str {
        "cedian model picker"
    }

    fn match_count(&self) -> usize {
        self.matches.len()
    }

    fn selected_index(&self) -> usize {
        self.selected
    }

    fn set_selected_index(&mut self, index: usize, _: &mut Window, cx: &mut Context<ModelPicker>) {
        self.selected = index.min(self.matches.len().saturating_sub(1));
        cx.notify();
    }

    fn placeholder_text(&self, _: &mut Window, _: &mut App) -> Arc<str> {
        "Search models…".into()
    }

    fn no_matches_text(&self, _: &mut Window, _: &mut App) -> Option<SharedString> {
        Some(if self.choices.is_empty() {
            "No models yet. Sign in to a provider on Account.".into()
        } else {
            "No model matches.".into()
        })
    }

    fn update_matches(
        &mut self,
        query: String,
        _: &mut Window,
        cx: &mut Context<ModelPicker>,
    ) -> Task<()> {
        self.matches = filter(&self.choices, &query);
        self.selected = 0;
        cx.notify();
        Task::ready(())
    }

    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ModelPicker>) {
        let Some(choice) = self
            .matches
            .get(self.selected)
            .and_then(|index| self.choices.get(*index))
        else {
            return;
        };
        (self.on_pick)(choice.key.clone(), window, cx);
        cx.emit(DismissEvent);
    }

    fn dismissed(&mut self, _: &mut Window, cx: &mut Context<ModelPicker>) {
        cx.emit(DismissEvent);
    }

    fn render_match(
        &self,
        index: usize,
        selected: bool,
        _: &mut Window,
        _: &mut Context<ModelPicker>,
    ) -> Option<Self::ListItem> {
        let choice = self.choices.get(*self.matches.get(index)?)?;
        Some(
            ListItem::new(index)
                .inset(true)
                .spacing(ListItemSpacing::Sparse)
                .toggle_state(selected)
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .gap_2()
                        .child(Label::new(choice.name.clone()))
                        .child(
                            Label::new(choice.provider.clone())
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// The shared model picker: a button showing `current`, opening a searchable
/// list of `choices` grouped by provider.
pub fn model_picker(
    id: impl Into<SharedString>,
    choices: Arc<[ModelChoice]>,
    current: Option<SharedString>,
    on_pick: impl Fn(SharedString, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let id = id.into();
    let label = current
        .as_ref()
        .and_then(|key| choices.iter().find(|choice| choice.key == *key))
        .map(|choice| choice.name.clone())
        .or(current)
        .unwrap_or_else(|| "Choose a model…".into());
    let on_pick: Arc<dyn Fn(SharedString, &mut Window, &mut App)> = Arc::new(on_pick);
    PopoverMenu::new(id.clone())
        .trigger(render_picker_trigger_button(id, label))
        .menu(move |window, cx| {
            let delegate = ModelPickerDelegate {
                matches: filter(&choices, ""),
                choices: choices.clone(),
                selected: 0,
                on_pick: on_pick.clone(),
            };
            Some(cx.new(|cx| {
                Picker::uniform_list(delegate, window, cx)
                    .show_scrollbar(true)
                    .initial_width(rems_from_px(320_f32))
                    .max_height(rems(20.))
                    .popover()
            }))
        })
        .anchor(gpui::Anchor::TopRight)
        .offset(gpui::Point {
            x: px(0.0),
            y: px(2.0),
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(provider: &'static str, name: &'static str) -> ModelChoice {
        ModelChoice {
            key: format!("{provider}/{name}").into(),
            name: name.into(),
            provider: provider.into(),
        }
    }

    #[test]
    fn models_group_by_provider_and_search_name_provider_or_key() {
        let choices = grouped(vec![
            choice("openai", "gpt-6"),
            choice("anthropic", "opus"),
            choice("anthropic", "haiku"),
        ]);
        let keys: Vec<_> = choices.iter().map(|c| c.key.as_ref()).collect();
        assert_eq!(keys, ["anthropic/haiku", "anthropic/opus", "openai/gpt-6"]);
        assert_eq!(filter(&choices, ""), [0, 1, 2]);
        assert_eq!(filter(&choices, "ANTHROP"), [0, 1]);
        assert_eq!(filter(&choices, "gpt"), [2]);
        assert_eq!(filter(&choices, "openai/"), [2]);
        assert!(filter(&choices, "llama").is_empty());
    }
}
