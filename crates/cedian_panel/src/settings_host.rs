use crate::account_page::AccountPage;
use crate::methodology_page::MethodologyPage;
use crate::models_page::ModelsPage;
use crate::omp_settings::OmpSettings;
use crate::panel::CedianPanel;
use collections::HashMap;
use gpui::{
    AnyView, AnyWindowHandle, App, AppContext as _, Entity, Global, Subscription, Window, WindowId,
};
use settings_ui::AgentPage;
use util::ResultExt as _;
use workspace::{MultiWorkspace, Workspace};

/// The agent views of one settings window, with the workspace window they
/// were built for; they go when their settings window closes.
struct Built {
    workspace_window: AnyWindowHandle,
    settings: Entity<OmpSettings>,
    account: Option<Entity<AccountPage>>,
    models: Option<Entity<ModelsPage>>,
    methodology: Option<Entity<MethodologyPage>>,
}

struct Views {
    views: HashMap<WindowId, Built>,
    _closed: Subscription,
}

impl Global for Views {}

fn views(cx: &mut App) -> &mut Views {
    if !cx.has_global::<Views>() {
        let closed = cx.on_window_closed(|cx, id| {
            cx.global_mut::<Views>().views.remove(&id);
        });
        cx.set_global(Views {
            views: HashMap::default(),
            _closed: closed,
        });
    }
    cx.global_mut::<Views>()
}

fn panel_of(
    handle: AnyWindowHandle,
    cx: &App,
) -> Option<(AnyWindowHandle, Entity<Workspace>, Entity<CedianPanel>)> {
    let workspace = handle
        .downcast::<MultiWorkspace>()?
        .read(cx)
        .ok()?
        .workspace()
        .clone();
    let panel = workspace.read(cx).panel::<CedianPanel>(cx)?;
    Some((handle, workspace, panel))
}

/// The panel of `origin`, the workspace window that opened Settings; the
/// frontmost workspace window's when there is no `origin`, it has closed,
/// or its panel has not loaded yet.
pub(crate) fn target(
    origin: Option<AnyWindowHandle>,
    cx: &App,
) -> Option<(AnyWindowHandle, Entity<Workspace>, Entity<CedianPanel>)> {
    let open = cx.windows();
    origin
        .filter(|origin| open.contains(origin))
        .and_then(|origin| panel_of(origin, cx))
        .or_else(|| {
            cx.window_stack()
                .unwrap_or(open)
                .into_iter()
                .find_map(|handle| panel_of(handle, cx))
        })
}

/// This settings window's agent views for `origin`, built on first use.
fn built<'a>(
    origin: Option<AnyWindowHandle>,
    window: &mut Window,
    cx: &'a mut App,
) -> Option<(&'a mut Built, Entity<CedianPanel>)> {
    let id = window.window_handle().window_id();
    let (handle, workspace, panel) = target(origin, cx)?;
    let current = cx
        .try_global::<Views>()
        .and_then(|views| views.views.get(&id))
        .is_some_and(|built| built.workspace_window == handle);
    if !current {
        let project = workspace.read(cx).project().clone();
        let root = project
            .read(cx)
            .visible_worktrees(cx)
            .next()?
            .read(cx)
            .abs_path()
            .to_path_buf();
        let settings = cx.new(|cx| OmpSettings::new(project, root, window, cx));
        views(cx).views.insert(
            id,
            Built {
                workspace_window: handle,
                settings,
                account: None,
                models: None,
                methodology: None,
            },
        );
    }
    Some((views(cx).views.get_mut(&id)?, panel))
}

pub fn agent_settings(
    origin: Option<AnyWindowHandle>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyView> {
    let (built, _) = built(origin, window, cx)?;
    Some(built.settings.clone().into())
}

/// Settings > Account or Models for the workspace window that opened
/// Settings.
pub fn agent_page(
    page: AgentPage,
    origin: Option<AnyWindowHandle>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyView> {
    let (built, panel) = built(origin, window, cx)?;
    let workspace_window = built.workspace_window;
    let settings = built.settings.clone();
    let existing: Option<AnyView> = match page {
        AgentPage::Account => built.account.clone().map(Into::into),
        AgentPage::Models => built.models.clone().map(Into::into),
        AgentPage::Methodology => built.methodology.clone().map(Into::into),
    };
    if let Some(view) = existing {
        return Some(view);
    }
    let id = window.window_handle().window_id();
    let view: AnyView = match page {
        AgentPage::Account => {
            let agent_dir = cedian_omp::AgentDirs::from_env()
                .ok()
                .map(|dirs| dirs.config);
            let view =
                cx.new(|cx| AccountPage::new(panel, workspace_window, agent_dir, window, cx));
            views(cx).views.get_mut(&id)?.account = Some(view.clone());
            view.into()
        }
        AgentPage::Models => {
            let view = cx.new(|cx| ModelsPage::new(panel, settings, window, cx));
            views(cx).views.get_mut(&id)?.models = Some(view.clone());
            view.into()
        }
        AgentPage::Methodology => {
            let view = cx.new(|cx| MethodologyPage::new(panel, settings, cx));
            views(cx).views.get_mut(&id)?.methodology = Some(view.clone());
            view.into()
        }
    };
    Some(view)
}

pub fn restart_agent(origin: Option<AnyWindowHandle>, cx: &mut App) {
    let Some((handle, _, panel)) = target(origin, cx) else {
        return;
    };
    handle
        .update(cx, |_, window, cx| {
            panel.update(cx, |panel, cx| panel.restart(window, cx));
        })
        .log_err();
}

/// Re-read every open agent-settings view after the agent's config changed.
pub fn reload(cx: &mut App) {
    let views: Vec<_> = cx
        .try_global::<Views>()
        .map(|views| {
            views
                .views
                .values()
                .map(|built| built.settings.clone())
                .collect()
        })
        .unwrap_or_default();
    for view in views {
        view.update(cx, |view, cx| view.reload(cx));
    }
}
