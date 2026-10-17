//! Settings > Models (SET-2): the default model, the default thinking level
//! and the model roles, written to the agent's config through its own CLI as
//! one typed `modelRoles` record (ADR-0045 decision 3).

use crate::omp_settings::OmpSettings;
use crate::panel::{CedianPanel, Connection};
use cedian_omp::model_roles::{ModelRoles, RoleModel, Thinking};
use editor::Editor;
use gpui::{AnyWindowHandle, App, Context, Entity, Render, Subscription, Window};
use settings_ui::{
    ModelChoice, RowState, ScopeBadge, SettingsSectionHeader, group_models, model_picker,
    render_row, render_row_state,
};
use std::sync::Arc;
use ui::{
    Button, ButtonStyle, ContextMenu, Divider, DropdownMenu, IconButton, IconName, Label,
    prelude::*,
};

/// Why a role's model cannot run, when it cannot: its provider offers no
/// model the agent can use, so the person signs in to it on Account.
pub fn role_state(model: Option<&RoleModel>, choices: &[ModelChoice]) -> Option<RowState> {
    let model = model?;
    if choices.is_empty() || choices.iter().any(|choice| choice.key == model.model) {
        return None;
    }
    let provider = model.provider();
    Some(
        if choices.iter().any(|choice| choice.provider == provider) {
            RowState::Unavailable(format!("{} is not offered by {provider}", model.model).into())
        } else {
            RowState::Unavailable(format!("Sign in to {provider}").into())
        },
    )
}

/// The Default Model row's state: the agent must run for the roles to load,
/// so a stopped agent says so instead of loading forever. While it is still
/// starting, show Loading — not a Start button that would relaunch it.
pub fn default_model_state(
    running: bool,
    starting: bool,
    loaded: bool,
    model: Option<&RoleModel>,
    choices: &[ModelChoice],
) -> Option<RowState> {
    if starting {
        Some(RowState::Loading)
    } else if !running {
        Some(RowState::NotRunning("Agent not running".into()))
    } else if !loaded {
        Some(RowState::Loading)
    } else {
        role_state(model, choices)
    }
}

/// A role as its row names it, in the Title Case of the other rows.
pub fn role_title(role: &str) -> String {
    let mut chars = role.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// A role name a person may add: new, and letters, digits, `-` or `_`.
pub fn check_new_role(name: &str, roles: &ModelRoles) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Name the role.".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("A role name has only letters, digits, - and _.".into());
    }
    if roles.rows().iter().any(|role| role == name) {
        return Err(format!("{name} is already a role."));
    }
    Ok(name.to_string())
}

pub struct ModelsPage {
    panel: Entity<CedianPanel>,
    workspace_window: AnyWindowHandle,
    settings: Entity<OmpSettings>,
    new_role: Entity<Editor>,
    /// Roles added here that have no model yet: they are written when one
    /// is picked.
    pending_roles: Vec<String>,
    role_error: Option<String>,
    _observe: [Subscription; 2],
}

impl ModelsPage {
    pub fn new(
        panel: Entity<CedianPanel>,
        workspace_window: AnyWindowHandle,
        settings: Entity<OmpSettings>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        panel.read(cx).refresh_models();
        let new_role = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("review", window, cx);
            editor
        });
        Self {
            _observe: [
                cx.observe(&panel, |_, _, cx| cx.notify()),
                cx.observe(&settings, |_, _, cx| cx.notify()),
            ],
            panel,
            workspace_window,
            settings,
            new_role,
            pending_roles: Vec::new(),
            role_error: None,
        }
    }

    pub fn choices(&self, cx: &App) -> Arc<[ModelChoice]> {
        group_models(
            self.panel
                .read(cx)
                .picker()
                .models
                .iter()
                .map(|model| ModelChoice {
                    key: crate::omp_link::model_key(model).into(),
                    name: model.name.clone().into(),
                    provider: model.provider.clone().into(),
                })
                .collect(),
        )
        .into()
    }

    pub fn pending_roles(&self) -> &[String] {
        &self.pending_roles
    }

    pub fn role_error(&self) -> Option<&str> {
        self.role_error.as_deref()
    }

    /// Point `role` at `model`, keeping its thinking level.
    pub fn pick(&mut self, role: &str, model: &str, cx: &mut Context<Self>) {
        self.pending_roles.retain(|pending| pending != role);
        self.write(cx, |roles| {
            let thinking = roles.get(role).and_then(|current| current.thinking);
            roles.set(
                role,
                RoleModel {
                    model: model.to_string(),
                    thinking,
                },
            );
        });
    }

    /// Pin `role`'s thinking level, or leave it to the default with `None`.
    pub fn set_role_thinking(
        &mut self,
        role: &str,
        thinking: Option<Thinking>,
        cx: &mut Context<Self>,
    ) {
        self.write(cx, |roles| {
            if let Some(current) = roles.get(role).cloned() {
                roles.set(
                    role,
                    RoleModel {
                        thinking,
                        ..current
                    },
                );
            }
        });
    }

    pub fn remove_role(&mut self, role: &str, cx: &mut Context<Self>) {
        self.pending_roles.retain(|pending| pending != role);
        if self.settings.read(cx).roles().get(role).is_some() {
            self.write(cx, |roles| roles.remove(role));
        }
        cx.notify();
    }

    pub fn add_role(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.new_role.read(cx).text(cx);
        let mut roles = self.settings.read(cx).roles();
        for pending in &self.pending_roles {
            roles.set(pending, RoleModel::new(""));
        }
        match check_new_role(&typed, &roles) {
            Ok(role) => {
                self.pending_roles.push(role);
                self.role_error = None;
                self.new_role
                    .update(cx, |editor, cx| editor.clear(window, cx));
            }
            Err(error) => self.role_error = Some(error),
        }
        cx.notify();
    }

    pub fn set_default_thinking(&mut self, level: Thinking, cx: &mut Context<Self>) {
        self.settings
            .update(cx, |settings, cx| settings.set_default_thinking(level, cx));
    }

    fn write(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut ModelRoles)) {
        self.settings.update(cx, |settings, cx| {
            let mut roles = settings.roles();
            change(&mut roles);
            settings.set_roles(&roles, cx);
        });
    }

    fn thinking_menu(
        &self,
        id: SharedString,
        current: Option<Thinking>,
        choices: &'static [Thinking],
        empty_label: Option<&'static str>,
        on_pick: impl Fn(&mut Self, Option<Thinking>, &mut Context<Self>) + Clone + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let this = cx.entity().downgrade();
        let label = current.map_or(empty_label.unwrap_or(""), Thinking::label);
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            let options = empty_label
                .map(|label| (label, None))
                .into_iter()
                .chain(choices.iter().map(|level| (level.label(), Some(*level))));
            for (label, level) in options {
                let this = this.clone();
                let on_pick = on_pick.clone();
                menu = menu.toggleable_entry(
                    label,
                    current == level,
                    IconPosition::Start,
                    None,
                    move |_, cx| {
                        this.update(cx, |this, cx| on_pick(this, level, cx)).ok();
                    },
                );
            }
            menu
        });
        DropdownMenu::new(id, label, menu)
            .style(ui::DropdownStyle::Outlined)
            .into_any_element()
    }

    fn role_row(
        &self,
        role: &str,
        model: Option<&RoleModel>,
        choices: &Arc<[ModelChoice]>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let this = cx.entity().downgrade();
        let picked_role = role.to_string();
        let picker = model_picker(
            SharedString::from(format!("cedian-role-{role}")),
            choices.clone(),
            model.map(|model| model.model.clone().into()),
            move |key, _, cx| {
                let role = picked_role.clone();
                this.update(cx, |this, cx| this.pick(&role, &key, cx)).ok();
            },
        );
        let thinking_role = role.to_string();
        let thinking = self.thinking_menu(
            SharedString::from(format!("cedian-role-thinking-{role}")),
            model.and_then(|model| model.thinking),
            Thinking::ROLE_CHOICES,
            Some("Default"),
            move |this, level, cx| this.set_role_thinking(&thinking_role, level, cx),
            window,
            cx,
        );
        let removed_role = role.to_string();
        let remove = IconButton::new(
            SharedString::from(format!("cedian-role-remove-{role}")),
            IconName::Trash,
        )
        .icon_size(IconSize::Small)
        .icon_color(Color::Muted)
        .disabled(model.is_none() && !self.pending_roles.iter().any(|pending| pending == role))
        .tooltip(ui::Tooltip::text("Remove"))
        .on_click(cx.listener(move |this, _, _, cx| this.remove_role(&removed_role, cx)));
        let state = role_state(model, choices);
        let control = h_flex()
            .gap_2()
            .when(
                matches!(&state, Some(RowState::Unavailable(text)) if text.starts_with("Sign in")),
                |this| this.child(open_account_button(role)),
            )
            .child(picker)
            .child(thinking)
            .child(remove)
            .into_any_element();
        render_row(
            SharedString::from(format!("cedian-role-row-{role}")),
            role_title(role).into(),
            None,
            None,
            state.as_ref(),
            control,
            cx,
        )
        .into_any_element()
    }
}

fn open_account_button(id: &str) -> AnyElement {
    Button::new(
        SharedString::from(format!("cedian-sign-in-{id}")),
        "Account",
    )
    .style(ButtonStyle::Subtle)
    .end_icon(Icon::new(IconName::ArrowUpRight).size(IconSize::Small))
    .on_click(|_, window, cx| {
        window.dispatch_action(
            Box::new(zed_actions::OpenSettingsPage {
                page: "Account".into(),
                target: None,
            }),
            cx,
        )
    })
    .into_any_element()
}

impl Render for ModelsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choices = self.choices(cx);
        let settings = self.settings.read(cx);
        let loaded = settings.setting("modelRoles").is_some();
        let roles = settings.roles();
        let message = settings.message().map(str::to_string);
        let default_thinking = settings.default_thinking();
        let layer = settings
            .setting("modelRoles")
            .map(|setting| setting.layer.label())
            .unwrap_or("global");

        let default_model = roles.get("default").cloned();
        let this = cx.entity().downgrade();
        let default_picker = model_picker(
            "cedian-default-model",
            choices.clone(),
            default_model
                .as_ref()
                .map(|model| model.model.clone().into()),
            move |key, _, cx| {
                this.update(cx, |this, cx| this.pick("default", &key, cx))
                    .ok();
            },
        );
        let connection = self.panel.read(cx).connection();
        let running = matches!(connection, Connection::Ready { .. });
        let starting = matches!(connection, Connection::Starting | Connection::Checking);
        let default_state =
            default_model_state(running, starting, loaded, default_model.as_ref(), &choices);
        let default_control = h_flex()
            .gap_2()
            .when(!running && !starting, |this| {
                this.child(
                    Button::new("cedian-models-start", "Start")
                        .style(ButtonStyle::OutlinedGhost)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let panel = this.panel.clone();
                            this.workspace_window
                                .update(cx, |_, window, cx| {
                                    panel.update(cx, |panel, cx| panel.restart(window, cx))
                                })
                                .ok();
                        })),
                )
            })
            .when(
                matches!(&default_state, Some(RowState::Unavailable(text)) if text.starts_with("Sign in")),
                |this| this.child(open_account_button("default")),
            )
            .child(default_picker)
            .into_any_element();
        let badge = ScopeBadge::Agent(layer);
        let defaults = vec![
            render_row(
                "cedian-default-model-row",
                "Default Model".into(),
                Some("The model new sessions start with.".into()),
                Some(badge),
                default_state.as_ref(),
                default_control,
                cx,
            )
            .into_any_element(),
            render_row(
                "cedian-default-thinking-row",
                "Default Thinking Level".into(),
                Some("How much the model reasons before answering.".into()),
                Some(badge),
                None,
                self.thinking_menu(
                    "cedian-default-thinking".into(),
                    Some(default_thinking.unwrap_or(Thinking::AGENT_DEFAULT)),
                    Thinking::DEFAULT_CHOICES,
                    None,
                    |this, level, cx| {
                        if let Some(level) = level {
                            this.set_default_thinking(level, cx);
                        }
                    },
                    window,
                    cx,
                ),
                cx,
            )
            .into_any_element(),
        ];

        let mut role_names = roles.rows();
        role_names.extend(
            self.pending_roles
                .iter()
                .filter(|pending| !role_names.contains(pending))
                .cloned()
                .collect::<Vec<_>>(),
        );
        let mut role_rows: Vec<AnyElement> = role_names
            .iter()
            .map(|role| self.role_row(role, roles.get(role), &choices, window, cx))
            .collect();
        let add = h_flex()
            .gap_2()
            .child(
                div()
                    .w(px(160.))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .child(self.new_role.clone()),
            )
            .child(
                Button::new("cedian-add-role", "Add Role")
                    .style(ButtonStyle::OutlinedGhost)
                    .on_click(cx.listener(|this, _, window, cx| this.add_role(window, cx))),
            )
            .into_any_element();
        role_rows.push(
            render_row(
                "cedian-add-role-row",
                "Add Custom Role".into(),
                Some("A role of your own, such as review, that skills and agents can name.".into()),
                None,
                self.role_error
                    .clone()
                    .map(|error| RowState::Error(error.into()))
                    .as_ref(),
                add,
                cx,
            )
            .into_any_element(),
        );

        let section = |title: &'static str, rows: Vec<AnyElement>| {
            v_flex()
                .gap_3()
                .pb_6()
                .child(SettingsSectionHeader::new(title).no_padding(true))
                .children(
                    rows.into_iter()
                        .map(|row| v_flex().gap_3().child(row).child(Divider::horizontal())),
                )
        };
        v_flex()
            .w_full()
            .pt_2()
            .when_some(message, |this, message| {
                this.child(
                    div().pb_3().child(
                        Label::new(message)
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
                )
            })
            .pb_8()
            .when(choices.is_empty(), |this| {
                this.children(render_row_state(
                    &RowState::Empty {
                        text: "No models to choose from yet. Sign in to a provider on Account."
                            .into(),
                    },
                    cx,
                ))
                .child(div().h_4())
            })
            .child(section("Defaults", defaults))
            .child(section("Roles", role_rows))
    }
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
    fn a_role_on_a_provider_without_sign_in_points_to_account() {
        let choices = [choice("anthropic", "opus")];
        assert_eq!(role_state(None, &choices), None);
        assert_eq!(
            role_state(Some(&RoleModel::new("anthropic/opus")), &choices),
            None
        );
        assert_eq!(
            role_state(Some(&RoleModel::new("openai/gpt-6")), &choices),
            Some(RowState::Unavailable("Sign in to openai".into()))
        );
        assert_eq!(
            role_state(Some(&RoleModel::new("anthropic/gone")), &choices),
            Some(RowState::Unavailable(
                "anthropic/gone is not offered by anthropic".into()
            ))
        );
        assert_eq!(
            role_state(Some(&RoleModel::new("openai/gpt-6")), &[]),
            None,
            "nothing is known before the agent lists its models"
        );
    }

    #[test]
    fn a_stopped_agent_offers_start_instead_of_loading_forever() {
        let choices = [choice("anthropic", "opus")];
        assert_eq!(
            default_model_state(false, false, false, None, &choices),
            Some(RowState::NotRunning("Agent not running".into()))
        );
        assert_eq!(
            default_model_state(true, false, false, None, &choices),
            Some(RowState::Loading)
        );
        assert_eq!(default_model_state(true, false, true, None, &choices), None);
    }

    #[test]
    fn a_starting_agent_shows_loading_not_start() {
        let choices = [choice("anthropic", "opus")];
        assert_eq!(
            default_model_state(false, true, false, None, &choices),
            Some(RowState::Loading)
        );
    }

    #[test]
    fn role_rows_are_title_case() {
        assert_eq!(role_title("default"), "Default");
        assert_eq!(role_title("my-review"), "My-review");
        assert_eq!(role_title(""), "");
    }

    #[test]
    fn a_custom_role_is_new_and_plainly_named() {
        let mut roles = ModelRoles::default();
        roles.set("review", RoleModel::new("x/y"));
        assert_eq!(check_new_role(" docs ", &roles), Ok("docs".into()));
        assert!(check_new_role("", &roles).is_err());
        assert!(check_new_role("review", &roles).is_err());
        assert!(check_new_role("smol", &roles).is_err(), "built in");
        assert!(check_new_role("a b", &roles).is_err());
        assert!(check_new_role("a\"b", &roles).is_err());
    }
}
