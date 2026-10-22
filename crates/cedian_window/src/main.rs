mod agent_pane;
mod assets;
mod composer;
mod daemon_client;
mod desktop;
mod layout;
mod line_breaks;
mod replay;
mod review_pane;
mod root_view;
mod settings_view;
mod shell_pane;
mod spike_view;
mod tokens;

use std::path::PathBuf;

use anyhow::Context as _;
use gpui::{
    App, AppContext as _, Bounds, KeyBinding, Menu, MenuItem, WindowBounds, WindowOptions, px, size,
};
use settings::KeymapFile;

use crate::{agent_pane::AgentPane, root_view::RootView};

enum Mode {
    /// The threads of a project, through the daemon.
    Live {
        cwd: PathBuf,
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

/// `--cwd <dir>` picks the project (default: the current directory). `--replay
/// <stream.jsonl>` plays a recorded run instead, read before the window opens so a bad file
/// fails fast; `--instant` applies every line before the first frame, for budgets on long
/// threads.
fn mode() -> anyhow::Result<Mode> {
    let mut args = std::env::args_os().skip(1);
    let Some(flag) = args.next() else {
        return Ok(Mode::Live {
            cwd: std::env::current_dir().context("reading the current directory")?,
        });
    };
    if flag == "--cwd" {
        let cwd = PathBuf::from(args.next().context("--cwd needs a directory")?);
        let cwd = cwd
            .canonicalize()
            .with_context(|| format!("opening {}", cwd.display()))?;
        anyhow::ensure!(args.next().is_none(), "{USAGE}");
        return Ok(Mode::Live { cwd });
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
    ]);
    // Zed's keymap binds cmd-q (ctrl-q on Linux) to zed::Quit; the app menu gives macOS its
    // Quit item. Quitting the window never stops a thread: the daemon runs on.
    cx.on_action(|_: &zed_actions::Quit, cx| cx.quit());
    cx.set_menus([Menu {
        name: "cedian".into(),
        items: vec![MenuItem::action("Quit cedian", zed_actions::Quit)],
        disabled: false,
    }]);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();

    let desktop = desktop::read_at_launch();
    desktop::Desktop::from_value(&desktop).apply(cx);
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
        ..Default::default()
    };
    cx.open_window(options, |window, cx| match mode {
        Mode::Live { cwd } => cx.new(|cx| RootView::live(cwd, layout, window, cx)),
        Mode::Replay(replay) => {
            let title = format!("replay {}", replay.path.display());
            let agent = cx.new(|_| AgentPane::new(title));
            replay::play(replay, agent.clone(), cx);
            cx.new(|cx| RootView::replay(agent, window, cx))
        }
    })
    .context("opening the window")?;
    cx.activate(true);
    Ok(())
}
