mod agent_pane;
mod assets;
mod composer;
mod daemon_client;
mod density;
mod desktop;
mod dock;
mod export;
mod file_tree;
mod git_pane;
mod layout;
mod line_breaks;
mod open_in;
mod project;
mod receipt;
mod replay;
mod review_pane;
mod root_view;
mod settings_view;
mod settings_window;
mod shell_pane;
mod sound;
mod status_bar;
mod tab_strip;
mod thread_details;
mod thread_list;
mod tokens;
mod tool_card;
mod tool_rail;
mod toolbar;
mod view_mode;

use std::path::PathBuf;

use anyhow::Context as _;
use gpui::{
    App, AppContext as _, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, point, px, size,
};
use settings::KeymapFile;

use crate::{agent_pane::AgentPane, root_view::RootView};

enum Mode {
    /// The threads of the projects in the tabs, through the daemon; `cwd` is the `--cwd`
    /// asked for.
    Live {
        cwd: Option<PathBuf>,
    },
    Replay(replay::Replay),
}

fn main() {
    let mode = match mode() {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("cedian: {error:#}");
            std::process::exit(2);
        }
    };
    gpui_platform::application()
        .with_assets(crate::assets::Assets)
        .run(move |cx: &mut App| {
            if let Err(error) = open(mode, cx) {
                log::error!("{error:#}");
                eprintln!("cedian: {error:#}");
                cx.quit();
            }
        });
}

const USAGE: &str = "usage: cedian [--cwd <dir>] | cedian --replay <stream.jsonl> [--instant]";

/// `--cwd <dir>` opens the project in front of the saved tabs (with neither, the current
/// directory). `--replay
/// <stream.jsonl>` plays a recorded run instead, read before the window opens so a bad file
/// fails fast; `--instant` applies every line before the first frame, for budgets on long
/// threads.
fn mode() -> anyhow::Result<Mode> {
    let mut args = std::env::args_os().skip(1);
    let Some(flag) = args.next() else {
        return Ok(Mode::Live { cwd: None });
    };
    if flag == "--cwd" {
        let cwd = PathBuf::from(args.next().context("--cwd needs a directory")?);
        let cwd = cwd
            .canonicalize()
            .with_context(|| format!("opening {}", cwd.display()))?;
        anyhow::ensure!(args.next().is_none(), "{USAGE}");
        return Ok(Mode::Live { cwd: Some(cwd) });
    }
    anyhow::ensure!(flag == "--replay", "unknown argument {flag:?}; {USAGE}");
    let path = PathBuf::from(args.next().context("--replay needs a stream.jsonl path")?);
    let instant = match args.next() {
        None => false,
        Some(flag) if flag == "--instant" => true,
        Some(other) => anyhow::bail!("unknown argument {other:?}; {USAGE}"),
    };
    let lines = replay::read(&path)?;
    Ok(Mode::Replay(replay::Replay {
        path,
        lines,
        instant,
    }))
}

#[cfg(target_os = "macos")]
const STOP_KEY: &str = "cmd-.";
#[cfg(not(target_os = "macos"))]
const STOP_KEY: &str = "ctrl-.";
#[cfg(target_os = "macos")]
const OMNIBOX_KEY: &str = "cmd-k";
#[cfg(not(target_os = "macos"))]
const OMNIBOX_KEY: &str = "ctrl-k";
/// Puts the selection in the editor or the shell into the composer, as Cursor's `⌘L` does;
/// `ctrl-l` clears a Linux shell, so Linux takes the shifted key.
#[cfg(target_os = "macos")]
const ATTACH_KEY: &str = "cmd-l";
#[cfg(not(target_os = "macos"))]
const ATTACH_KEY: &str = "ctrl-shift-l";
/// Folds the sidebar to its rail; `ctrl-b` belongs to the shell on Linux.
#[cfg(target_os = "macos")]
const SIDEBAR_KEY: &str = "cmd-b";
#[cfg(not(target_os = "macos"))]
const SIDEBAR_KEY: &str = "ctrl-shift-b";
/// The project tabs: `⌘T` opens one, `⌘W` closes it, `⌘1`–`⌘9` pick one, as in Chrome. On
/// Linux the shell keeps `ctrl-t` and `ctrl-w`, so those take the shifted key.
#[cfg(target_os = "macos")]
const TAB_KEYS: (&str, &str, &str) = ("cmd-t", "cmd-w", "cmd");
#[cfg(not(target_os = "macos"))]
const TAB_KEYS: (&str, &str, &str) = ("ctrl-shift-t", "ctrl-shift-w", "ctrl");
#[cfg(target_os = "macos")]
const TOOL_KEY: &str = "cmd-alt";
#[cfg(not(target_os = "macos"))]
const TOOL_KEY: &str = "ctrl-alt";

fn open(mode: Mode, cx: &mut App) -> anyhow::Result<()> {
    crate::assets::Assets
        .load_fonts(cx)
        .context("loading fonts")?;
    settings::init(cx);
    // Loads cedian Dark and cedian Light with Zed's bundled themes; `desktop.theme` picks.
    theme_settings::init(theme::LoadThemes::All(Box::new(crate::assets::Assets)), cx);
    release_channel::init(semver::Version::new(0, 1, 0), cx);
    editor::init(cx);
    let bindings = KeymapFile::load_asset_allow_partial_failure(settings::DEFAULT_KEYMAP_PATH, cx)
        .context("loading the default keymap")?;
    cx.bind_keys(bindings);
    cx.bind_keys([
        KeyBinding::new("enter", composer::Submit, Some("Composer > Editor")),
        KeyBinding::new("j", review_pane::Next, Some("ReviewList")),
        KeyBinding::new("k", review_pane::Previous, Some("ReviewList")),
        KeyBinding::new("y", review_pane::Accept, Some("ReviewList")),
        KeyBinding::new("n", review_pane::Reject, Some("ReviewList")),
        KeyBinding::new(
            "enter",
            review_pane::ConfirmReject,
            Some("RejectReason > Editor"),
        ),
        KeyBinding::new("escape", review_pane::Cancel, Some("RejectReason > Editor")),
        // Stop works from anywhere in the window, the composer too, where Zed's keymap gives
        // the key to code actions.
        KeyBinding::new(STOP_KEY, agent_pane::StopRun, None),
        KeyBinding::new(STOP_KEY, agent_pane::StopRun, Some("Composer > Editor")),
        KeyBinding::new(OMNIBOX_KEY, toolbar::FocusOmnibox, None),
        KeyBinding::new(
            OMNIBOX_KEY,
            toolbar::FocusOmnibox,
            Some("Composer > Editor"),
        ),
        KeyBinding::new("up", toolbar::SelectPrevious, Some("Omnibox > Editor")),
        KeyBinding::new("down", toolbar::SelectNext, Some("Omnibox > Editor")),
        KeyBinding::new("enter", toolbar::Confirm, Some("Omnibox > Editor")),
        KeyBinding::new("escape", toolbar::Dismiss, Some("Omnibox > Editor")),
        KeyBinding::new(SIDEBAR_KEY, root_view::ToggleSidebar, None),
        KeyBinding::new(
            SIDEBAR_KEY,
            root_view::ToggleSidebar,
            Some("Composer > Editor"),
        ),
        KeyBinding::new(ATTACH_KEY, shell_pane::AttachSelection, Some("Shell")),
        KeyBinding::new(
            ATTACH_KEY,
            shell_pane::AttachSelection,
            Some("ToolEditor > Editor"),
        ),
        KeyBinding::new(&format!("{TOOL_KEY}-1"), tool_rail::ToggleEditor, None),
        KeyBinding::new(&format!("{TOOL_KEY}-2"), tool_rail::ToggleTerminal, None),
        KeyBinding::new(&format!("{TOOL_KEY}-3"), tool_rail::ToggleReview, None),
        KeyBinding::new(&format!("{TOOL_KEY}-4"), tool_rail::ToggleFiles, None),
        KeyBinding::new(&format!("{TOOL_KEY}-6"), tool_rail::ToggleGit, None),
        KeyBinding::new(&format!("{TOOL_KEY}-7"), tool_rail::ToggleDetails, None),
        KeyBinding::new(&format!("{TOOL_KEY}-0"), tool_rail::HideTools, None),
    ]);
    // Bound in the composer too, where Zed's keymap would otherwise take some of these keys.
    let (new_tab, close_tab, pick) = TAB_KEYS;
    for context in [None, Some("Composer > Editor")] {
        cx.bind_keys([
            KeyBinding::new(new_tab, tab_strip::NewTab, context),
            KeyBinding::new(close_tab, tab_strip::CloseTab, context),
            KeyBinding::new("ctrl-tab", tab_strip::NextTab, context),
            KeyBinding::new("ctrl-shift-tab", tab_strip::PreviousTab, context),
        ]);
        cx.bind_keys(
            (1..=9).map(|n| {
                KeyBinding::new(&format!("{pick}-{n}"), tab_strip::ActivateTab(n), context)
            }),
        );
    }
    // Zed's keymap binds cmd-q (ctrl-q on Linux) to zed::Quit; the app menu gives macOS its
    // Quit item. Quitting the window never stops a thread: the daemon runs on.
    cx.on_action(|_: &zed_actions::Quit, cx| cx.quit());
    cx.set_menus([Menu {
        name: "cedian".into(),
        items: vec![
            MenuItem::action("Settings…", zed_actions::OpenSettings),
            MenuItem::separator(),
            MenuItem::action("Quit cedian", zed_actions::Quit),
        ],
        disabled: false,
    }]);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();

    let desktop = desktop::read_at_launch();
    let settings = desktop::Desktop::from_value(&desktop);
    settings.apply(cx);
    let view_mode = settings.view_mode;
    let layout = layout::Layout::from_desktop(&desktop);
    let saved = match &mode {
        Mode::Live { .. } => layout.bounds_on_screen(cx),
        Mode::Replay(_) => None,
    };
    let bounds = saved.unwrap_or_else(|| {
        WindowBounds::Windowed(Bounds::centered(None, size(px(1280.), px(800.)), cx))
    });
    let options = WindowOptions {
        window_bounds: Some(bounds),
        window_min_size: Some(size(px(1024.), px(640.))),
        // The project tabs are drawn in the title bar, under the traffic lights (macOS; Linux
        // keeps its window manager's title bar).
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(14.))),
        }),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| match mode {
        Mode::Live { cwd } => {
            let here = std::env::current_dir().unwrap_or_default();
            let (tabs, front) = layout.tabs(cwd, here);
            cx.new(|cx| RootView::live(tabs, front, layout, view_mode, window, cx))
        }
        Mode::Replay(replay) => {
            let title = format!("replay {}", replay.path.display());
            let agent = cx.new(|_| AgentPane::new(title).with_view_mode(view_mode));
            replay::play(replay, agent.clone(), cx);
            cx.new(|cx| RootView::replay(agent, window, cx))
        }
    })
    .context("opening the window")?;
    cx.activate(true);
    Ok(())
}
