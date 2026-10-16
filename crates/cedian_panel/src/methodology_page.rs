use crate::omp_settings::OmpSettings;
use crate::panel::CedianPanel;
use cedian_shell::pstack::{
    DefaultEffort, Effort, PstackModels, PstackRole, RoleModel, Switches, write_switches,
};
use gpui::{App, ClipboardItem, Context, Entity, Render, Subscription, Window};
use settings_ui::{
    ModelChoice, RowState, ScopeBadge, SettingsSectionHeader, group_models, model_picker,
    pstack_version, render_row,
};
use std::path::PathBuf;
use std::sync::Arc;
use ui::{
    Button, ButtonStyle, ContextMenu, Disclosure, Divider, DropdownMenu, Switch, ToggleState,
    prelude::*,
};

/// The picker key for "Agent default": the role's line goes.
pub const AGENT_DEFAULT: &str = "";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Pstack,
    Mandate,
    DefaultEffort,
    Role(PstackRole),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Methodology {
    settings_path: Option<PathBuf>,
    sheet_path: Option<PathBuf>,
    pub switches: Switches,
    pub sheet: PstackModels,
    pub error: Option<(Row, String)>,
}

impl Methodology {
    /// The values in the user's `cedian.toml` at `settings_path` and the
    /// sheet at `sheet_path`; defaults for a file that does not exist yet.
    pub fn load(settings_path: Option<PathBuf>, sheet_path: Option<PathBuf>) -> Self {
        let mut errors = Vec::new();
        let switches = match settings_path.as_deref().map(std::fs::read_to_string) {
            Some(Ok(text)) => match cedian_shell::parse_settings(&text) {
                Ok(settings) => Switches {
                    pstack: settings.pstack,
                    mandate: settings.pstack_mandate,
                },
                Err(e) => {
                    errors.push(e.to_string());
                    Switches::default()
                }
            },
            Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => {
                errors.push(e.to_string());
                Switches::default()
            }
            _ => Switches::default(),
        };
        let sheet = match sheet_path.as_deref().map(PstackModels::read) {
            Some(Ok(sheet)) => sheet,
            Some(Err(e)) => {
                errors.push(e);
                PstackModels::default()
            }
            None => PstackModels::default(),
        };
        if settings_path.is_none() || sheet_path.is_none() {
            errors.push("HOME is not set, so cedian has nowhere to keep these settings".into());
        }
        Self {
            settings_path,
            sheet_path,
            switches,
            sheet,
            error: (!errors.is_empty()).then(|| (Row::Pstack, errors.join("; "))),
        }
    }

    /// "User" on a row only while the user's files set it to something other
    /// than the default; a fresh profile shows no badge.
    pub fn badge(&self, row: Row) -> Option<ScopeBadge> {
        let set = match row {
            Row::Pstack => self.switches.pstack != Switches::default().pstack,
            Row::Mandate => self.switches.mandate != Switches::default().mandate,
            Row::DefaultEffort => {
                self.sheet.default_effort != PstackModels::default().default_effort
            }
            Row::Role(role) => self.sheet.get(role).is_some(),
        };
        set.then_some(ScopeBadge::User)
    }

    /// For this environment: `cedian.toml` where cedian reads it, the sheet
    /// in the agent's config dir.
    pub fn load_from_env() -> Self {
        Self::load(
            cedian_shell::user_settings_path(),
            cedian_shell::pstack::sheet_path().ok(),
        )
    }

    pub fn set_pstack(&mut self, on: bool) {
        self.write_switches(
            Row::Pstack,
            Switches {
                pstack: on,
                ..self.switches
            },
        );
    }

    pub fn set_mandate(&mut self, on: bool) {
        self.write_switches(
            Row::Mandate,
            Switches {
                mandate: on,
                ..self.switches
            },
        );
    }

    pub fn set_default_effort(&mut self, effort: DefaultEffort) {
        let mut sheet = self.sheet.clone();
        sheet.default_effort = effort;
        self.write_sheet(Row::DefaultEffort, sheet);
    }

    /// Point `role` at the picked `key` (a model, `@role`, or
    /// [`AGENT_DEFAULT`]), keeping its effort.
    pub fn pick(&mut self, role: PstackRole, key: &str) {
        let model = if key == AGENT_DEFAULT {
            None
        } else {
            let effort = self.sheet.get(role).and_then(|model| model.effort);
            match RoleModel::new(key, effort) {
                Ok(model) => Some(model),
                Err(e) => {
                    self.error = Some((Row::Role(role), e));
                    return;
                }
            }
        };
        let mut sheet = self.sheet.clone();
        sheet.set(role, model);
        self.write_sheet(Row::Role(role), sheet);
    }

    /// `None` follows the default effort. A role on the agent's default has
    /// no line to carry an effort, so it keeps none.
    pub fn set_effort(&mut self, role: PstackRole, effort: Option<Effort>) {
        let Some(current) = self.sheet.get(role) else {
            return;
        };
        let Ok(model) = RoleModel::new(current.model(), effort) else {
            return;
        };
        let mut sheet = self.sheet.clone();
        sheet.set(role, Some(model));
        self.write_sheet(Row::Role(role), sheet);
    }

    pub fn child_state(&self) -> Option<RowState> {
        (!self.switches.pstack).then_some(RowState::DisabledChild)
    }

    pub fn row_state(&self, row: Row, fallback: Option<RowState>) -> Option<RowState> {
        match &self.error {
            Some((at, error)) if *at == row => Some(RowState::Error(error.clone().into())),
            _ => fallback,
        }
    }

    fn write_switches(&mut self, row: Row, switches: Switches) {
        let Some(path) = &self.settings_path else {
            return;
        };
        match write_switches(path, switches) {
            Ok(()) => {
                self.switches = switches;
                self.error = None;
            }
            Err(e) => self.error = Some((row, e)),
        }
    }

    fn write_sheet(&mut self, row: Row, sheet: PstackModels) {
        let Some(path) = &self.sheet_path else {
            return;
        };
        match sheet.write(path) {
            Ok(()) => {
                self.sheet = sheet;
                self.error = None;
            }
            Err(e) => self.error = Some((row, e)),
        }
    }
}

pub struct MethodologyPage {
    panel: Entity<CedianPanel>,
    settings: Entity<OmpSettings>,
    state: Methodology,
    more_roles: bool,
    _observe: [Subscription; 2],
}

impl MethodologyPage {
    pub fn new(
        panel: Entity<CedianPanel>,
        settings: Entity<OmpSettings>,
        cx: &mut Context<Self>,
    ) -> Self {
        panel.read(cx).refresh_models();
        Self {
            _observe: [
                cx.observe(&panel, |_, _, cx| cx.notify()),
                cx.observe(&settings, |_, _, cx| cx.notify()),
            ],
            panel,
            settings,
            state: Methodology::load_from_env(),
            more_roles: false,
        }
    }

    fn toggle_more_roles(&mut self, cx: &mut Context<Self>) {
        self.more_roles = !self.more_roles;
        cx.notify();
    }

    pub fn state(&self) -> &Methodology {
        &self.state
    }

    fn choices(&self, cx: &App) -> Arc<[ModelChoice]> {
        let models = group_models(
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
        );
        let roles = self
            .settings
            .read(cx)
            .roles()
            .rows()
            .into_iter()
            .map(|role| ModelChoice {
                key: format!("@{role}").into(),
                name: format!("@{role}").into(),
                provider: "Agent role".into(),
            });
        std::iter::once(ModelChoice {
            key: AGENT_DEFAULT.into(),
            name: "Agent default".into(),
            provider: "".into(),
        })
        .chain(models)
        .chain(roles)
        .collect()
    }

    fn update(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Methodology)) {
        change(&mut self.state);
        cx.notify();
    }

    fn switch(
        &self,
        id: &'static str,
        on: bool,
        disabled: bool,
        write: fn(&mut Methodology, bool),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let this = cx.entity().downgrade();
        Switch::new(id, ToggleState::from(on))
            .disabled(disabled)
            .on_click(move |state, _, cx| {
                let on = *state == ToggleState::Selected;
                this.update(cx, |this, cx| this.update(cx, |state| write(state, on)))
                    .ok();
            })
            .into_any_element()
    }

    fn effort_menu<T: Copy + PartialEq + 'static>(
        &self,
        id: SharedString,
        current: T,
        options: Vec<(&'static str, T)>,
        disabled: bool,
        write: impl Fn(&mut Methodology, T) + Clone + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = options
            .iter()
            .find(|(_, value)| *value == current)
            .map_or("", |(label, _)| label);
        let this = cx.entity().downgrade();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for (label, value) in options.iter().copied() {
                let this = this.clone();
                let write = write.clone();
                menu = menu.toggleable_entry(
                    label,
                    current == value,
                    IconPosition::Start,
                    None,
                    move |_, cx| {
                        this.update(cx, |this, cx| this.update(cx, |state| write(state, value)))
                            .ok();
                    },
                );
            }
            menu
        });
        DropdownMenu::new(id, label, menu)
            .style(ui::DropdownStyle::Outlined)
            .disabled(disabled)
            .into_any_element()
    }

    fn role_row(
        &self,
        role: PstackRole,
        choices: &Arc<[ModelChoice]>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.state.child_state();
        let disabled = state.is_some();
        let current = self.state.sheet.get(role);
        let id = role.sheet_label().replace([',', ' '], "-");
        let picker = if disabled {
            let key = current.map_or(AGENT_DEFAULT, |model| model.model());
            let name = choices
                .iter()
                .find(|choice| choice.key.as_ref() == key)
                .map_or_else(
                    || SharedString::from(key.to_string()),
                    |choice| choice.name.clone(),
                );
            Button::new(
                SharedString::from(format!("cedian-pstack-model-{id}")),
                name,
            )
            .style(ButtonStyle::Outlined)
            .disabled(true)
            .into_any_element()
        } else {
            let this = cx.entity().downgrade();
            model_picker(
                SharedString::from(format!("cedian-pstack-model-{id}")),
                choices.clone(),
                Some(
                    current
                        .map_or(AGENT_DEFAULT, |model| model.model())
                        .to_string()
                        .into(),
                ),
                move |key, _, cx| {
                    this.update(cx, |this, cx| {
                        this.update(cx, |state| state.pick(role, &key))
                    })
                    .ok();
                },
            )
        };
        let mut efforts = vec![("Default", None)];
        efforts.extend(Effort::ALL.map(|effort| (effort.label(), Some(effort))));
        let effort = self.effort_menu(
            SharedString::from(format!("cedian-pstack-effort-{id}")),
            current.and_then(|model| model.effort),
            efforts,
            disabled || current.is_none(),
            move |state, effort| state.set_effort(role, effort),
            window,
            cx,
        );
        render_row(
            SharedString::from(format!("cedian-pstack-row-{id}")),
            role.title().into(),
            Some(role.description().into()),
            self.state.badge(Row::Role(role)),
            self.state.row_state(Row::Role(role), state).as_ref(),
            h_flex()
                .gap_2()
                .child(picker)
                .child(effort)
                .into_any_element(),
            cx,
        )
        .into_any_element()
    }
}

impl Render for MethodologyPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choices = self.choices(cx);
        let child = self.state.child_state();
        let disabled = child.is_some();
        let switches = self.state.switches;

        let pstack_switch = self.switch(
            "cedian-pstack",
            switches.pstack,
            false,
            Methodology::set_pstack,
            cx,
        );
        let mandate_switch = self.switch(
            "cedian-pstack-mandate",
            switches.mandate,
            disabled,
            Methodology::set_mandate,
            cx,
        );
        let mut levels = vec![("Same as session", DefaultEffort::Session)];
        levels.extend(Effort::ALL.map(|effort| (effort.label(), DefaultEffort::Level(effort))));
        let default_effort = self.effort_menu(
            "cedian-pstack-default-effort".into(),
            self.state.sheet.default_effort,
            levels,
            disabled,
            Methodology::set_default_effort,
            window,
            cx,
        );
        let general = vec![
            render_row(
                "cedian-pstack-row",
                "pstack".into(),
                Some("Use pstack's skills and playbooks in every session. Takes effect when the agent restarts.".into()),
                self.state.badge(Row::Pstack),
                self.state.row_state(Row::Pstack, None).as_ref(),
                pstack_switch,
                cx,
            )
            .into_any_element(),
            render_row(
                "cedian-pstack-mandate-row",
                "Mandate".into(),
                Some("Append the poteto-mode mandate to the agent's system prompt. Reviewers never get it.".into()),
                self.state.badge(Row::Mandate),
                self.state.row_state(Row::Mandate, child.clone()).as_ref(),
                mandate_switch,
                cx,
            )
            .into_any_element(),
            render_row(
                "cedian-pstack-default-effort-row",
                "Default Reasoning Effort".into(),
                Some("Effort for roles that do not set their own.".into()),
                self.state.badge(Row::DefaultEffort),
                self.state.row_state(Row::DefaultEffort, child).as_ref(),
                default_effort,
                cx,
            )
            .into_any_element(),
        ];

        let mut roles: Vec<AnyElement> = PstackRole::ALL
            .into_iter()
            .filter(|role| !role.folded_under_more_roles())
            .map(|role| self.role_row(role, &choices, window, cx))
            .collect();
        let more_open = self.more_roles;
        roles.push(
            h_flex()
                .id("cedian-pstack-more-roles-row")
                .debug_selector(|| "cedian-pstack-more-roles-row".to_string())
                .gap_1()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.toggle_more_roles(cx)))
                .child(
                    Disclosure::new("cedian-pstack-more-roles", more_open)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_more_roles(cx))),
                )
                .child(Label::new("More roles").color(Color::Muted))
                .into_any_element(),
        );
        if more_open {
            roles.extend(
                PstackRole::ALL
                    .into_iter()
                    .filter(|role| role.folded_under_more_roles())
                    .map(|role| self.role_row(role, &choices, window, cx)),
            );
        }

        let version = vec![
            render_row(
                "cedian-pstack-version-row",
                "pstack Version".into(),
                Some("The bundled version and commit (Lauren Tan, MIT).".into()),
                None,
                None,
                h_flex()
                    .gap_2()
                    .child(
                        Label::new(pstack_version())
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        Button::new("cedian-pstack-copy-version", "Copy")
                            .style(ButtonStyle::OutlinedGhost)
                            .on_click(|_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(pstack_version()))
                            }),
                    )
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
        ];

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
            .child(section("pstack", general))
            .child(section("Role Models", roles))
            .child(section("Version", version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("cedian.toml");
        let sheet = dir.path().join("agent/pstack-models.md");
        (dir, settings, sheet)
    }

    #[test]
    fn a_fresh_install_shows_pstack_and_the_mandate_on() {
        let (_dir, settings, sheet) = paths();
        let state = Methodology::load(Some(settings), Some(sheet));
        assert_eq!(state.switches, Switches::default());
        assert_eq!(state.sheet, PstackModels::default());
        assert_eq!(state.error, None);
        assert_eq!(state.child_state(), None);
    }

    #[test]
    fn each_row_writes_its_own_file_and_reloads_the_same() {
        let (_dir, settings, sheet) = paths();
        let mut state = Methodology::load(Some(settings.clone()), Some(sheet.clone()));
        state.set_mandate(false);
        state.pick(PstackRole::BugFix, "anthropic/claude-opus-5");
        state.set_effort(PstackRole::BugFix, Some(Effort::High));
        state.pick(PstackRole::BugFix, "@slow");
        state.pick(PstackRole::Hillclimb, "@plan");
        state.pick(PstackRole::Hillclimb, AGENT_DEFAULT);
        state.set_effort(PstackRole::Judgment, Some(Effort::Max));
        state.set_default_effort(DefaultEffort::Level(Effort::Medium));
        assert_eq!(state.error, None);

        let toml =
            cedian_shell::parse_settings(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert!(toml.pstack && !toml.pstack_mandate);
        let text = std::fs::read_to_string(&sheet).unwrap();
        let lines: Vec<_> = text.lines().filter(|line| line.contains(": ")).collect();
        assert_eq!(
            lines,
            ["bug-fix: @slow @high", "default effort: medium"],
            "a new model keeps the role's effort; Agent default drops the line; a role with no model takes no effort"
        );
        assert_eq!(Methodology::load(Some(settings), Some(sheet)), state);
    }

    #[test]
    fn turning_pstack_off_disables_the_rows_and_keeps_their_values() {
        let (_dir, settings, sheet) = paths();
        let mut state = Methodology::load(Some(settings), Some(sheet.clone()));
        state.pick(PstackRole::Feature, "@slow");
        state.set_pstack(false);
        assert_eq!(state.child_state(), Some(RowState::DisabledChild));
        assert!(state.child_state().unwrap().disables_control());
        assert_eq!(
            state.sheet.get(PstackRole::Feature).map(RoleModel::model),
            Some("@slow")
        );
        state.set_pstack(true);
        assert_eq!(state.child_state(), None);
        assert!(
            std::fs::read_to_string(&sheet)
                .unwrap()
                .contains("feature, refactoring: @slow")
        );
    }

    #[test]
    fn a_failed_write_shows_on_the_row_that_failed() {
        let (dir, settings, _sheet) = paths();
        let blocked = dir.path().join("file");
        std::fs::write(&blocked, "").unwrap();
        let mut state = Methodology::load(Some(settings), Some(blocked.join("pstack-models.md")));
        state.pick(PstackRole::BugFix, "@slow");
        assert!(matches!(
            state.row_state(Row::Role(PstackRole::BugFix), None),
            Some(RowState::Error(_))
        ));
        assert_eq!(state.row_state(Row::Pstack, None), None);
        assert_eq!(state.row_state(Row::Role(PstackRole::Feature), None), None);

        state.set_default_effort(DefaultEffort::Level(Effort::Low));
        assert!(matches!(
            state.row_state(Row::DefaultEffort, None),
            Some(RowState::Error(_))
        ));
        assert_eq!(state.row_state(Row::Role(PstackRole::BugFix), None), None);

        state.set_mandate(false);
        assert_eq!(
            state.error, None,
            "the switch wrote, so no row shows an error"
        );
    }

    #[test]
    fn a_refused_write_says_so_and_keeps_the_shown_value() {
        let (dir, settings, sheet) = paths();
        std::fs::write(&settings, "schema = 1\nnot = valid = toml").unwrap();
        let mut state = Methodology::load(Some(settings.clone()), Some(sheet));
        assert!(state.error.is_some(), "the read error is shown");
        state.set_pstack(false);
        assert!(state.switches.pstack, "unchanged");
        assert!(state.error.is_some());
        assert_eq!(
            std::fs::read_to_string(&settings).unwrap(),
            "schema = 1\nnot = valid = toml",
            "never overwritten"
        );

        let blocked = dir.path().join("file");
        std::fs::write(&blocked, "").unwrap();
        let mut state = Methodology::load(None, Some(blocked.join("pstack-models.md")));
        state.set_default_effort(DefaultEffort::Level(Effort::Low));
        assert_eq!(state.sheet.default_effort, DefaultEffort::Session);
        assert!(state.error.is_some());
    }

    #[test]
    fn a_row_shows_the_user_badge_only_once_the_user_sets_it() {
        let (_dir, settings, sheet) = paths();
        let mut state = Methodology::load(Some(settings), Some(sheet));
        let rows = [
            Row::Pstack,
            Row::Mandate,
            Row::DefaultEffort,
            Row::Role(PstackRole::BugFix),
        ];
        for row in rows {
            assert_eq!(state.badge(row), None, "{row:?} on a fresh profile");
        }
        state.set_mandate(false);
        state.pick(PstackRole::BugFix, "@slow");
        state.set_default_effort(DefaultEffort::Level(Effort::High));
        for row in &rows[1..] {
            assert_eq!(state.badge(*row), Some(ScopeBadge::User), "{row:?}");
        }
        assert_eq!(state.badge(Row::Pstack), None);
    }

    #[gpui::test]
    async fn clicking_the_more_roles_label_opens_them(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let store = settings::SettingsStore::test(cx);
            cx.set_global(store);
            theme_settings::init(theme::LoadThemes::JustBase, cx);
            editor::init(cx);
        });
        let project = project::Project::test(fs::FakeFs::new(cx.executor()), [], cx).await;
        let (page, cx) = cx.add_window_view(|window, cx| {
            let panel = cx.new(|cx| CedianPanel::new(project.clone(), window, cx));
            let settings =
                cx.new(|cx| OmpSettings::new(project.clone(), PathBuf::from("/"), window, cx));
            MethodologyPage::new(panel, settings, cx)
        });
        cx.run_until_parked();
        for open in [true, false] {
            let row = cx
                .debug_bounds("cedian-pstack-more-roles-row")
                .expect("the More roles row is drawn");
            cx.simulate_click(
                gpui::point(row.left() + gpui::px(48.), row.center().y),
                gpui::Modifiers::none(),
            );
            cx.run_until_parked();
            page.read_with(cx, |page, _| assert_eq!(page.more_roles, open));
        }
    }
}
