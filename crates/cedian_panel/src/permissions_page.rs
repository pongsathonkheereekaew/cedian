use cedian_shell::settings_writer::{ProjectChange, Tier, write_project, write_tier};
use cedian_shell::{Policy, RunKind, Settings, SettingsError, Verdict};
use gpui::{Context, PathPromptOptions, Render, Window};
use settings_ui::{RowState, ScopeBadge, SettingsSectionHeader, render_row};
use std::path::{Path, PathBuf};
use ui::{Button, ButtonStyle, ContextMenu, Divider, DropdownMenu, prelude::*};
use util::ResultExt as _;

pub const OMP_POLICY_WARNING: &str = "Approvals and desktop control follow the agent's config, yolo included. A prompt-injected agent can do whatever that config allows.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    Tier(Tier),
    Policy(String),
    Pstack(String),
    Folders(String),
    Entry(String),
}

/// One project as Settings shows it: what the user's `cedian.toml` stores
/// under its key, and why any of it does not apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEntry {
    pub key: String,
    pub open: bool,
    pub policy: Policy,
    pub pstack: Option<bool>,
    pub read_folders: Vec<PathBuf>,
    pub notes: Vec<String>,
}

/// The Permissions and Projects pages' values. Read only from the user's
/// `cedian.toml`, never from a file inside a project, and every change goes
/// back there through the typed writer.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSettings {
    path: Option<PathBuf>,
    settings: Settings,
    pub load_error: Option<String>,
    pub error: Option<(Row, String)>,
}

impl ProjectSettings {
    pub fn load(path: Option<PathBuf>) -> Self {
        let (settings, load_error) = match path.as_deref().map(cedian_shell::load_user_file) {
            Some(Ok(settings)) => (settings, None),
            Some(Err(SettingsError::Missing(_))) => (Settings::default(), None),
            Some(Err(e)) => (Settings::default(), Some(e.to_string())),
            None => (
                Settings::default(),
                Some("HOME is not set, so cedian has nowhere to keep these settings".to_string()),
            ),
        };
        Self {
            path,
            settings,
            load_error,
            error: None,
        }
    }

    pub fn load_from_env() -> Self {
        Self::load(cedian_shell::user_settings_path())
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn tier(&self, tier: Tier) -> Verdict {
        let permissions = &self.settings.permissions;
        match tier {
            Tier::Safe => permissions.safe,
            Tier::ProjectWrite => permissions.project_write,
            Tier::Dangerous => permissions.dangerous,
        }
    }

    /// The policy `project` runs under now, after every check.
    pub fn policy_in_effect(&self, project: &Path) -> Policy {
        self.settings
            .policy_for(project, RunKind::Interactive)
            .policy
    }

    pub fn set_tier(&mut self, tier: Tier, verdict: Verdict) {
        self.write(Row::Tier(tier), |path| write_tier(path, tier, verdict));
    }

    pub fn set_policy(&mut self, key: &str, policy: Policy) {
        self.write_project(Row::Policy(key.into()), key, ProjectChange::Policy(policy));
    }

    pub fn set_pstack(&mut self, key: &str, pstack: Option<bool>) {
        self.write_project(Row::Pstack(key.into()), key, ProjectChange::Pstack(pstack));
    }

    /// Allow reads of `folder` (canonicalized) in the project at `key`.
    pub fn add_folder(&mut self, key: &str, folder: &Path) {
        let folder = match folder.canonicalize() {
            Ok(folder) => folder,
            Err(e) => {
                self.error = Some((
                    Row::Folders(key.into()),
                    format!("{}: {e}", folder.display()),
                ));
                return;
            }
        };
        let mut folders = self.settings.project_read_folders(key).to_vec();
        if !folders.contains(&folder) {
            folders.push(folder);
        }
        self.write_project(
            Row::Folders(key.into()),
            key,
            ProjectChange::ReadFolders(folders),
        );
    }

    pub fn remove_folder(&mut self, key: &str, folder: &Path) {
        let folders = self
            .settings
            .project_read_folders(key)
            .iter()
            .filter(|each| each.as_path() != folder)
            .cloned()
            .collect();
        self.write_project(
            Row::Folders(key.into()),
            key,
            ProjectChange::ReadFolders(folders),
        );
    }

    pub fn remove_entry(&mut self, key: &str) {
        self.write_project(Row::Entry(key.into()), key, ProjectChange::Remove);
    }

    /// One entry per `[projects]` key and per open project, `current`
    /// first and marked open.
    pub fn entries(&self, open: &[PathBuf], current: Option<&Path>) -> Vec<ProjectEntry> {
        let mut keys: Vec<String> = current
            .into_iter()
            .chain(open.iter().map(PathBuf::as_path))
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        keys.extend(self.settings.project_keys().into_iter().map(String::from));
        let mut seen = std::collections::HashSet::new();
        keys.retain(|key| seen.insert(key.clone()));
        keys.into_iter()
            .map(|key| {
                let mut notes: Vec<String> = cedian_shell::project_key_problem(&key)
                    .into_iter()
                    .collect();
                if notes.is_empty() {
                    let path = Path::new(&key);
                    let own = self
                        .settings
                        .policy_for(path, RunKind::Interactive)
                        .notes
                        .into_iter()
                        .chain(self.settings.pstack_for(path).notes)
                        .chain(self.settings.read_folders_for(path).notes)
                        .filter(|note| !note.starts_with("[projects."));
                    for note in own {
                        if !notes.contains(&note) {
                            notes.push(note);
                        }
                    }
                }
                ProjectEntry {
                    open: current.is_some_and(|current| current == Path::new(&key)),
                    policy: self.settings.project_policy(&key).unwrap_or_default(),
                    pstack: self.settings.project_pstack(&key),
                    read_folders: self.settings.project_read_folders(&key).to_vec(),
                    notes,
                    key,
                }
            })
            .collect()
    }

    pub fn row_state(&self, row: &Row) -> Option<RowState> {
        match &self.error {
            Some((at, error)) if at == row => Some(RowState::Error(error.clone().into())),
            _ => None,
        }
    }

    fn write_project(&mut self, row: Row, key: &str, change: ProjectChange) {
        self.write(row, |path| write_project(path, key, change));
    }

    fn write(&mut self, row: Row, write: impl FnOnce(&Path) -> Result<(), String>) {
        let Some(path) = self.path.clone() else {
            self.error = Some((
                row,
                "HOME is not set, so cedian has nowhere to keep this".into(),
            ));
            return;
        };
        match write(&path) {
            Ok(()) => *self = Self::load(Some(path)),
            Err(e) => self.error = Some((row, e)),
        }
    }
}

/// The open project's root and every open project's, for the pages.
#[derive(Debug, Clone, Default)]
pub struct OpenProjects {
    pub current: Option<PathBuf>,
    pub all: Vec<PathBuf>,
}

pub(crate) fn dropdown<T: Copy + PartialEq + 'static, V: 'static>(
    id: impl Into<SharedString>,
    current: T,
    options: Vec<(&'static str, T)>,
    disabled: bool,
    write: impl Fn(&mut V, T, &mut Context<V>) + Clone + 'static,
    window: &mut Window,
    cx: &mut Context<V>,
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
                    this.update(cx, |this, cx| {
                        write(this, value, cx);
                        cx.notify();
                    })
                    .ok();
                },
            );
        }
        menu
    });
    DropdownMenu::new(id.into(), label, menu)
        .style(ui::DropdownStyle::Outlined)
        .disabled(disabled)
        .into_any_element()
}

pub(crate) const POLICY_OPTIONS: [(&str, Policy); 2] = [
    ("cedian", Policy::Cedian),
    ("Use the agent's own config", Policy::Omp),
];

pub(crate) fn section(title: &'static str, rows: Vec<AnyElement>) -> Div {
    v_flex()
        .gap_3()
        .pb_6()
        .child(SettingsSectionHeader::new(title).no_padding(true))
        .children(
            rows.into_iter()
                .map(|row| v_flex().gap_3().child(row).child(Divider::horizontal())),
        )
}

pub(crate) fn muted(text: impl Into<SharedString>) -> AnyElement {
    Label::new(text.into())
        .size(LabelSize::Small)
        .color(Color::Muted)
        .into_any_element()
}

/// The control of an Agent policy row: the dropdown, and the warning in
/// red while the agent's own config is chosen.
pub(crate) fn policy_control<V: 'static>(
    id: impl Into<SharedString>,
    policy: Policy,
    write: impl Fn(&mut V, Policy, &mut Context<V>) + Clone + 'static,
    window: &mut Window,
    cx: &mut Context<V>,
) -> AnyElement {
    v_flex()
        .items_end()
        .gap_1()
        .child(dropdown(
            id,
            policy,
            POLICY_OPTIONS.to_vec(),
            false,
            write,
            window,
            cx,
        ))
        .when(policy == Policy::Omp, |this| {
            this.child(
                div().max_w(px(260.)).child(
                    Label::new(OMP_POLICY_WARNING)
                        .size(LabelSize::Small)
                        .color(Color::Error),
                ),
            )
        })
        .into_any_element()
}

/// The folder list of the project at `key`: one row per folder with
/// Remove, and Add folder.
pub(crate) fn folders_control<V: 'static>(
    id: &str,
    key: &str,
    folders: &[PathBuf],
    state: fn(&mut V) -> &mut ProjectSettings,
    cx: &mut Context<V>,
) -> AnyElement {
    let this = cx.entity().downgrade();
    let add = Button::new(SharedString::from(format!("{id}-add")), "Add folder")
        .style(ButtonStyle::OutlinedGhost)
        .on_click({
            let this = this.clone();
            let key = key.to_string();
            move |_, _, cx| {
                let picked = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Allow Reads".into()),
                });
                let this = this.clone();
                let key = key.clone();
                cx.spawn(async move |cx| {
                    let Some(folder) = picked
                        .await
                        .ok()
                        .and_then(|result| result.log_err())
                        .flatten()
                        .and_then(|paths| paths.into_iter().next())
                    else {
                        return;
                    };
                    this.update(cx, |this, cx| {
                        state(this).add_folder(&key, &folder);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        });
    v_flex()
        .items_end()
        .gap_1()
        .children(folders.iter().enumerate().map(|(index, folder)| {
            let this = this.clone();
            let key = key.to_string();
            let remove = folder.clone();
            h_flex()
                .gap_2()
                .child(muted(folder.display().to_string()))
                .child(
                    Button::new(SharedString::from(format!("{id}-remove-{index}")), "Remove")
                        .style(ButtonStyle::Subtle)
                        .on_click(move |_, _, cx| {
                            this.update(cx, |this, cx| {
                                state(this).remove_folder(&key, &remove);
                                cx.notify();
                            })
                            .ok();
                        }),
                )
        }))
        .child(add)
        .into_any_element()
}

pub(crate) fn folders_state(
    settings: &ProjectSettings,
    key: &str,
    folders: &[PathBuf],
) -> Option<RowState> {
    settings.row_state(&Row::Folders(key.into())).or_else(|| {
        folders.is_empty().then(|| RowState::Empty {
            text: "None. Every outside read asks.".into(),
        })
    })
}

fn tier_text(tier: Tier) -> (&'static str, &'static str) {
    match tier {
        Tier::Safe => ("Safe Actions", "Reading files, searching, listing."),
        Tier::ProjectWrite => (
            "Project Writes",
            "Editing and creating files inside the project, worktrees.",
        ),
        Tier::Dangerous => (
            "Dangerous Actions",
            "Shell commands, code evaluation, browser, subagents. Allow still asks for shell and eval under the cedian policy; cedian can deny what the agent allowed, never the reverse.",
        ),
    }
}

/// What a session's agent runs with under `policy` (ADR-0020, ADR-0035).
pub fn what_the_agent_sees(policy: Policy) -> [(&'static str, &'static str); 4] {
    match policy {
        Policy::Cedian => [
            ("Approval mode", "write"),
            ("Shell and eval", "ask"),
            ("Desktop control", "off"),
            ("Host tools", "allowed"),
        ],
        Policy::Omp => [
            ("Approval mode", "the agent's config"),
            ("Shell and eval", "the agent's config"),
            ("Desktop control", "the agent's config"),
            ("Host tools", "allowed"),
        ],
    }
}

pub const PROTECTED_PATHS: [&str; 4] = [
    "`.git`, `.cedian` and `.omp` in the project",
    "cedian's state for the project",
    "the agent's config folder",
    "`~/.omp`",
];

pub struct PermissionsPage {
    state: ProjectSettings,
    project: OpenProjects,
}

impl PermissionsPage {
    pub fn new(project: OpenProjects) -> Self {
        Self {
            state: ProjectSettings::load_from_env(),
            project,
        }
    }

    pub fn state(&self) -> &ProjectSettings {
        &self.state
    }

    fn state_mut(&mut self) -> &mut ProjectSettings {
        &mut self.state
    }

    fn tier_row(&self, tier: Tier, cx: &mut Context<Self>) -> AnyElement {
        let current = self.state.tier(tier);
        let (title, description) = tier_text(tier);
        let buttons = [
            ("Allow", Verdict::Allow),
            ("Ask", Verdict::Ask),
            ("Deny", Verdict::Deny),
        ]
        .map(|(label, verdict)| {
            Button::new(
                SharedString::from(format!("cedian-tier-{tier:?}-{label}")),
                label,
            )
            .style(if current == verdict {
                ButtonStyle::Filled
            } else {
                ButtonStyle::Subtle
            })
            .toggle_state(current == verdict)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.state.set_tier(tier, verdict);
                cx.notify();
            }))
        });
        render_row(
            SharedString::from(format!("cedian-tier-row-{tier:?}")),
            title.into(),
            Some(description.into()),
            Some(ScopeBadge::User),
            self.state.row_state(&Row::Tier(tier)).as_ref(),
            h_flex().gap_1().children(buttons).into_any_element(),
            cx,
        )
        .into_any_element()
    }

    fn audit_log(&self) -> Option<PathBuf> {
        let root = self.project.current.as_deref()?;
        Some(
            cedian_shell::state::dir(root)
                .ok()?
                .join(cedian_shell::audit::AUDIT_FILE),
        )
    }
}

impl Render for PermissionsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tiers = Tier::ALL
            .into_iter()
            .map(|tier| self.tier_row(tier, cx))
            .collect();

        let current = self.project.current.clone();
        let key = current
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned());
        let entry = current.as_deref().and_then(|current| {
            self.state
                .entries(std::slice::from_ref(&current.to_path_buf()), Some(current))
                .into_iter()
                .next()
        });
        let in_effect = current.as_deref().map_or(Policy::Cedian, |current| {
            self.state.policy_in_effect(current)
        });

        let no_project = || RowState::Unavailable("Open a project to set this.".into());
        let folders = match (&key, &entry) {
            (Some(key), Some(entry)) => (
                folders_control(
                    "cedian-read-folders",
                    key,
                    &entry.read_folders,
                    Self::state_mut,
                    cx,
                ),
                folders_state(&self.state, key, &entry.read_folders),
            ),
            _ => (div().into_any_element(), Some(no_project())),
        };
        let reading = vec![
            render_row(
                "cedian-reads-outside-row",
                "Reads Outside the Project Ask You".into(),
                Some("The agent reads inside the project freely. Any path outside asks in the approval dialog, and the answer is logged.".into()),
                Some(ScopeBadge::SetByCedian),
                None,
                ui::Switch::new("cedian-reads-outside", ui::ToggleState::Selected)
                    .disabled(true)
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
            render_row(
                "cedian-read-folders-row",
                "Folders the Agent May Read Without Asking".into(),
                Some("For this project only. Never read from a file the repository carries.".into()),
                Some(ScopeBadge::User),
                folders.1.as_ref(),
                folders.0,
                cx,
            )
            .into_any_element(),
            render_row(
                "cedian-protected-paths-row",
                "Protected Paths".into(),
                Some("Always denied. Not configurable.".into()),
                Some(ScopeBadge::SetByCedian),
                None,
                v_flex()
                    .items_end()
                    .children(PROTECTED_PATHS.map(muted))
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
        ];

        let policy = match (&key, &entry) {
            (Some(key), Some(entry)) => {
                let key = key.clone();
                (
                    policy_control(
                        "cedian-agent-policy",
                        entry.policy,
                        move |this: &mut Self, policy, _| this.state.set_policy(&key, policy),
                        window,
                        cx,
                    ),
                    self.state
                        .row_state(&Row::Policy(entry.key.clone()))
                        .or_else(|| {
                            (!entry.notes.is_empty())
                                .then(|| RowState::Error(entry.notes.join("; ").into()))
                        }),
                )
            }
            _ => (div().into_any_element(), Some(no_project())),
        };
        let audit = self.audit_log();
        let project = vec![
            render_row(
                "cedian-agent-policy-row",
                "Agent Policy".into(),
                Some("cedian: the tiers above and cedian's gate. Use the agent's own config: approvals and desktop control follow the agent's config. The panel shows a badge and every call is logged.".into()),
                Some(ScopeBadge::User),
                policy.1.as_ref(),
                policy.0,
                cx,
            )
            .into_any_element(),
            render_row(
                "cedian-agent-sees-row",
                "What the Agent Sees".into(),
                None,
                Some(ScopeBadge::SetByCedian),
                None,
                v_flex()
                    .items_end()
                    .children(
                        what_the_agent_sees(in_effect)
                            .map(|(what, value)| muted(format!("{what}: {value}"))),
                    )
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
            render_row(
                "cedian-approval-log-row",
                "Approval Log".into(),
                Some("Every approval and its source, kept in cedian's state for this project.".into()),
                None,
                None,
                Button::new("cedian-approval-log", "Open")
                    .style(ButtonStyle::OutlinedGhost)
                    .disabled(audit.as_ref().is_none_or(|path| !path.exists()))
                    .on_click(move |_, _, cx| {
                        if let Some(path) = &audit {
                            cx.open_with_system(path);
                        }
                    })
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
        ];

        v_flex()
            .w_full()
            .pt_2()
            .when(in_effect == Policy::Omp, |this| {
                this.child(
                    h_flex()
                        .mb_4()
                        .gap_2()
                        .child(Icon::new(IconName::Warning).color(Color::Warning))
                        .child(Label::new(
                            "This project uses the agent's own approvals (OMP policy).",
                        )),
                )
            })
            .when_some(self.state.load_error.clone(), |this, error| {
                this.child(
                    div()
                        .mb_4()
                        .child(Label::new(error).size(LabelSize::Small).color(Color::Error)),
                )
            })
            .child(section("Approval Tiers", tiers))
            .child(section("Reading Outside the Project", reading))
            .child(section("This Project", project))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        config: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().canonicalize().unwrap();
            Self {
                config: root.join("config/cedian.toml"),
                root,
                _dir: dir,
            }
        }

        fn project(&self, name: &str) -> PathBuf {
            let path = self.root.join(name);
            std::fs::create_dir_all(&path).unwrap();
            path
        }

        fn load(&self) -> ProjectSettings {
            ProjectSettings::load(Some(self.config.clone()))
        }
    }

    fn key(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn tiers_default_to_allow_allow_ask_and_write_to_the_user_file() {
        let fixture = Fixture::new();
        let mut state = fixture.load();
        assert_eq!(
            Tier::ALL.map(|tier| state.tier(tier)),
            [Verdict::Allow, Verdict::Allow, Verdict::Ask]
        );
        state.set_tier(Tier::Dangerous, Verdict::Deny);
        state.set_tier(Tier::Safe, Verdict::Ask);
        assert_eq!(state.error, None);
        assert_eq!(fixture.load(), state);
        assert_eq!(
            Tier::ALL.map(|tier| state.tier(tier)),
            [Verdict::Ask, Verdict::Allow, Verdict::Deny]
        );
    }

    #[test]
    fn each_project_keeps_its_own_policy_pstack_and_folders() {
        let fixture = Fixture::new();
        let one = fixture.project("one");
        let two = fixture.project("two");
        let docs = fixture.project("docs");
        let mut state = fixture.load();
        state.set_policy(&key(&one), Policy::Omp);
        state.set_pstack(&key(&one), Some(false));
        state.add_folder(&key(&two), &docs);
        state.add_folder(&key(&two), &docs);
        assert_eq!(state.error, None);

        let state = fixture.load();
        let entries = state.entries(std::slice::from_ref(&two), Some(&one));
        assert_eq!(entries.len(), 2);
        let (first, second) = (&entries[0], &entries[1]);
        assert!(first.open && !second.open, "the open project comes first");
        assert_eq!(
            (first.policy, first.pstack, first.read_folders.as_slice()),
            (Policy::Omp, Some(false), &[][..])
        );
        assert_eq!(
            (second.policy, second.pstack, second.read_folders.as_slice()),
            (Policy::Cedian, None, std::slice::from_ref(&docs)),
            "adding a folder twice keeps one"
        );
        assert_eq!(state.policy_in_effect(&one), Policy::Omp);
        assert_eq!(state.policy_in_effect(&two), Policy::Cedian);

        let mut state = state;
        state.remove_folder(&key(&two), &docs);
        state.remove_entry(&key(&one));
        assert!(state.entries(&[], None).is_empty());
        assert_eq!(
            std::fs::read_to_string(&fixture.config).unwrap(),
            "schema = 1\n"
        );
    }

    #[test]
    fn values_are_never_read_from_a_file_the_project_carries() {
        let fixture = Fixture::new();
        let repo = fixture.project("repo");
        let docs = fixture.project("docs");
        std::fs::write(
            repo.join("cedian.toml"),
            format!(
                "schema = 1\n[projects.{:?}]\npolicy = \"omp\"\nread_folders = [{:?}]\n",
                key(&repo),
                key(&docs)
            ),
        )
        .unwrap();
        let state = fixture.load();
        let entry = &state.entries(&[], Some(&repo))[0];
        assert_eq!(
            (entry.policy, entry.read_folders.len()),
            (Policy::Cedian, 0)
        );
        assert_eq!(state.policy_in_effect(&repo), Policy::Cedian);

        let mut inside = ProjectSettings::load(Some(repo.join("cedian.toml")));
        assert_eq!(
            inside.policy_in_effect(&repo),
            Policy::Cedian,
            "pointed at the project's own file, the opt-in still does not apply"
        );
        assert!(inside.settings().read_folders_for(&repo).folders.is_empty());
        inside.set_pstack(&key(&repo), Some(false));
        assert!(matches!(
            inside.row_state(&Row::Pstack(key(&repo))),
            Some(RowState::Error(text)) if text.contains("inside")
        ));
    }

    #[test]
    fn a_bad_key_shows_its_note_on_its_row_and_can_be_removed() {
        let fixture = Fixture::new();
        let open = fixture.project("open");
        std::fs::create_dir_all(fixture.config.parent().unwrap()).unwrap();
        std::fs::write(
            &fixture.config,
            "schema = 1\n[projects.\"relative/x\"]\npolicy = \"omp\"\n",
        )
        .unwrap();
        let mut state = fixture.load();
        let entries = state.entries(&[], Some(&open));
        assert!(entries[0].notes.is_empty(), "{:?}", entries[0].notes);
        assert_eq!(entries[1].key, "relative/x");
        assert_eq!(entries[1].notes, ["not an absolute path"]);
        state.remove_entry("relative/x");
        assert_eq!(state.error, None);
        assert_eq!(state.entries(&[], None), []);
    }

    #[test]
    fn an_unparseable_file_is_shown_and_never_overwritten() {
        let fixture = Fixture::new();
        let one = fixture.project("one");
        std::fs::create_dir_all(fixture.config.parent().unwrap()).unwrap();
        std::fs::write(&fixture.config, "schema = 1\nnot = valid = toml").unwrap();
        let mut state = fixture.load();
        assert!(state.load_error.is_some());
        state.set_policy(&key(&one), Policy::Omp);
        state.set_tier(Tier::Safe, Verdict::Deny);
        assert!(state.row_state(&Row::Tier(Tier::Safe)).is_some());
        assert_eq!(
            std::fs::read_to_string(&fixture.config).unwrap(),
            "schema = 1\nnot = valid = toml"
        );
    }

    #[test]
    fn locked_rows_are_set_by_cedian_and_the_omp_policy_names_its_risk() {
        assert_eq!(
            what_the_agent_sees(Policy::Cedian)[0],
            ("Approval mode", "write")
        );
        assert!(
            what_the_agent_sees(Policy::Omp)
                .iter()
                .any(|(_, value)| *value == "the agent's config")
        );
        assert!(
            OMP_POLICY_WARNING.contains("yolo") && OMP_POLICY_WARNING.contains("prompt-injected")
        );
        assert!(PROTECTED_PATHS.iter().any(|path| path.contains("~/.omp")));
    }
}
