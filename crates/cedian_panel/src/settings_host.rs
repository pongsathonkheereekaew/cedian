//! What the settings window asks of the agent panel (Settings > Advanced):
//! the agent's settings view and Restart agent. The app binds these into
//! `settings_ui` at startup.

use crate::omp_settings::OmpSettings;
use crate::panel::CedianPanel;
use collections::HashMap;
use gpui::{AnyView, AnyWindowHandle, App, AppContext as _, Entity, Global, Window, WindowId};
use workspace::{MultiWorkspace, Workspace};

/// One agent-settings view per settings window, built on first use.
#[derive(Default)]
struct Views(HashMap<WindowId, Entity<OmpSettings>>);

impl Global for Views {}

/// The workspace windows, frontmost first, with their cedian panel.
fn panels(cx: &App) -> Vec<(AnyWindowHandle, Entity<Workspace>, Entity<CedianPanel>)> {
    let windows = cx.window_stack().unwrap_or_else(|| cx.windows());
    windows
        .into_iter()
        .filter_map(|handle| {
            let workspace = handle
                .downcast::<MultiWorkspace>()?
                .read(cx)
                .ok()?
                .workspace()
                .clone();
            let panel = workspace.read(cx).panel::<CedianPanel>(cx)?;
            Some((handle, workspace, panel))
        })
        .collect()
}

pub fn agent_settings(window: &mut Window, cx: &mut App) -> Option<AnyView> {
    let id = window.window_handle().window_id();
    if let Some(view) = cx.try_global::<Views>().and_then(|views| views.0.get(&id)) {
        return Some(view.clone().into());
    }
    let (_, workspace, _) = panels(cx).into_iter().next()?;
    let project = workspace.read(cx).project().clone();
    let root = project
        .read(cx)
        .visible_worktrees(cx)
        .next()?
        .read(cx)
        .abs_path()
        .to_path_buf();
    let view = cx.new(|cx| OmpSettings::new(project, root, window, cx));
    cx.default_global::<Views>().0.insert(id, view.clone());
    Some(view.into())
}

pub fn restart_agent(cx: &mut App) {
    let Some((handle, _, panel)) = panels(cx).into_iter().next() else {
        return;
    };
    handle
        .update(cx, |_, window, cx| {
            panel.update(cx, |panel, cx| panel.restart(window, cx));
        })
        .ok();
}

/// Re-read every open agent-settings view after the agent's config changed.
pub fn reload(cx: &mut App) {
    let views: Vec<_> = cx
        .try_global::<Views>()
        .map(|views| views.0.values().cloned().collect())
        .unwrap_or_default();
    for view in views {
        view.update(cx, |view, cx| view.reload(cx));
    }
}
