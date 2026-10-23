mod agent_pane;
mod assets;
mod block_drag;
mod block_header;
mod block_keys;
mod browser_pane;
mod cdp;
mod composer;
mod content_search;
mod daemon_client;
mod density;
mod desktop;
mod dock;
mod editor_file;
mod export;
mod file_finder;
mod file_tree;
mod git_pane;
mod layout;
mod line_breaks;
mod memory_pane;
mod motion;
mod on_click;
mod open_in;
mod outputs_pane;
mod overview;
mod popup_menu;
mod project;
mod receipt;
mod replay;
mod review_pane;
mod root_view;
mod settings_view;
mod settings_window;
mod shell_pane;
mod sidebar_foot;
mod skills_pane;
mod sound;
mod tab_strip;
mod thread_details;
mod thread_list;
mod thread_search;
mod tokens;
mod tool_card;
mod tool_menu;
mod tooltip;
mod type_scale;
mod usage_pane;
mod view_mode;
mod workplace;

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

/// The project tabs are drawn in the title bar, under the traffic lights (macOS; Linux keeps
/// its window manager's title bar).
pub fn titlebar() -> TitlebarOptions {
    TitlebarOptions {
        title: None,
        appears_transparent: true,
        traffic_light_position: Some(point(px(14.), px(14.))),
    }
}

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
/// Finds a file by name, as in Zed and VS Code.
#[cfg(target_os = "macos")]
const FIND_FILE_KEY: &str = "cmd-p";
#[cfg(not(target_os = "macos"))]
const FIND_FILE_KEY: &str = "ctrl-p";
/// Searches the files' contents, as in Zed and VS Code.
#[cfg(target_os = "macos")]
const SEARCH_CONTENTS_KEY: &str = "cmd-shift-f";
#[cfg(not(target_os = "macos"))]
const SEARCH_CONTENTS_KEY: &str = "ctrl-shift-f";
#[cfg(target_os = "macos")]
const SEARCH_KEY: &str = "cmd-k";
#[cfg(not(target_os = "macos"))]
const SEARCH_KEY: &str = "ctrl-k";
/// Puts the selection in the editor or the shell into the composer, as Cursor's `⌘L` does;
/// `ctrl-l` clears a Linux shell, so Linux takes the shifted key.
#[cfg(target_os = "macos")]
const ATTACH_KEY: &str = "cmd-l";
#[cfg(not(target_os = "macos"))]
const ATTACH_KEY: &str = "ctrl-shift-l";
/// Writes the Editor's file, as in every editor.
#[cfg(target_os = "macos")]
const SAVE_KEY: &str = "cmd-s";
#[cfg(not(target_os = "macos"))]
const SAVE_KEY: &str = "ctrl-s";
/// The project tabs: `⌘T` opens one, `⌘⇧W` closes it (`⌘W` closes a block), `⌘1`–`⌘9` pick
/// one, as in Chrome. On Linux the shell keeps `ctrl-t` and `ctrl-w`, so those take the
/// shifted key.
#[cfg(target_os = "macos")]
const TAB_KEYS: (&str, &str, &str) = ("cmd-t", "cmd-shift-w", "cmd");
#[cfg(not(target_os = "macos"))]
const TAB_KEYS: (&str, &str, &str) = ("ctrl-shift-t", "ctrl-shift-w", "ctrl");
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
        KeyBinding::new("up", composer::CommandUp, Some("CommandList > Editor")),
        KeyBinding::new("down", composer::CommandDown, Some("CommandList > Editor")),
        KeyBinding::new("tab", composer::CommandPick, Some("CommandList > Editor")),
        KeyBinding::new("enter", composer::CommandPick, Some("CommandList > Editor")),
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
        KeyBinding::new(SEARCH_KEY, thread_search::FocusSearch, None),
        KeyBinding::new(FIND_FILE_KEY, file_finder::FindFile, None),
        KeyBinding::new(
            FIND_FILE_KEY,
            file_finder::FindFile,
            Some("Composer > Editor"),
        ),
        KeyBinding::new("up", file_finder::Previous, Some("FileFinder > Editor")),
        KeyBinding::new("down", file_finder::Next, Some("FileFinder > Editor")),
        KeyBinding::new("enter", file_finder::Pick, Some("FileFinder > Editor")),
        KeyBinding::new("enter", browser_pane::Go, Some("BrowserUrl > Editor")),
        KeyBinding::new(SEARCH_CONTENTS_KEY, content_search::SearchContents, None),
        KeyBinding::new(
            SEARCH_CONTENTS_KEY,
            content_search::SearchContents,
            Some("Composer > Editor"),
        ),
        KeyBinding::new(
            "up",
            content_search::Previous,
            Some("ContentSearch > Editor"),
        ),
        KeyBinding::new("down", content_search::Next, Some("ContentSearch > Editor")),
        KeyBinding::new(
            "enter",
            content_search::Pick,
            Some("ContentSearch > Editor"),
        ),
        KeyBinding::new(
            SEARCH_KEY,
            thread_search::FocusSearch,
            Some("Composer > Editor"),
        ),
        KeyBinding::new(
            "up",
            thread_search::SelectPrevious,
            Some("ThreadSearch > Editor"),
        ),
        KeyBinding::new(
            "down",
            thread_search::SelectNext,
            Some("ThreadSearch > Editor"),
        ),
        KeyBinding::new(
            "enter",
            thread_search::Confirm,
            Some("ThreadSearch > Editor"),
        ),
        KeyBinding::new(
            "escape",
            thread_search::Dismiss,
            Some("ThreadSearch > Editor"),
        ),
        KeyBinding::new(ATTACH_KEY, shell_pane::AttachSelection, Some("Shell")),
        KeyBinding::new(
            ATTACH_KEY,
            shell_pane::AttachSelection,
            Some("ToolEditor > Editor"),
        ),
        KeyBinding::new(SAVE_KEY, editor_file::Save, Some("ToolEditor > Editor")),
    ]);
    cx.bind_keys(block_keys::bindings());
    // After the block keys' window-wide Escape, which would otherwise take it first.
    cx.bind_keys([
        KeyBinding::new(
            "escape",
            composer::CommandDismiss,
            Some("CommandList > Editor"),
        ),
        KeyBinding::new("escape", file_finder::Close, Some("FileFinder > Editor")),
        KeyBinding::new(
            "escape",
            content_search::Close,
            Some("ContentSearch > Editor"),
        ),
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
        titlebar: Some(titlebar()),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| match mode {
        Mode::Live { cwd } => {
            // Opened from Finder or the Dock, the app starts in `/`; home is the better first tab.
            let here = std::env::current_dir()
                .ok()
                .filter(|dir| dir.parent().is_some())
                .or_else(std::env::home_dir)
                .unwrap_or_default();
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
