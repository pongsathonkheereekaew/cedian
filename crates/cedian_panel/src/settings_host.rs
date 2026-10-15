//! What the settings window asks of the agent panel (Settings > Advanced):
//! the agent's settings view and Restart agent. The app binds these into
//! `settings_ui` at startup.

use crate::omp_settings::OmpSettings;
use crate::panel::CedianPanel;
use collections::HashMap;
use gpui::{
    AnyView, AnyWindowHandle, App, AppContext as _, Entity, Global, Subscription, Window, WindowId,
};
use util::ResultExt as _;
use workspace::{MultiWorkspace, Workspace};

/// One agent-settings view per settings window, with the workspace window it
/// was built for; a view goes when its settings window closes.
struct Views {
    views: HashMap<WindowId, (AnyWindowHandle, Entity<OmpSettings>)>,
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
/// frontmost workspace window's when there is no `origin` or it has closed.
pub(crate) fn target(
    origin: Option<AnyWindowHandle>,
    cx: &App,
) -> Option<(AnyWindowHandle, Entity<Workspace>, Entity<CedianPanel>)> {
    let open = cx.windows();
    if let Some(origin) = origin.filter(|origin| open.contains(origin)) {
        return panel_of(origin, cx);
    }
    cx.window_stack()
        .unwrap_or(open)
        .into_iter()
        .find_map(|handle| panel_of(handle, cx))
}

pub fn agent_settings(
    origin: Option<AnyWindowHandle>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyView> {
    let id = window.window_handle().window_id();
    let (handle, workspace, _) = target(origin, cx)?;
    if let Some((built_for, view)) = cx
        .try_global::<Views>()
        .and_then(|views| views.views.get(&id))
        && *built_for == handle
    {
        return Some(view.clone().into());
    }
    let project = workspace.read(cx).project().clone();
    let root = project
        .read(cx)
        .visible_worktrees(cx)
        .next()?
        .read(cx)
        .abs_path()
        .to_path_buf();
    let view = cx.new(|cx| OmpSettings::new(project, root, window, cx));
    views(cx).views.insert(id, (handle, view.clone()));
    Some(view.into())
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
        .map(|views| views.views.values().map(|(_, view)| view.clone()).collect())
        .unwrap_or_default();
    for view in views {
        view.update(cx, |view, cx| view.reload(cx));
    }
}
