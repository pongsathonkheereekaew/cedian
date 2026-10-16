//! cedian's arrangement of the settings window (Settings spec, SET-1): the
//! grouped sidebar with an icon per page, the pages cedian adds or rebuilds
//! (General, Notifications, Privacy, Advanced, About), the scope badges and
//! the row states every cedian row uses, and the host hooks through which
//! the app binds the agent panel to the window. Zed's own row design is
//! kept as is, so the window looks like one app.

use std::path::{Path, PathBuf};

use gpui::{AnyView, AnyWindowHandle, PromptLevel};
use settings::{FollowUp, TurnFinishedNotification};
use ui::Chip;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SidebarGroup {
    Personal,
    Agent,
    Workspace,
    System,
}

impl SidebarGroup {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Personal => "PERSONAL",
            Self::Agent => "AGENT",
            Self::Workspace => "WORKSPACE",
            Self::System => "SYSTEM",
        }
    }
}

pub(crate) struct SidebarPage {
    pub title: &'static str,
    pub icon: IconName,
    pub group: SidebarGroup,
    pub subtitle: Option<&'static str>,
    /// A cedian page resets all its rows at once; Zed pages keep Zed's
    /// per-row reset.
    pub restore_defaults: bool,
}

const fn page(
    title: &'static str,
    icon: IconName,
    group: SidebarGroup,
    subtitle: Option<&'static str>,
    restore_defaults: bool,
) -> SidebarPage {
    SidebarPage {
        title,
        icon,
        group,
        subtitle,
        restore_defaults,
    }
}

/// The sidebar, in order. A page the window builds that is not listed here
/// is not shown.
pub(crate) const SIDEBAR: &[SidebarPage] = {
    use SidebarGroup::*;
    &[
        page(
            "General",
            IconName::Settings,
            Personal,
            Some("Defaults for the app and for new sessions."),
            true,
        ),
        page(
            "Appearance",
            IconName::Image,
            Personal,
            Some("Theme, fonts and how agent conversations look."),
            false,
        ),
        page(
            "Notifications",
            IconName::Bell,
            Personal,
            Some("When cedian tells you the agent finished or needs you."),
            true,
        ),
        page(
            "Keyboard Shortcuts",
            IconName::Keyboard,
            Personal,
            Some("Change, add or remove the shortcut for every command."),
            false,
        ),
        page(
            "Account",
            IconName::Person,
            Agent,
            Some("Sign in to the providers the agent uses, and bring your setup from OMP."),
            false,
        ),
        page(
            "Models",
            IconName::Sparkle,
            Agent,
            Some("Which models the agent uses, by role."),
            false,
        ),
        page(
            "Methodology",
            IconName::Book,
            Agent,
            Some(
                "pstack, poteto's agent style: skills, playbooks and the mandate. Bundled with cedian; off runs the plain agent.",
            ),
            false,
        ),
        page(
            "Permissions",
            IconName::Lock,
            Agent,
            Some("What the agent may do without asking you."),
            false,
        ),
        page(
            "Projects",
            IconName::Folder,
            Workspace,
            Some("Per-project settings, stored on this computer and never in the repository."),
            false,
        ),
        page("Editor", IconName::Pencil, Workspace, None, false),
        page(
            "Languages & Tools",
            IconName::FileCode,
            Workspace,
            None,
            false,
        ),
        page(
            "Search & Files",
            IconName::MagnifyingGlass,
            Workspace,
            None,
            false,
        ),
        page("Window & Layout", IconName::Split, Workspace, None, false),
        page(
            "Panels",
            IconName::ThreadsSidebarRightOpen,
            Workspace,
            None,
            false,
        ),
        page("Terminal", IconName::Terminal, Workspace, None, false),
        page("Debugger", IconName::Debug, Workspace, None, false),
        page(
            "Version Control",
            IconName::GitCommit,
            Workspace,
            None,
            false,
        ),
        page("Network", IconName::Public, Workspace, None, false),
        page(
            "Privacy",
            IconName::EyeOff,
            System,
            Some("What leaves this computer. By default, nothing."),
            true,
        ),
        page(
            "Advanced",
            IconName::Code,
            System,
            Some("Everything else, including every agent setting."),
            false,
        ),
        page(
            "About",
            IconName::Info,
            System,
            Some("Versions, updates and licenses."),
            false,
        ),
    ]
};

/// Zed pages cedian does not show: they configure Zed's collaboration and
/// Zed's own agent, which cedian hides. Developer moves into Advanced.
pub(crate) const HIDDEN_PAGES: &[&str] = &["Collaboration", "AI", "Developer"];

pub(crate) fn sidebar_page(title: &str) -> Option<&'static SidebarPage> {
    SIDEBAR.iter().find(|page| page.title == title)
}

/// Where a row's value lives, shown as a chip after its label.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScopeBadge {
    /// One value per machine.
    User,
    /// Pinned by cedian's spawn overlay; read-only.
    SetByCedian,
    /// Lives in the agent's config, in the named layer.
    Agent(&'static str),
}

impl ScopeBadge {
    pub fn label(self) -> SharedString {
        match self {
            Self::User => "User".into(),
            Self::SetByCedian => "Set by cedian".into(),
            Self::Agent(layer) => format!("Agent · {layer}").into(),
        }
    }
}

pub fn render_scope_badge(badge: ScopeBadge, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let chip = Chip::new(badge.label())
        .label_size(LabelSize::XSmall)
        .height(px(18.));
    match badge {
        ScopeBadge::SetByCedian => chip
            .label_color(Color::Accent)
            .border_color(colors.border_focused.opacity(0.6))
            .icon(IconName::Lock)
            .icon_color(Color::Accent),
        _ => chip.label_color(Color::Muted),
    }
    .into_any_element()
}

/// The fixed vocabulary of row states (spec, Shell > States).
#[derive(Clone, PartialEq, Debug)]
pub enum RowState {
    /// The value is being read; the control is disabled.
    Loading,
    /// A list with nothing in it: one line and the primary action's label.
    Empty { text: SharedString },
    /// The last read or write failed; the control stays enabled to retry.
    Error(SharedString),
    /// Something the row needs is missing; the text says what.
    Unavailable(SharedString),
    /// The agent the row needs is not running; the control starts it, so it
    /// stays enabled.
    NotRunning(SharedString),
    /// The parent switch is off: dimmed, control disabled, no text.
    DisabledChild,
}

impl RowState {
    pub fn disables_control(&self) -> bool {
        matches!(
            self,
            Self::Loading | Self::DisabledChild | Self::Unavailable(_)
        )
    }
}

/// The line under a row for `state`, if the state has one.
pub fn render_row_state(state: &RowState, cx: &App) -> Option<AnyElement> {
    let line = |icon: IconName, text: SharedString, color: Color| {
        h_flex()
            .gap_1()
            .pt_1()
            .child(Icon::new(icon).size(IconSize::XSmall).color(color))
            .child(Label::new(text).size(LabelSize::Small).color(color))
            .into_any_element()
    };
    match state {
        RowState::Loading => Some(line(IconName::ArrowCircle, "Loading…".into(), Color::Muted)),
        RowState::Empty { text } => Some(
            div()
                .mt_1()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_dashed()
                .border_color(cx.theme().colors().border_variant)
                .child(
                    Label::new(text.clone())
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .into_any_element(),
        ),
        RowState::Error(text) => Some(line(IconName::XCircle, text.clone(), Color::Error)),
        RowState::Unavailable(text) | RowState::NotRunning(text) => {
            Some(line(IconName::Warning, text.clone(), Color::Warning))
        }
        RowState::DisabledChild => None,
    }
}

/// A cedian row that is not a Zed setting: read-only information, a pinned
/// value, or an action.
pub(crate) struct CedianRow {
    pub title: &'static str,
    pub description: &'static str,
    pub badge: Option<ScopeBadge>,
    pub control: fn(&SettingsWindow, &mut Window, &mut Context<SettingsWindow>) -> AnyElement,
    pub state: Option<fn(&App) -> Option<RowState>>,
}

impl PartialEq for CedianRow {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
    }
}

pub(crate) fn render_cedian_row(
    row: &CedianRow,
    settings_window: &SettingsWindow,
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> Stateful<Div> {
    let state = row.state.and_then(|state| state(cx));
    let control = (row.control)(settings_window, window, cx);
    render_row(
        row.title,
        row.title.into(),
        (!row.description.is_empty()).then(|| row.description.into()),
        row.badge,
        state.as_ref(),
        control,
        cx,
    )
}

/// One row as every cedian row looks: title with its scope badge, the muted
/// description, the state line under them, and the control on the right,
/// dimmed while the state disables it.
pub fn render_row(
    id: impl Into<ElementId>,
    title: SharedString,
    description: Option<SharedString>,
    badge: Option<ScopeBadge>,
    state: Option<&RowState>,
    control: AnyElement,
    cx: &App,
) -> Stateful<Div> {
    let dimmed = state.is_some_and(RowState::disables_control);
    h_flex()
        .id(id)
        .min_w_0()
        .justify_between()
        .gap_4()
        .child(
            v_flex()
                .w_full()
                .max_w_2_3()
                .min_w_0()
                .child(
                    h_flex()
                        .gap_1p5()
                        .child(Label::new(title))
                        .when_some(badge, |this, badge| {
                            this.child(render_scope_badge(badge, cx))
                        }),
                )
                .when_some(description, |this, description| {
                    this.child(
                        Label::new(description)
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .render_code_spans(),
                    )
                })
                .when_some(
                    state.and_then(|state| render_row_state(state, cx)),
                    |this, line| this.child(line),
                ),
        )
        .child(
            div()
                .flex_none()
                .when(dimmed, |this| this.opacity(0.5))
                .child(control),
        )
}

fn row(
    title: &'static str,
    description: &'static str,
    badge: Option<ScopeBadge>,
    control: fn(&SettingsWindow, &mut Window, &mut Context<SettingsWindow>) -> AnyElement,
) -> SettingsPageItem {
    SettingsPageItem::CedianRow(CedianRow {
        title,
        description,
        badge,
        control,
        state: None,
    })
}

fn value_text(text: impl Into<SharedString>) -> AnyElement {
    Label::new(text.into())
        .size(LabelSize::Small)
        .color(Color::Muted)
        .into_any_element()
}

fn user(metadata: Option<Box<SettingsFieldMetadata>>) -> Option<Box<SettingsFieldMetadata>> {
    let mut metadata = metadata.unwrap_or_default();
    metadata.badge = Some(ScopeBadge::User);
    Some(metadata)
}

/// The app binds the agent panel to the window through these; a window with
/// no host (tests, a build without the panel) shows the rows as unavailable.
#[derive(Clone, Copy)]
pub struct CedianHost {
    /// The agent's settings view for the workspace window that opened
    /// Settings, built for `window`.
    pub agent_settings: fn(Option<AnyWindowHandle>, &mut Window, &mut App) -> Option<AnyView>,
    /// Restart the agent of the workspace window that opened Settings.
    pub restart_agent: fn(Option<AnyWindowHandle>, &mut App),
    /// The Account or Models page for the workspace window that opened
    /// Settings, built for `window`.
    pub agent_page:
        fn(AgentPage, Option<AnyWindowHandle>, &mut Window, &mut App) -> Option<AnyView>,
}

/// A page the agent panel draws, in Settings' row style.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AgentPage {
    Account,
    Models,
    Methodology,
    Permissions,
    Projects,
}

/// A page item the host draws: its rows change with the agent (one per
/// provider, one per role), so they are not fixed `CedianRow`s.
#[derive(PartialEq)]
pub(crate) struct CedianView {
    pub title: &'static str,
    pub keywords: &'static str,
    pub page: AgentPage,
}

pub(crate) fn agent_page_view(
    page: AgentPage,
    origin: Option<AnyWindowHandle>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyView> {
    (host(cx)?.agent_page)(page, origin, window, cx)
}

pub(crate) fn render_cedian_view(
    view: &CedianView,
    settings_window: &SettingsWindow,
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let origin = settings_window.original_window.map(Into::into);
    match agent_page_view(view.page, origin, window, cx) {
        Some(view) => div().w_full().child(view).into_any_element(),
        None => render_row_state(
            &RowState::Unavailable(
                "Open a project with the agent panel to change the agent's account and models."
                    .into(),
            ),
            cx,
        )
        .unwrap_or_else(|| div().into_any_element()),
    }
}

impl Global for CedianHost {}

pub fn set_host(host: CedianHost, cx: &mut App) {
    cx.set_global(host);
}

fn host(cx: &App) -> Option<CedianHost> {
    cx.try_global::<CedianHost>().copied()
}

/// Rebuild Zed's pages into cedian's: move rows between pages, add the
/// cedian pages, drop the hidden ones and order the rest as the sidebar
/// lists them.
pub(crate) fn arrange(zed: Vec<SettingsPage>) -> Vec<SettingsPage> {
    let mut by_title: HashMap<&'static str, Vec<SettingsPageItem>> = zed
        .into_iter()
        .map(|page| (page.title, page.items.into_vec()))
        .collect();
    let mut zed_general = by_title.remove("General").unwrap_or_default();
    let developer = by_title.remove("Developer").unwrap_or_default();

    let accessible = take_item(&mut zed_general, "Accessible Mode");
    let restore = take_item(&mut zed_general, "Restore On Startup");
    let redact = take_item(&mut zed_general, "Redact Private Values");
    let private_files = take_item(&mut zed_general, "Private Files");
    for gone in [
        "Trust All Projects By Default",
        "Telemetry Diagnostics",
        "Telemetry Metrics",
        "Anthropic Data Retention",
        "Auto Update",
    ] {
        take_item(&mut zed_general, gone);
    }
    zed_general.retain(|item| !matches!(item, SettingsPageItem::SectionHeader("General Settings")));
    drop_empty_sections(&mut zed_general);

    let mut general = vec![
        SettingsPageItem::SectionHeader("App"),
        restricted_mode_item(),
    ];
    general.extend(restore);
    general.push(confirm_quit_item());
    general.extend(accessible);
    general.extend(zed_general);
    general.extend(composer_items());
    by_title.insert("General", general);

    by_title.insert(
        "Account",
        vec![SettingsPageItem::CedianView(CedianView {
            title: "Providers",
            keywords: "account sign in log in log out login logout api key import omp skills",
            page: AgentPage::Account,
        })],
    );
    by_title.insert(
        "Models",
        vec![SettingsPageItem::CedianView(CedianView {
            title: "Model Roles",
            keywords: "default model thinking level roles smol slow plan advisor",
            page: AgentPage::Models,
        })],
    );
    by_title.insert(
        "Methodology",
        vec![SettingsPageItem::CedianView(CedianView {
            title: "pstack",
            keywords: "pstack poteto methodology mandate skills playbooks effort reasoning role models feature refactoring bug fix judgment prose perf hillclimb version",
            page: AgentPage::Methodology,
        })],
    );
    by_title.insert(
        "Permissions",
        vec![SettingsPageItem::CedianView(CedianView {
            title: "Permissions",
            keywords: "permissions approval tiers allow ask deny safe project writes dangerous read outside folders protected paths agent policy omp audit log",
            page: AgentPage::Permissions,
        })],
    );
    by_title.insert(
        "Projects",
        vec![SettingsPageItem::CedianView(CedianView {
            title: "Projects",
            keywords: "projects project path remove entry agent policy omp pstack inherit read folders",
            page: AgentPage::Projects,
        })],
    );
    by_title.insert("Notifications", notification_items());
    by_title.insert(
        "Privacy",
        privacy_items(redact.into_iter().chain(private_files)),
    );
    let mut advanced = advanced_items();
    advanced.extend(developer);
    by_title.insert("Advanced", advanced);
    by_title.insert("About", about_items());

    SIDEBAR
        .iter()
        .filter(|page| !HIDDEN_PAGES.contains(&page.title))
        .filter_map(|page| {
            let items = by_title.remove(page.title)?;
            Some(SettingsPage {
                title: page.title,
                items: items.into_boxed_slice(),
            })
        })
        .collect()
}

fn take_item(items: &mut Vec<SettingsPageItem>, title: &str) -> Option<SettingsPageItem> {
    let index = items.iter().position(|item| match item {
        SettingsPageItem::SettingItem(item) => item.title == title,
        SettingsPageItem::DynamicItem(item) => item.discriminant.title == title,
        _ => false,
    })?;
    let mut item = items.remove(index);
    if let SettingsPageItem::SettingItem(setting) = &mut item {
        setting.metadata = user(setting.metadata.take());
    }
    Some(item)
}

fn drop_empty_sections(items: &mut Vec<SettingsPageItem>) {
    let mut keep = vec![true; items.len()];
    for (index, item) in items.iter().enumerate() {
        let is_header = matches!(item, SettingsPageItem::SectionHeader(_));
        let next_is_header_or_end = items
            .get(index + 1)
            .is_none_or(|next| matches!(next, SettingsPageItem::SectionHeader(_)));
        if is_header && next_is_header_or_end {
            keep[index] = false;
        }
    }
    let mut keep = keep.into_iter();
    items.retain(|_| keep.next().unwrap_or(true));
}

fn restricted_mode_item() -> SettingsPageItem {
    SettingsPageItem::SettingItem(SettingItem {
        title: "Open Projects in Restricted Mode",
        description: "Ask before trusting a project you have not opened before. Restricted Mode keeps language servers and project tasks off until you trust it.",
        field: Box::new(SettingField {
            organization_override: None,
            json_path: Some("session.trust_all_worktrees"),
            pick: |content| {
                content
                    .session
                    .as_ref()
                    .and_then(|session| session.trust_all_worktrees.as_ref())
                    .map(|trust_all| if *trust_all { &false } else { &true })
            },
            write: |content, restricted, _| {
                content.session.get_or_insert_default().trust_all_worktrees =
                    restricted.map(|restricted| !restricted);
            },
        }),
        metadata: user(None),
        files: USER,
    })
}

fn confirm_quit_item() -> SettingsPageItem {
    SettingsPageItem::SettingItem(SettingItem {
        title: "Confirm Before Quitting",
        description: "Ask before a quit shortcut closes cedian's windows.",
        field: Box::new(SettingField {
            organization_override: None,
            json_path: Some("confirm_quit"),
            pick: |content| content.workspace.confirm_quit.as_ref(),
            write: |content, value, _| content.workspace.confirm_quit = value,
        }),
        metadata: user(None),
        files: USER,
    })
}

fn composer_items() -> Vec<SettingsPageItem> {
    vec![
        SettingsPageItem::SectionHeader("Composer"),
        SettingsPageItem::SettingItem(SettingItem {
            title: "Messages Sent While the Agent Works",
            description: "Queue them for after the current turn, or steer: stop the turn and send now. ⌘ Enter steers once.",
            field: Box::new(SettingField::<FollowUp> {
                organization_override: None,
                json_path: Some("cedian.follow_up"),
                pick: |content| content.cedian.as_ref()?.follow_up.as_ref(),
                write: |content, value, _| content.cedian.get_or_insert_default().follow_up = value,
            }),
            metadata: user(None),
            files: USER,
        }),
    ]
}

fn notification_items() -> Vec<SettingsPageItem> {
    fn switch(
        title: &'static str,
        description: &'static str,
        json_path: &'static str,
        pick: fn(&SettingsContent) -> Option<&bool>,
        write: fn(&mut SettingsContent, Option<bool>, &App),
    ) -> SettingsPageItem {
        SettingsPageItem::SettingItem(SettingItem {
            title,
            description,
            field: Box::new(SettingField {
                organization_override: None,
                json_path: Some(json_path),
                pick,
                write,
            }),
            metadata: user(None),
            files: USER,
        })
    }
    vec![
        SettingsPageItem::SectionHeader("In the App"),
        SettingsPageItem::SettingItem(SettingItem {
            title: "Turn Finished",
            description: "Tell you when a turn ends.",
            field: Box::new(SettingField::<TurnFinishedNotification> {
                organization_override: None,
                json_path: Some("cedian.notify_turn_finished"),
                pick: |content| content.cedian.as_ref()?.notify_turn_finished.as_ref(),
                write: |content, value, _| {
                    content.cedian.get_or_insert_default().notify_turn_finished = value
                },
            }),
            metadata: user(Some(Box::new(SettingsFieldMetadata {
                should_do_titlecase: Some(false),
                ..Default::default()
            }))),
            files: USER,
        }),
        switch(
            "Approval Needed",
            "Tell you when the agent is waiting for an approval.",
            "cedian.notify_approval_needed",
            |content| content.cedian.as_ref()?.notify_approval_needed.as_ref(),
            |content, value, _| {
                content
                    .cedian
                    .get_or_insert_default()
                    .notify_approval_needed = value
            },
        ),
        switch(
            "Question Asked",
            "Tell you when the agent asks you something.",
            "cedian.notify_question_asked",
            |content| content.cedian.as_ref()?.notify_question_asked.as_ref(),
            |content, value, _| {
                content.cedian.get_or_insert_default().notify_question_asked = value
            },
        ),
        SettingsPageItem::SectionHeader("Dock and System"),
        switch(
            "Bounce the Dock Icon",
            "Draw attention when cedian is in the background.",
            "cedian.bounce_dock_icon",
            |content| content.cedian.as_ref()?.bounce_dock_icon.as_ref(),
            |content, value, _| content.cedian.get_or_insert_default().bounce_dock_icon = value,
        ),
        SettingsPageItem::SettingItem(SettingItem {
            title: "Sound",
            description: "Play a sound with notifications.",
            field: Box::new(SettingField::<settings::NotificationSound> {
                organization_override: None,
                json_path: Some("cedian.notification_sound"),
                pick: |content| content.cedian.as_ref()?.notification_sound.as_ref(),
                write: |content, value, _| {
                    content.cedian.get_or_insert_default().notification_sound = value
                },
            }),
            metadata: user(Some(Box::new(SettingsFieldMetadata {
                should_do_titlecase: Some(false),
                ..Default::default()
            }))),
            files: USER,
        }),
        SettingsPageItem::CedianRow(CedianRow {
            title: "Desktop Notifications",
            description: "Use the macOS notification center when cedian is in the background.",
            badge: None,
            control: |_, _, _| {
                Switch::new("cedian-desktop-notifications", ToggleState::Unselected)
                    .disabled(true)
                    .into_any_element()
            },
            state: Some(|_| {
                Some(RowState::Unavailable(
                    "Not available yet: needs a signed build (unverified).".into(),
                ))
            }),
        }),
    ]
}

/// The one line Privacy shows for Zed's telemetry, which cedian turns off
/// in code (`client::TelemetrySettings`).
pub(crate) const TELEMETRY_OFF: &str = "Crash reports and usage metrics are off in code. cedian never sends them to Zed Industries' servers, and no setting turns them on.";

fn privacy_items(redaction: impl IntoIterator<Item = SettingsPageItem>) -> Vec<SettingsPageItem> {
    let mut items = vec![
        SettingsPageItem::SectionHeader("What Is Sent"),
        row(
            "Crash Reports and Usage Metrics",
            TELEMETRY_OFF,
            None,
            |_, _, _| value_text("Off"),
        ),
    ];
    items.extend((0..cedian_omp::PRIVACY_PINS.len()).map(pin_row));
    items.push(SettingsPageItem::SectionHeader("Redaction"));
    items.extend(redaction);
    items.push(SettingsPageItem::SectionHeader("Your Data"));
    items.extend([
        row(
            "Settings",
            "cedian.toml and the agent's config.",
            None,
            |_, _, _| open_button("open-settings-dir", data_dir(DataDir::Settings)),
        ),
        row(
            "Agent State",
            "Sessions, logs and sign-in tokens the agent writes.",
            None,
            |_, _, _| open_button("open-agent-state", data_dir(DataDir::AgentState)),
        ),
        row(
            "App Settings",
            "The editor's settings, keymap and themes.",
            None,
            |_, _, _| open_button("open-app-config", Some(paths::config_dir().clone())),
        ),
    ]);
    items
}

fn pin_row(index: usize) -> SettingsPageItem {
    let pin = &cedian_omp::PRIVACY_PINS[index];
    SettingsPageItem::CedianRow(CedianRow {
        title: pin.title,
        description: pin.description,
        badge: Some(ScopeBadge::SetByCedian),
        control: |_, _, _| value_text("Off"),
        state: None,
    })
}

enum DataDir {
    Settings,
    AgentState,
}

fn data_dir(which: DataDir) -> Option<PathBuf> {
    let dirs = cedian_omp::agent_dirs::AgentDirs::from_env().ok()?;
    match which {
        DataDir::Settings => dirs.config.parent().map(Path::to_path_buf),
        DataDir::AgentState => Some(dirs.state),
    }
}

fn open_button(id: &'static str, path: Option<PathBuf>) -> AnyElement {
    h_flex()
        .gap_2()
        .when_some(path.clone(), |this, path| {
            let shown = util::paths::home_dir()
                .as_path()
                .to_str()
                .and_then(|home| path.to_str().map(|path| path.replacen(home, "~", 1)))
                .unwrap_or_else(|| path.display().to_string());
            this.child(
                div()
                    .id(SharedString::from(format!("{id}-path")))
                    .tooltip(Tooltip::text(shown.clone()))
                    .child(value_text(truncate_middle(&shown, PATH_CHARS))),
            )
        })
        .child(
            Button::new(id, "Open")
                .style(ButtonStyle::OutlinedGhost)
                .size(ButtonSize::Medium)
                .disabled(path.is_none())
                .on_click(move |_, _, cx| {
                    if let Some(path) = &path {
                        std::fs::create_dir_all(path).log_err();
                        cx.open_with_system(path);
                    }
                }),
        )
        .into_any_element()
}

/// A path longer than this squeezes the row's description into lines.
const PATH_CHARS: usize = 32;

/// `text` cut to `max` characters by dropping its middle, so a path keeps
/// its root and its last folder.
fn truncate_middle(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let kept = max.saturating_sub(1);
    let head = kept / 2;
    let tail = kept - head;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(count - tail).collect();
    format!("{start}…{end}")
}

fn cedian_toml_path() -> Option<PathBuf> {
    Some(data_dir(DataDir::Settings)?.join("cedian.toml"))
}

fn advanced_items() -> Vec<SettingsPageItem> {
    vec![
        SettingsPageItem::SectionHeader("Agent"),
        SettingsPageItem::SubPageLink(SubPageLink {
            title: "All Agent Settings".into(),
            r#type: SubPageType::default(),
            description: Some(
                "Every key the agent exposes, with its value and layer. Scalars edit in place."
                    .into(),
            ),
            search_aliases: &["omp", "config", "agent", "model roles"],
            json_path: None,
            in_json: false,
            files: USER,
            render: render_agent_settings_page,
        }),
        SettingsPageItem::CedianRow(CedianRow {
            title: "Restart Agent",
            description: "Stop the agent of the open project and start it again. A running turn ends.",
            badge: None,
            control: |_, _, cx| {
                Button::new("cedian-restart-agent", "Restart")
                    .style(ButtonStyle::OutlinedGhost)
                    .size(ButtonSize::Medium)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(host) = host(cx) {
                            (host.restart_agent)(this.original_window.map(Into::into), cx);
                        }
                    }))
                    .into_any_element()
            },
            state: Some(|cx| {
                host(cx)
                    .is_none()
                    .then(|| RowState::Unavailable("The agent panel is not loaded.".into()))
            }),
        }),
        SettingsPageItem::SectionHeader("Files"),
        row(
            "Open cedian.toml",
            "Escape hatch. Every setting has a row in this window; an unknown key fails the whole file.",
            Some(ScopeBadge::User),
            |_, _, cx| {
                Button::new("cedian-open-toml", "Open")
                    .style(ButtonStyle::OutlinedGhost)
                    .size(ButtonSize::Medium)
                    .end_icon(Icon::new(IconName::ArrowUpRight).size(IconSize::Small))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(path) = cedian_toml_path() {
                            this.open_in_workspace(path, window, cx);
                        }
                    }))
                    .into_any_element()
            },
        ),
        row(
            "Open Agent Config Folder",
            "The agent's config.yml, models.yml, skills and rules.",
            Some(ScopeBadge::Agent("global")),
            |_, _, _| {
                open_button(
                    "cedian-open-agent-config",
                    cedian_omp::agent_dirs::AgentDirs::from_env()
                        .ok()
                        .map(|dirs| dirs.config),
                )
            },
        ),
    ]
}

fn render_agent_settings_page(
    settings_window: &SettingsWindow,
    _: &ScrollHandle,
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let origin = settings_window.original_window.map(Into::into);
    let view = host(cx).and_then(|host| (host.agent_settings)(origin, window, cx));
    match view {
        Some(view) => div().size_full().px_8().child(view).into_any_element(),
        None => v_flex()
            .px_8()
            .child(
                render_row_state(
                    &RowState::Unavailable(
                        "Open a project with the agent panel to edit the agent's settings.".into(),
                    ),
                    cx,
                )
                .unwrap_or_else(|| div().into_any_element()),
            )
            .into_any_element(),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AgentPin {
    pub version: String,
    pub commit: String,
}

/// The bundled agent's pin (ADR-0075), from the file the build reads.
pub(crate) fn agent_pin() -> Option<AgentPin> {
    let pin: serde_json::Value =
        serde_json::from_str(include_str!("../../../vendor/omp-revision.json")).ok()?;
    Some(AgentPin {
        version: pin["upstream"]["version"].as_str()?.to_string(),
        commit: pin["commit"].as_str()?.to_string(),
    })
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(10)]
}

fn cedian_version(cx: &App) -> String {
    let channel = ReleaseChannel::try_global(cx)
        .map(|channel| channel.display_name())
        .unwrap_or("Dev");
    let commit = release_channel::AppCommitSha::try_global(cx)
        .map(|sha| short(&sha.full()).to_string())
        .unwrap_or_else(|| "unknown commit".to_string());
    format!("pre-release · {channel} · {commit}")
}

fn zed_version(cx: &App) -> String {
    release_channel::AppVersion::global(cx).to_string()
}

fn agent_version(_: &App) -> String {
    agent_pin()
        .map(|pin| format!("{} · fork {}", pin.version, short(&pin.commit)))
        .unwrap_or_else(|| "unknown".to_string())
}

/// `0.15.24 · 1e5c90a4b6 · Lauren Tan, MIT`, for About and Methodology.
pub fn pstack_version() -> String {
    cedian_shell::pstack::pin()
        .map(|pin| {
            format!(
                "{} · {} · {}, {}",
                pin.version,
                short(&pin.commit),
                pin.author,
                pin.license
            )
        })
        .unwrap_or_else(|error| {
            log::error!("reading the pstack pin: {error}");
            "unknown".to_string()
        })
}

pub(crate) fn about_lines(cx: &App) -> Vec<(&'static str, String)> {
    vec![
        ("cedian", cedian_version(cx)),
        ("Zed", zed_version(cx)),
        ("Agent", agent_version(cx)),
        ("pstack", pstack_version()),
    ]
}

fn about_items() -> Vec<SettingsPageItem> {
    vec![
        SettingsPageItem::SectionHeader("Versions"),
        row(
            "cedian",
            "Version and the commit it was built from.",
            None,
            |_, _, cx| value_text(cedian_version(cx)),
        ),
        row(
            "Zed",
            "The Zed version cedian is built on.",
            None,
            |_, _, cx| value_text(zed_version(cx)),
        ),
        row(
            "Agent",
            "The bundled agent's version and its fork commit.",
            None,
            |_, _, cx| value_text(agent_version(cx)),
        ),
        row(
            "pstack",
            "Lauren Tan's agent style and skills.",
            None,
            |_, _, _| value_text(pstack_version()),
        ),
        row(
            "Copy All",
            "Every line above, for a bug report.",
            None,
            |_, _, _| {
                Button::new("cedian-copy-versions", "Copy")
                    .style(ButtonStyle::OutlinedGhost)
                    .size(ButtonSize::Medium)
                    .on_click(|_, _, cx| {
                        let text = about_lines(cx)
                            .into_iter()
                            .map(|(name, value)| format!("{name}: {value}"))
                            .collect::<Vec<_>>()
                            .join("\n");
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    })
                    .into_any_element()
            },
        ),
        SettingsPageItem::SectionHeader("Licenses"),
        row(
            "Licenses",
            "cedian and Zed: GPL-3.0. The agent (OMP): MIT. pstack by Lauren Tan: MIT. Third-party notices are listed in full.",
            None,
            |_, _, _| {
                Button::new("cedian-licenses", "View")
                    .style(ButtonStyle::OutlinedGhost)
                    .size(ButtonSize::Medium)
                    .on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(zed_actions::OpenLicenses), cx)
                    })
                    .into_any_element()
            },
        ),
    ]
}

impl SettingsWindow {
    pub(crate) fn open_in_workspace(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(original_window) = self.original_window else {
            return;
        };
        original_window
            .update(cx, |multi_workspace, window, cx| {
                multi_workspace.workspace().update(cx, |workspace, cx| {
                    workspace
                        .open_abs_path(path, OpenOptions::default(), window, cx)
                        .detach_and_log_err(cx);
                });
                window.activate_window();
            })
            .log_err();
        window.remove_window();
    }

    /// Reset every row of the current page to its default, after a confirm
    /// that names the page and its rows.
    pub(crate) fn restore_page_defaults(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = self.current_page();
        let title = page.title;
        let rows: Vec<&'static str> = page
            .items
            .iter()
            .filter_map(|item| match item {
                SettingsPageItem::SettingItem(item) => Some(item.title),
                SettingsPageItem::DynamicItem(item) => Some(item.discriminant.title),
                _ => None,
            })
            .collect();
        let detail = format!("Resets: {}.", rows.join(", "));
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Restore the defaults of {title}?"),
            Some(&detail),
            &["Restore Defaults", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if answer.await.ok() != Some(0) {
                return;
            }
            this.update_in(cx, |this, window, cx| {
                this.reset_current_page(window, cx);
            })
            .log_err();
        })
        .detach();
    }

    /// What Restore defaults resets on the current page: one reset per row
    /// whose value differs from its default in the current file.
    pub(crate) fn current_page_resets(&self, cx: &App) -> Vec<Box<dyn Fn(&mut Window, &mut App)>> {
        let file = self.current_file.clone();
        self.current_page()
            .items
            .iter()
            .filter_map(|item| match item {
                SettingsPageItem::SettingItem(item) => Some(item),
                SettingsPageItem::DynamicItem(item) => Some(&item.discriminant),
                _ => None,
            })
            .filter_map(|item| {
                let (set_in, _) = item.field.file_set_in(file.clone(), cx);
                item.field.reset_to_default_fn(&file, &set_in, cx)
            })
            .collect()
    }

    pub(crate) fn reset_current_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for reset in self.current_page_resets(cx) {
            reset(window, cx);
        }
        cx.notify();
    }
}

pub(crate) fn render_page_title(
    title: &'static str,
    window_entity: Entity<SettingsWindow>,
) -> AnyElement {
    let entry = sidebar_page(title);
    h_flex()
        .w_full()
        .mt_2()
        .mb_3()
        .justify_between()
        .items_start()
        .child(
            v_flex()
                .gap_0p5()
                .child(Label::new(title).size(LabelSize::Large))
                .when_some(entry.and_then(|entry| entry.subtitle), |this, subtitle| {
                    this.child(
                        Label::new(subtitle)
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                }),
        )
        .when(entry.is_some_and(|entry| entry.restore_defaults), |this| {
            this.child(
                Button::new("cedian-restore-defaults", "Restore Defaults")
                    .style(ButtonStyle::OutlinedGhost)
                    .size(ButtonSize::Medium)
                    .start_icon(Icon::new(IconName::Undo).size(IconSize::Small))
                    .on_click(move |_, window, cx| {
                        window_entity.update(cx, |this, cx| {
                            this.restore_page_defaults(window, cx);
                        });
                    }),
            )
        })
        .into_any_element()
}

/// A row in the navigation list: a group heading or a page or section.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NavRow {
    Group(SidebarGroup),
    Entry(usize),
}

impl SettingsWindow {
    /// The navigation list as drawn: each visible entry, with a group
    /// heading before the first root of each sidebar group.
    pub(crate) fn nav_rows(&self) -> Vec<NavRow> {
        let mut rows = Vec::new();
        let mut group = None;
        for (index, entry) in self.visible_navbar_entries() {
            if entry.is_root {
                let page_group =
                    sidebar_page(self.pages[entry.page_index].title).map(|page| page.group);
                if page_group.is_some() && page_group != group && !self.has_query {
                    rows.push(NavRow::Group(page_group.expect("checked")));
                }
                group = page_group;
            }
            rows.push(NavRow::Entry(index));
        }
        rows
    }

    pub(crate) fn nav_row_of(&self, entry_index: usize) -> Option<usize> {
        self.nav_rows()
            .iter()
            .position(|row| *row == NavRow::Entry(entry_index))
    }
}

pub(crate) fn render_group_heading(group: SidebarGroup) -> AnyElement {
    h_flex()
        .id(group.label())
        .h(rems_from_px(28_f32))
        .px_1p5()
        .pt_2()
        .items_end()
        .child(
            Label::new(group.label())
                .size(LabelSize::XSmall)
                .color(Color::Muted),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::register_settings;
    use gpui::TestAppContext;

    fn open_window(
        cx: &mut TestAppContext,
    ) -> (Entity<SettingsWindow>, &mut gpui::VisualTestContext) {
        cx.update(|cx| {
            register_settings(cx);
            let app_state = AppState::test(cx);
            AppState::set_global(app_state, cx);
        });
        let (window, cx) = cx.add_window_view(|window, cx| SettingsWindow::new(None, window, cx));
        cx.run_until_parked();
        (window, cx)
    }

    fn page<'a>(window: &'a SettingsWindow, title: &str) -> &'a SettingsPage {
        window
            .pages
            .iter()
            .find(|page| page.title == title)
            .unwrap_or_else(|| panic!("no {title} page"))
    }

    fn setting<'a>(page: &'a SettingsPage, title: &str) -> &'a SettingItem {
        page.items
            .iter()
            .find_map(|item| match item {
                SettingsPageItem::SettingItem(item) if item.title == title => Some(item),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no {title} row on {}", page.title))
    }

    fn field<T: 'static>(item: &SettingItem) -> SettingField<T> {
        *item
            .field
            .as_any()
            .downcast_ref::<SettingField<T>>()
            .expect("field type")
    }

    fn cedian_row<'a>(page: &'a SettingsPage, title: &str) -> &'a CedianRow {
        page.items
            .iter()
            .find_map(|item| match item {
                SettingsPageItem::CedianRow(row) if row.title == title => Some(row),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no {title} row on {}", page.title))
    }

    #[gpui::test]
    fn sidebar_is_grouped_with_an_icon_per_page_and_hides_zeds_pages(cx: &mut TestAppContext) {
        let (window, cx) = open_window(cx);
        window.read_with(cx, |window, _| {
            let drawn: Vec<String> = window
                .nav_rows()
                .into_iter()
                .filter_map(|row| match row {
                    NavRow::Group(group) => Some(group.label().to_string()),
                    NavRow::Entry(index) if window.navbar_entries[index].is_root => {
                        Some(window.navbar_entries[index].title.to_string())
                    }
                    NavRow::Entry(_) => None,
                })
                .collect();
            assert_eq!(
                drawn,
                [
                    "PERSONAL",
                    "General",
                    "Appearance",
                    "Notifications",
                    "Keyboard Shortcuts",
                    "AGENT",
                    "Account",
                    "Models",
                    "Methodology",
                    "Permissions",
                    "WORKSPACE",
                    "Projects",
                    "Editor",
                    "Languages & Tools",
                    "Search & Files",
                    "Window & Layout",
                    "Panels",
                    "Terminal",
                    "Debugger",
                    "Version Control",
                    "Network",
                    "SYSTEM",
                    "Privacy",
                    "Advanced",
                    "About",
                ]
            );
            for page in &window.pages {
                assert!(
                    sidebar_page(page.title).is_some(),
                    "{} has no icon",
                    page.title
                );
                assert!(
                    !HIDDEN_PAGES.contains(&page.title),
                    "{} is shown",
                    page.title
                );
            }
        });
    }

    #[gpui::test]
    fn no_page_offers_a_telemetry_switch(cx: &mut TestAppContext) {
        let (window, cx) = open_window(cx);
        window.read_with(cx, |window, _| {
            for page in &window.pages {
                for item in page.items.iter() {
                    if let SettingsPageItem::SettingItem(item) = item {
                        let path = item.field.json_path().unwrap_or("");
                        assert!(
                            !path.starts_with("telemetry."),
                            "{} > {} still offers {path}",
                            page.title,
                            item.title
                        );
                    }
                }
            }
            let privacy = page(window, "Privacy");
            assert_eq!(
                cedian_row(privacy, "Crash Reports and Usage Metrics").description,
                TELEMETRY_OFF
            );
            for pin in cedian_omp::PRIVACY_PINS {
                assert_eq!(
                    cedian_row(privacy, pin.title).badge,
                    Some(ScopeBadge::SetByCedian),
                    "{}",
                    pin.key
                );
            }
            setting(privacy, "Redact Private Values");
            setting(privacy, "Private Files");
        });
    }

    #[gpui::test]
    fn general_rows_write_what_they_show(cx: &mut TestAppContext) {
        let (window, cx) = open_window(cx);
        window.read_with(cx, |window, cx| {
            let general = page(window, "General");
            let defaults = SettingsStore::global(cx).raw_default_settings();
            let mut content = SettingsContent::default();

            let restricted = field::<bool>(setting(general, "Open Projects in Restricted Mode"));
            assert_eq!((restricted.pick)(defaults), Some(&true));
            (restricted.write)(&mut content, Some(false), cx);
            assert_eq!(
                content.session.as_ref().unwrap().trust_all_worktrees,
                Some(true)
            );
            assert_eq!((restricted.pick)(&content), Some(&false));

            let follow_up =
                field::<FollowUp>(setting(general, "Messages Sent While the Agent Works"));
            assert_eq!((follow_up.pick)(defaults), Some(&FollowUp::Queue));
            (follow_up.write)(&mut content, Some(FollowUp::Steer), cx);
            assert_eq!(
                content.cedian.as_ref().unwrap().follow_up,
                Some(FollowUp::Steer)
            );

            let quit = field::<bool>(setting(general, "Confirm Before Quitting"));
            (quit.write)(&mut content, Some(true), cx);
            assert_eq!(content.workspace.confirm_quit, Some(true));
            setting(general, "Accessible Mode");
            setting(general, "Restore On Startup");
            let headers: Vec<_> = general
                .items
                .iter()
                .filter_map(|item| match item {
                    SettingsPageItem::SectionHeader(header) => Some(*header),
                    _ => None,
                })
                .collect();
            assert_eq!(headers.first(), Some(&"App"));
            assert!(
                !headers.contains(&"General Settings"),
                "Zed's general rows join App; the page names no second General: {headers:?}"
            );
        });
    }

    #[test]
    fn a_long_path_keeps_its_root_and_last_folder() {
        assert_eq!(truncate_middle("~/.config/cedian", 32), "~/.config/cedian");
        let long = "~/Library/Application Support/cedian/agent/state";
        let short = truncate_middle(long, 32);
        assert_eq!(short.chars().count(), 32);
        assert_eq!(short, "~/Library/Appli…dian/agent/state");
    }

    #[gpui::test]
    fn notification_rows_write_what_they_show(cx: &mut TestAppContext) {
        let (window, cx) = open_window(cx);
        window.read_with(cx, |window, cx| {
            let notifications = page(window, "Notifications");
            let defaults = SettingsStore::global(cx).raw_default_settings();
            let mut content = SettingsContent::default();

            let turn = field::<TurnFinishedNotification>(setting(notifications, "Turn Finished"));
            assert_eq!(
                (turn.pick)(defaults),
                Some(&TurnFinishedNotification::WhenUnfocused)
            );
            (turn.write)(&mut content, Some(TurnFinishedNotification::Never), cx);
            for (title, read) in [
                (
                    "Approval Needed",
                    (|c: &SettingsContent| c.cedian.as_ref()?.notify_approval_needed)
                        as fn(&SettingsContent) -> Option<bool>,
                ),
                ("Question Asked", |c| {
                    c.cedian.as_ref()?.notify_question_asked
                }),
                ("Bounce the Dock Icon", |c| {
                    c.cedian.as_ref()?.bounce_dock_icon
                }),
            ] {
                let switch = field::<bool>(setting(notifications, title));
                assert_eq!((switch.pick)(defaults), Some(&true), "{title}");
                (switch.write)(&mut content, Some(false), cx);
                assert_eq!(read(&content), Some(false), "{title}");
            }
            assert_eq!(
                content.cedian.unwrap().notify_turn_finished,
                Some(TurnFinishedNotification::Never)
            );
            let desktop = cedian_row(notifications, "Desktop Notifications");
            assert!(matches!(
                desktop.state.and_then(|state| state(cx)),
                Some(RowState::Unavailable(_))
            ));
        });
    }

    #[gpui::test]
    async fn restore_defaults_resets_each_changed_row_of_the_page(cx: &mut TestAppContext) {
        let (window, cx) = open_window(cx);
        let changed = r#"{"cedian": {"follow_up": "steer"}, "session": {"trust_all_worktrees": true}, "confirm_quit": true}"#;
        let fs = cx.update(|_, cx| <dyn fs::Fs>::global(cx));
        fs.write(paths::settings_file(), changed.as_bytes())
            .await
            .unwrap();
        cx.update(|_, cx| {
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.set_user_settings(changed, cx).result().unwrap();
            });
        });
        window.update_in(cx, |window, gpui_window, cx| {
            let general = window
                .navbar_entries
                .iter()
                .position(|entry| entry.is_root && entry.title == "General")
                .unwrap();
            window.open_navbar_entry_page(general);
            assert_eq!(window.current_page().title, "General");
            assert_eq!(window.current_page_resets(cx).len(), 3);
            assert!(sidebar_page("General").unwrap().restore_defaults);
            assert!(!sidebar_page("Editor").unwrap().restore_defaults);
            window.reset_current_page(gpui_window, cx);
        });
        cx.run_until_parked();
        let written = fs.load(paths::settings_file()).await.unwrap();
        let written: serde_json::Value = settings::parse_json_with_comments(&written)
            .unwrap_or_else(|error| panic!("{error}: {written:?}"));
        for (value, default) in [
            (&written["cedian"]["follow_up"], serde_json::json!("queue")),
            (
                &written["session"]["trust_all_worktrees"],
                serde_json::json!(false),
            ),
            (&written["confirm_quit"], serde_json::json!(false)),
        ] {
            assert!(value.is_null() || *value == default, "{written}");
        }
    }

    #[gpui::test]
    fn about_names_versions_and_credits(cx: &mut TestAppContext) {
        let (window, cx) = open_window(cx);
        window.read_with(cx, |window, cx| {
            let lines = about_lines(cx);
            let names: Vec<_> = lines.iter().map(|(name, _)| *name).collect();
            assert_eq!(names, ["cedian", "Zed", "Agent", "pstack"]);
            let pin = agent_pin().unwrap();
            assert!(lines[2].1.contains(&pin.version) && lines[2].1.contains(&pin.commit[..10]));
            assert_eq!(lines[3].1, "0.15.24 · 1e5c90a4b6 · Lauren Tan, MIT");
            let licenses = cedian_row(page(window, "About"), "Licenses").description;
            for credit in ["GPL", "MIT", "Lauren Tan"] {
                assert!(licenses.contains(credit), "{credit}");
            }
            let advanced = page(window, "Advanced");
            assert!(advanced.items.iter().any(|item| matches!(
                item,
                SettingsPageItem::SubPageLink(link) if link.title.as_ref() == "All Agent Settings"
            )));
            cedian_row(advanced, "Restart Agent");
            cedian_row(advanced, "Open cedian.toml");
            assert!(advanced.items.iter().any(|item| matches!(
                item,
                SettingsPageItem::SectionHeader("Feature Flags" | "Instrumentation")
            )));
        });
    }

    #[gpui::test]
    fn agent_pages_show_the_hosts_view_or_say_the_agent_is_missing(cx: &mut TestAppContext) {
        struct Hosted(AgentPage);
        impl Render for Hosted {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
            }
        }
        let (window, cx) = open_window(cx);
        window.read_with(cx, |window, _| {
            for (title, which) in [
                ("Account", AgentPage::Account),
                ("Models", AgentPage::Models),
                ("Methodology", AgentPage::Methodology),
                ("Permissions", AgentPage::Permissions),
                ("Projects", AgentPage::Projects),
            ] {
                let view = page(window, title)
                    .items
                    .iter()
                    .find_map(|item| match item {
                        SettingsPageItem::CedianView(view) => Some(view),
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("{title} has no agent view"));
                assert_eq!(view.page, which);
            }
        });
        cx.update(|window, cx| {
            assert!(agent_page_view(AgentPage::Models, None, window, cx).is_none());
            set_host(
                CedianHost {
                    agent_settings: |_, _, _| None,
                    restart_agent: |_, _| {},
                    agent_page: |page, _, _, cx| Some(cx.new(|_| Hosted(page)).into()),
                },
                cx,
            );
            let view = agent_page_view(AgentPage::Models, None, window, cx).unwrap();
            assert_eq!(
                view.downcast::<Hosted>().unwrap().read(cx).0,
                AgentPage::Models
            );
        });
    }

    #[test]
    fn every_sidebar_icon_and_page_is_listed_once() {
        let mut titles: Vec<_> = SIDEBAR.iter().map(|page| page.title).collect();
        titles.sort();
        titles.dedup();
        assert_eq!(titles.len(), SIDEBAR.len());
    }

    #[test]
    fn agent_pin_reads_the_vendored_revision() {
        let pin = agent_pin().expect("vendor/omp-revision.json parses");
        assert_eq!(pin.commit.len(), 40);
        assert!(pin.version.split('.').count() == 3, "{pin:?}");
    }
}
