//! Settings > Account (SET-2): sign in and out of the agent's providers
//! through the panel's own login flow, the person's API keys in the agent
//! dir's `.env` (ADR-0082), and Import from OMP.

use crate::panel::{Account, CedianPanel, Connection, KeyRestart};
use cedian_omp::agent_import::{self, Item, Options};
use cedian_omp::api_keys::{API_KEY_PROVIDERS, ApiKey, ApiKeyProvider, EnvFile};
use editor::Editor;
use gpui::{
    AnyWindowHandle, App, Context, Entity, PathPromptOptions, Render, Subscription, Window,
};
use settings_ui::{RowState, ScopeBadge, SettingsSectionHeader, render_row, render_row_state};
use std::path::PathBuf;
use ui::{Button, ButtonStyle, Checkbox, Divider, Label, ToggleState, prelude::*};

/// What one provider's row says and offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRow {
    pub status: String,
    pub action: ProviderAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAction {
    LogIn,
    /// Another login runs, or the agent cannot log in to this provider.
    LogInDisabled,
    LogOut,
    Cancel,
}

pub fn provider_row(account: &Account, provider: &omp_rpc::LoginProvider) -> ProviderRow {
    if account.running.as_deref() == Some(provider.id.as_str()) {
        return ProviderRow {
            status: "Signing in… Finish in your browser.".to_string(),
            action: ProviderAction::Cancel,
        };
    }
    if provider.authenticated {
        let labels: Vec<&str> = account
            .accounts
            .get(&provider.id)
            .into_iter()
            .flatten()
            .map(|account| account.label.as_str())
            .filter(|label| !label.is_empty())
            .collect();
        return ProviderRow {
            status: match labels.as_slice() {
                [] => "Signed in".to_string(),
                labels => format!("Signed in as {}", labels.join(", ")),
            },
            action: ProviderAction::LogOut,
        };
    }
    ProviderRow {
        status: "Not signed in".to_string(),
        action: if provider.available && account.running.is_none() {
            ProviderAction::LogIn
        } else {
            ProviderAction::LogInDisabled
        },
    }
}

/// The line under the API keys after a save: where the key is on its way.
pub fn key_restart_note(restart: KeyRestart) -> Option<&'static str> {
    match restart {
        KeyRestart::None => None,
        KeyRestart::AfterTurn => Some("Takes effect after this turn."),
        KeyRestart::Restarted => Some("Saved · agent restarted."),
    }
}

struct KeyRow {
    provider: &'static ApiKeyProvider,
    input: Entity<Editor>,
    ending: Result<Option<String>, String>,
    replacing: bool,
    error: Option<String>,
}

#[derive(Default)]
struct ImportForm {
    source: Option<PathBuf>,
    settings: bool,
    models: bool,
    preview: Option<Result<Vec<Item>, String>>,
    applied: Option<Result<Vec<String>, String>>,
}

impl ImportForm {
    fn options(&self, overwrite: bool) -> Options {
        Options {
            settings: self.settings,
            models: self.models,
            force: overwrite,
            apply: true,
            from: self.source.clone(),
        }
    }
}

pub struct AccountPage {
    panel: Entity<CedianPanel>,
    workspace_window: AnyWindowHandle,
    agent_dir: Option<PathBuf>,
    keys: Vec<KeyRow>,
    import: ImportForm,
    _observe: Subscription,
}

impl AccountPage {
    pub fn new(
        panel: Entity<CedianPanel>,
        workspace_window: AnyWindowHandle,
        agent_dir: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let keys = API_KEY_PROVIDERS
            .iter()
            .map(|provider| {
                let input = cx.new(|cx| {
                    let mut editor = Editor::single_line(window, cx);
                    editor.set_masked(true, cx);
                    editor.set_placeholder_text("Enter API key", window, cx);
                    editor
                });
                KeyRow {
                    provider,
                    input,
                    ending: Ok(None),
                    replacing: false,
                    error: None,
                }
            })
            .collect();
        panel.update(cx, |panel, _| panel.refresh_account());
        let mut this = Self {
            _observe: cx.observe(&panel, |_, _, cx| cx.notify()),
            panel,
            workspace_window,
            import: ImportForm {
                source: util::paths::home_dir()
                    .join(".omp/agent")
                    .is_dir()
                    .then(|| util::paths::home_dir().join(".omp/agent")),
                ..Default::default()
            },
            agent_dir,
            keys,
        };
        this.read_keys();
        this
    }

    fn env(&self) -> Result<EnvFile, String> {
        self.agent_dir
            .as_deref()
            .map(EnvFile::in_dir)
            .ok_or_else(|| "HOME is not set, so the agent has no config dir.".to_string())
    }

    fn read_keys(&mut self) {
        let env = self.env();
        for row in &mut self.keys {
            row.ending = env.clone().and_then(|env| env.key_ending(row.provider.var));
        }
    }

    pub fn key_ending(&self, var: &str) -> Option<&Result<Option<String>, String>> {
        self.keys
            .iter()
            .find(|row| row.provider.var == var)
            .map(|row| &row.ending)
    }

    pub fn key_error(&self, var: &str) -> Option<&str> {
        self.keys
            .iter()
            .find(|row| row.provider.var == var)
            .and_then(|row| row.error.as_deref())
    }

    pub fn key_input(&self, var: &str) -> Option<&Entity<Editor>> {
        self.keys
            .iter()
            .find(|row| row.provider.var == var)
            .map(|row| &row.input)
    }

    /// Save the typed key for `var` and restart the agent so it reads it.
    pub fn save_key(&mut self, var: &str, window: &mut Window, cx: &mut Context<Self>) {
        let env = self.env();
        let Some(row) = self.keys.iter_mut().find(|row| row.provider.var == var) else {
            return;
        };
        let typed = row.input.read(cx).text(cx);
        let saved = ApiKey::parse(&typed).and_then(|key| env?.save(var, &key));
        row.error = saved.err();
        if row.error.is_none() {
            row.replacing = false;
            row.input.update(cx, |editor, cx| editor.clear(window, cx));
            self.read_keys();
            self.restart_agent(cx);
        }
        cx.notify();
    }

    pub fn remove_key(&mut self, var: &str, cx: &mut Context<Self>) {
        let removed = self.env().and_then(|env| env.remove(var));
        if let Some(row) = self.keys.iter_mut().find(|row| row.provider.var == var) {
            row.error = removed.err();
            row.replacing = false;
        }
        self.read_keys();
        if self.key_error(var).is_none() {
            self.restart_agent(cx);
        }
        cx.notify();
    }

    fn restart_agent(&self, cx: &mut App) {
        let panel = self.panel.clone();
        self.workspace_window
            .update(cx, |_, window, cx| {
                panel.update(cx, |panel, cx| panel.restart_for_key(window, cx));
            })
            .ok();
    }

    fn with_panel(
        &self,
        cx: &mut App,
        f: impl FnOnce(&mut CedianPanel, &mut Window, &mut Context<CedianPanel>),
    ) {
        let panel = self.panel.clone();
        self.workspace_window
            .update(cx, |_, window, cx| {
                panel.update(cx, |panel, cx| f(panel, window, cx))
            })
            .ok();
    }

    pub fn set_import_source(&mut self, source: PathBuf, cx: &mut Context<Self>) {
        self.import.source = Some(source);
        self.import.preview = None;
        self.import.applied = None;
        cx.notify();
    }

    pub fn set_import_parts(&mut self, settings: bool, models: bool, cx: &mut Context<Self>) {
        self.import.settings = settings;
        self.import.models = models;
        self.import.preview = None;
        self.import.applied = None;
        cx.notify();
    }

    pub fn import_preview(&self) -> Option<&Result<Vec<Item>, String>> {
        self.import.preview.as_ref()
    }

    pub fn import_result(&self) -> Option<&Result<Vec<String>, String>> {
        self.import.applied.as_ref()
    }

    /// List what an import would copy, and which targets already exist.
    pub fn preview_import(&mut self, cx: &mut Context<Self>) {
        let (Some(source), Some(dest)) = (self.import.source.clone(), self.agent_dir.clone())
        else {
            self.import.preview = Some(Err("Choose the OMP folder to import from.".into()));
            cx.notify();
            return;
        };
        let options = self.import.options(false);
        let read = cx.background_spawn(async move {
            if !source.is_dir() {
                return Err(format!("No OMP folder at {}.", source.display()));
            }
            agent_import::plan(&source, &dest, &options)
        });
        cx.spawn(async move |this, cx| {
            let preview = read.await;
            this.update(cx, |this, cx| {
                this.import.preview = Some(preview);
                this.import.applied = None;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Copy the previewed files; `overwrite` replaces targets that exist.
    pub fn apply_import(&mut self, overwrite: bool, cx: &mut Context<Self>) {
        let (Some(source), Some(dest)) = (self.import.source.clone(), self.agent_dir.clone())
        else {
            return;
        };
        let options = self.import.options(overwrite);
        let copy = cx.background_spawn(async move { agent_import::run(&source, &dest, &options) });
        cx.spawn(async move |this, cx| {
            let applied = copy.await;
            this.update(cx, |this, cx| {
                if applied.is_ok() {
                    this.import.preview = None;
                    crate::settings_host::reload(cx);
                }
                this.import.applied = Some(applied);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn choose_import_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(source) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| this.set_import_source(source, cx))
                .ok();
        })
        .detach();
    }

    fn render_providers(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let panel = self.panel.read(cx);
        let account = panel.account().clone();
        let running = matches!(panel.connection(), Connection::Ready { .. });
        let mut rows = Vec::new();
        if !running {
            let control = Button::new("cedian-account-start", "Start")
                .style(ButtonStyle::OutlinedGhost)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.with_panel(cx, |panel, window, cx| panel.restart(window, cx));
                }))
                .into_any_element();
            rows.push(
                render_row(
                    "cedian-account-agent",
                    "Agent".into(),
                    Some("Sign-in goes through the agent of the open project.".into()),
                    None,
                    Some(&RowState::Unavailable("Agent not running".into())),
                    control,
                    cx,
                )
                .into_any_element(),
            );
            return rows;
        }
        if account.providers.is_empty() {
            rows.push(
                render_row(
                    "cedian-account-checking",
                    "Providers".into(),
                    Some(if account.checking {
                        "Checking…".into()
                    } else {
                        "The agent lists no providers.".into()
                    }),
                    None,
                    account.checking.then_some(&RowState::Loading),
                    div().into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
        }
        for provider in &account.providers {
            let row = provider_row(&account, provider);
            let id = provider.id.clone();
            let action_id = SharedString::from(format!("cedian-account-{id}"));
            let control = match row.action {
                ProviderAction::LogIn | ProviderAction::LogInDisabled => {
                    Button::new(action_id, "Log In")
                        .style(ButtonStyle::OutlinedGhost)
                        .disabled(row.action == ProviderAction::LogInDisabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let id = id.clone();
                            this.panel.update(cx, |panel, cx| panel.start_login(id, cx));
                        }))
                        .into_any_element()
                }
                ProviderAction::LogOut => Button::new(action_id, "Log Out")
                    .style(ButtonStyle::OutlinedGhost)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.panel.update(cx, |panel, cx| panel.log_out(&id, cx));
                    }))
                    .into_any_element(),
                ProviderAction::Cancel => {
                    let link = account
                        .url
                        .as_ref()
                        .map(|url| url.launch_url.clone().unwrap_or_else(|| url.url.clone()));
                    h_flex()
                        .gap_2()
                        .when_some(link, |this, link| {
                            this.child(
                                Button::new("cedian-account-link", "Open Sign-In Page")
                                    .style(ButtonStyle::Filled)
                                    .on_click(move |_, _, cx| cx.open_url(&link)),
                            )
                        })
                        .child(
                            Button::new(action_id, "Cancel")
                                .style(ButtonStyle::OutlinedGhost)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.panel.update(cx, |panel, cx| panel.cancel_login(cx));
                                })),
                        )
                        .into_any_element()
                }
            };
            rows.push(
                render_row(
                    SharedString::from(format!("cedian-provider-{}", provider.id)),
                    provider.name.clone().into(),
                    Some(row.status.into()),
                    None,
                    None,
                    control,
                    cx,
                )
                .into_any_element(),
            );
        }
        if let Some(error) = account.error {
            rows.extend(render_row_state(&RowState::Error(error.into()), cx));
        }
        rows
    }

    fn render_keys(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        for row in &self.keys {
            let var = row.provider.var;
            let saved = row.ending.as_ref().ok().cloned().flatten();
            let control = match saved.filter(|_| !row.replacing) {
                Some(ending) => h_flex()
                    .gap_2()
                    .child(
                        Label::new(format!("•••• ends in {ending}"))
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        Button::new(SharedString::from(format!("{var}-replace")), "Replace")
                            .style(ButtonStyle::OutlinedGhost)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(row) =
                                    this.keys.iter_mut().find(|row| row.provider.var == var)
                                {
                                    row.replacing = true;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("{var}-remove")), "Remove")
                            .style(ButtonStyle::OutlinedGhost)
                            .on_click(cx.listener(move |this, _, _, cx| this.remove_key(var, cx))),
                    )
                    .into_any_element(),
                None => h_flex()
                    .gap_2()
                    .child(
                        div()
                            .w(px(220.))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(cx.theme().colors().border)
                            .child(row.input.clone()),
                    )
                    .child(
                        Button::new(SharedString::from(format!("{var}-save")), "Save")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.save_key(var, window, cx)
                            })),
                    )
                    .into_any_element(),
            };
            let state = row
                .error
                .clone()
                .or_else(|| row.ending.clone().err())
                .map(|error| RowState::Error(error.into()));
            rows.push(
                render_row(
                    SharedString::from(format!("cedian-key-{var}")),
                    format!("{} API Key", row.provider.name).into(),
                    Some("Use your own key, billed by the provider.".into()),
                    Some(ScopeBadge::Agent(".env")),
                    state.as_ref(),
                    control,
                    cx,
                )
                .into_any_element(),
            );
        }
        if let Some(note) = key_restart_note(self.panel.read(cx).account().key_restart) {
            rows.push(
                Label::new(note)
                    .size(LabelSize::Small)
                    .color(Color::Muted)
                    .into_any_element(),
            );
        }
        rows
    }

    fn render_import(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let form = &self.import;
        let toggle = |on: bool| {
            if on {
                ToggleState::Selected
            } else {
                ToggleState::Unselected
            }
        };
        let parts = h_flex()
            .gap_3()
            .child(
                Checkbox::new("cedian-import-skills", ToggleState::Selected)
                    .label("Skills")
                    .disabled(true),
            )
            .child(
                Checkbox::new("cedian-import-settings", toggle(form.settings))
                    .label("Settings")
                    .on_click(cx.listener(|this, state: &ToggleState, _, cx| {
                        let models = this.import.models;
                        this.set_import_parts(state.selected(), models, cx);
                    })),
            )
            .child(
                Checkbox::new("cedian-import-models", toggle(form.models))
                    .label("Models")
                    .on_click(cx.listener(|this, state: &ToggleState, _, cx| {
                        let settings = this.import.settings;
                        this.set_import_parts(settings, state.selected(), cx);
                    })),
            )
            .into_any_element();
        let source = h_flex()
            .gap_2()
            .child(
                Label::new(
                    form.source
                        .as_ref()
                        .map(|source| source.display().to_string())
                        .unwrap_or_else(|| "No OMP folder found".to_string()),
                )
                .size(LabelSize::Small)
                .color(Color::Muted),
            )
            .child(
                Button::new("cedian-import-source", "Change")
                    .style(ButtonStyle::OutlinedGhost)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.choose_import_source(window, cx)),
                    ),
            )
            .into_any_element();
        let mut rows = vec![
            render_row(
                "cedian-import-parts",
                "Import from OMP".into(),
                Some("Bring skills, and on request settings and models, into cedian's agent. Sign-ins and keys are never copied.".into()),
                Some(ScopeBadge::User),
                None,
                parts,
                cx,
            )
            .into_any_element(),
            render_row(
                "cedian-import-source-row",
                "Source Folder".into(),
                None,
                None,
                None,
                source,
                cx,
            )
            .into_any_element(),
        ];
        let conflicts: Vec<String> = match &form.preview {
            Some(Ok(items)) => items
                .iter()
                .filter(|item| item.exists)
                .map(|item| item.rel.display().to_string())
                .collect(),
            _ => Vec::new(),
        };
        let ready = matches!(&form.preview, Some(Ok(items)) if !items.is_empty());
        let actions = h_flex()
            .gap_2()
            .child(
                Button::new("cedian-import-preview", "Preview")
                    .style(ButtonStyle::OutlinedGhost)
                    .on_click(cx.listener(|this, _, _, cx| this.preview_import(cx))),
            )
            .when(ready && conflicts.is_empty(), |this| {
                this.child(
                    Button::new("cedian-import-apply", "Apply")
                        .style(ButtonStyle::Filled)
                        .on_click(cx.listener(|this, _, _, cx| this.apply_import(false, cx))),
                )
            })
            .when(!conflicts.is_empty(), |this| {
                this.child(
                    Button::new("cedian-import-overwrite", "Overwrite")
                        .style(ButtonStyle::Filled)
                        .on_click(cx.listener(|this, _, _, cx| this.apply_import(true, cx))),
                )
            })
            .into_any_element();
        let state = match &form.preview {
            Some(Ok(items)) if items.is_empty() => Some(RowState::Empty {
                text: "Nothing to import.".into(),
            }),
            Some(Err(error)) => Some(RowState::Error(error.clone().into())),
            _ => match &form.applied {
                Some(Err(error)) => Some(RowState::Error(error.clone().into())),
                _ => None,
            },
        };
        let description = match (&form.preview, &form.applied) {
            (Some(Ok(items)), _) if !items.is_empty() => {
                let mut lines: Vec<String> = items
                    .iter()
                    .map(|item| {
                        let note = if item.exists { " (already there)" } else { "" };
                        format!("{}{note}", item.rel.display())
                    })
                    .collect();
                if !conflicts.is_empty() {
                    lines.push(format!(
                        "Overwrite replaces what is already there: {}.",
                        conflicts.join(", ")
                    ));
                }
                Some(lines.join("\n"))
            }
            (_, Some(Ok(lines))) => Some(lines.join("\n")),
            _ => Some("List what would be copied before anything is.".to_string()),
        };
        rows.push(
            render_row(
                "cedian-import-preview-row",
                "Preview".into(),
                description.map(Into::into),
                None,
                state.as_ref(),
                actions,
                cx,
            )
            .into_any_element(),
        );
        rows
    }
}

impl Render for AccountPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(section("Providers", self.render_providers(cx)))
            .child(section("API Keys", self.render_keys(cx)))
            .child(section("Import", self.render_import(cx)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(id: &str, available: bool, authenticated: bool) -> omp_rpc::LoginProvider {
        omp_rpc::LoginProvider {
            id: id.into(),
            name: id.into(),
            available,
            authenticated,
        }
    }

    #[test]
    fn each_provider_row_says_its_state_and_offers_one_action() {
        let mut account = Account::default();
        let anthropic = provider("anthropic", true, true);
        assert_eq!(
            provider_row(&account, &anthropic),
            ProviderRow {
                status: "Signed in".into(),
                action: ProviderAction::LogOut
            }
        );
        account.accounts.insert(
            "anthropic".into(),
            vec![omp_rpc::LogoutAccount {
                credential_id: 1,
                provider: "anthropic".into(),
                label: "pond@example.com".into(),
                detail: String::new(),
                r#type: omp_rpc::LogoutAccountType::Oauth,
                active: true,
            }],
        );
        assert_eq!(
            provider_row(&account, &anthropic).status,
            "Signed in as pond@example.com"
        );

        let openai = provider("openai", true, false);
        assert_eq!(
            provider_row(&account, &openai).action,
            ProviderAction::LogIn
        );
        assert_eq!(
            provider_row(&account, &provider("cursor", false, false)).action,
            ProviderAction::LogInDisabled,
            "the agent cannot log in here"
        );

        account.running = Some("openai".into());
        assert_eq!(
            provider_row(&account, &openai),
            ProviderRow {
                status: "Signing in… Finish in your browser.".into(),
                action: ProviderAction::Cancel
            }
        );
        assert_eq!(
            provider_row(&account, &provider("google", true, false)).action,
            ProviderAction::LogInDisabled,
            "one login at a time"
        );
    }

    struct Dir(PathBuf);

    impl Drop for Dir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    async fn page(
        cx: &mut gpui::TestAppContext,
        agent_dir: PathBuf,
    ) -> (Entity<CedianPanel>, gpui::WindowHandle<AccountPage>) {
        cx.update(|cx| {
            let store = settings::SettingsStore::test(cx);
            cx.set_global(store);
            theme_settings::init(theme::LoadThemes::JustBase, cx);
            editor::init(cx);
        });
        let fs = fs::FakeFs::new(cx.executor());
        let project = project::Project::test(fs, [], cx).await;
        let panel_window = cx.add_window(|window, cx| CedianPanel::new(project, window, cx));
        let panel = panel_window.root(cx).unwrap();
        let page = cx.add_window(|window, cx| {
            AccountPage::new(
                panel.clone(),
                panel_window.into(),
                Some(agent_dir),
                window,
                cx,
            )
        });
        (panel, page)
    }

    #[gpui::test]
    async fn a_key_is_saved_masked_replaced_and_removed_and_the_agent_restarts(
        cx: &mut gpui::TestAppContext,
    ) {
        use std::os::unix::fs::PermissionsExt as _;
        let dir =
            Dir(std::env::temp_dir().join(format!("cedian-account-keys-{}", std::process::id())));
        std::fs::remove_dir_all(&dir.0).ok();
        let agent = dir.0.join("agent");
        let (panel, page) = page(cx, agent.clone()).await;
        let var = "OPENAI_API_KEY";
        let type_key = |cx: &mut gpui::TestAppContext, key: &'static str| {
            page.update(cx, |page, window, cx| {
                page.key_input(var)
                    .unwrap()
                    .update(cx, |editor, cx| editor.set_text(key, window, cx));
                page.save_key(var, window, cx);
            })
            .unwrap();
        };

        type_key(cx, "two words");
        page.update(cx, |page, _, _| {
            assert!(page.key_error(var).is_some_and(|e| e.contains("no spaces")));
            assert_eq!(page.key_ending(var), Some(&Ok(None)));
        })
        .unwrap();
        assert!(!agent.join(".env").exists(), "a refused key writes nothing");

        page.update(cx, |page, window, cx| {
            page.key_input(var).unwrap().update(cx, |editor, cx| {
                editor.set_text("sk-test-4f2a", window, cx);
                assert!(
                    !editor.display_text(cx).contains("sk-test"),
                    "the field is masked"
                );
            });
        })
        .unwrap();
        type_key(cx, "sk-test-4f2a");
        page.update(cx, |page, _, cx| {
            assert_eq!(page.key_error(var), None);
            assert_eq!(page.key_ending(var), Some(&Ok(Some("4f2a".into()))));
            assert_eq!(
                page.key_input(var).unwrap().read(cx).text(cx),
                "",
                "the field clears"
            );
        })
        .unwrap();
        let env = std::fs::metadata(agent.join(".env")).unwrap();
        assert_eq!(env.permissions().mode() & 0o777, 0o600);
        panel.read_with(cx, |panel, _| {
            assert_eq!(panel.account().key_restart, KeyRestart::Restarted)
        });

        type_key(cx, "sk-next-9b1c");
        assert_eq!(
            std::fs::read_to_string(agent.join(".env")).unwrap(),
            "OPENAI_API_KEY=sk-next-9b1c\n"
        );
        page.update(cx, |page, _, cx| page.remove_key(var, cx))
            .unwrap();
        assert_eq!(std::fs::read_to_string(agent.join(".env")).unwrap(), "");
        page.update(cx, |page, _, _| {
            assert_eq!(page.key_ending(var), Some(&Ok(None)))
        })
        .unwrap();
    }

    #[gpui::test]
    async fn import_previews_then_copies_and_never_takes_credentials(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir =
            Dir(std::env::temp_dir().join(format!("cedian-account-import-{}", std::process::id())));
        std::fs::remove_dir_all(&dir.0).ok();
        let source = dir.0.join("omp");
        std::fs::create_dir_all(source.join("skills/alpha")).unwrap();
        std::fs::write(source.join("skills/alpha/SKILL.md"), "alpha").unwrap();
        std::fs::write(source.join("config.yml"), "{}").unwrap();
        std::fs::write(source.join("agent.db"), "TOKENS").unwrap();
        std::fs::write(source.join(".env"), "OPENAI_API_KEY=theirs").unwrap();
        let agent = dir.0.join("agent");
        std::fs::create_dir_all(agent.join("skills/alpha")).unwrap();
        std::fs::write(agent.join("skills/alpha/SKILL.md"), "mine").unwrap();
        std::fs::write(agent.join(".env"), "OPENAI_API_KEY=mine\n").unwrap();
        let (_, page) = page(cx, agent.clone()).await;

        page.update(cx, |page, _, cx| {
            page.set_import_source(source.clone(), cx);
            page.set_import_parts(true, false, cx);
            page.preview_import(cx);
        })
        .unwrap();
        cx.run_until_parked();
        page.update(cx, |page, _, cx| {
            let items = page.import_preview().unwrap().as_ref().unwrap();
            let listed: Vec<_> = items
                .iter()
                .map(|item| (item.rel.display().to_string(), item.exists))
                .collect();
            assert_eq!(
                listed,
                [
                    ("skills/alpha".to_string(), true),
                    ("config.yml".to_string(), false)
                ]
            );
            page.apply_import(false, cx);
        })
        .unwrap();
        cx.run_until_parked();
        page.update(cx, |page, _, cx| {
            let refused = page.import_result().unwrap().as_ref().unwrap_err();
            assert!(refused.contains("skills/alpha"), "{refused}");
            page.apply_import(true, cx);
        })
        .unwrap();
        cx.run_until_parked();
        page.update(cx, |page, _, _| {
            assert!(page.import_result().unwrap().is_ok())
        })
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(agent.join("skills/alpha/SKILL.md")).unwrap(),
            "alpha"
        );
        assert!(agent.join("config.yml").is_file());
        assert!(!agent.join("agent.db").exists());
        assert_eq!(
            std::fs::read_to_string(agent.join(".env")).unwrap(),
            "OPENAI_API_KEY=mine\n",
            "credentials are never copied"
        );
    }

    #[test]
    fn a_saved_key_says_when_it_takes_effect() {
        assert_eq!(key_restart_note(KeyRestart::None), None);
        assert_eq!(
            key_restart_note(KeyRestart::AfterTurn),
            Some("Takes effect after this turn.")
        );
        assert_eq!(
            key_restart_note(KeyRestart::Restarted),
            Some("Saved · agent restarted.")
        );
    }
}
