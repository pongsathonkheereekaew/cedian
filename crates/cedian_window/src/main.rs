mod agent_pane;
mod replay;
mod root_view;
mod spike_view;

use std::path::PathBuf;

use anyhow::Context as _;
use gpui::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use settings::KeymapFile;

use crate::{agent_pane::AgentPane, root_view::RootView, spike_view::SpikeView};

fn main() {
    let replay = match replay_arg() {
        Ok(replay) => replay,
        Err(error) => {
            eprintln!("cedian: {error:#}");
            std::process::exit(2);
        }
    };
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            if let Err(error) = open(replay, cx) {
                log::error!("{error:#}");
                eprintln!("cedian: {error:#}");
                cx.quit();
            }
        });
}

const USAGE: &str = "usage: cedian [--replay <stream.jsonl> [--instant]]";

/// `--replay <stream.jsonl>`, read before the window opens so a bad file fails fast.
/// `--instant` applies every line before the first frame, for budgets on long threads.
fn replay_arg() -> anyhow::Result<Option<replay::Replay>> {
    let mut args = std::env::args_os().skip(1);
    let Some(flag) = args.next() else {
        return Ok(None);
    };
    anyhow::ensure!(flag == "--replay", "unknown argument {flag:?}; {USAGE}");
    let path = PathBuf::from(args.next().context("--replay needs a stream.jsonl path")?);
    let instant = match args.next() {
        None => false,
        Some(flag) if flag == "--instant" => true,
        Some(other) => anyhow::bail!("unknown argument {other:?}; {USAGE}"),
    };
    let lines = replay::read(&path)?;
    Ok(Some(replay::Replay {
        path,
        lines,
        instant,
    }))
}

fn open(replay: Option<replay::Replay>, cx: &mut App) -> anyhow::Result<()> {
    assets::Assets.load_fonts(cx).context("loading fonts")?;
    settings::init(cx);
    theme_settings::init(theme::LoadThemes::JustBase, cx);
    release_channel::init(semver::Version::new(0, 1, 0), cx);
    editor::init(cx);
    let bindings = KeymapFile::load_asset_allow_partial_failure(settings::DEFAULT_KEYMAP_PATH, cx)
        .context("loading the default keymap")?;
    cx.bind_keys(bindings);

    let bounds = Bounds::centered(None, size(px(1200.), px(760.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        let tools = cx.new(|cx| SpikeView::new(window, cx));
        let title = replay
            .as_ref()
            .map(|replay| format!("replay {}", replay.path.display()))
            .unwrap_or_else(|| "no thread".to_owned());
        let agent = cx.new(|_| AgentPane::new(title));
        if let Some(replay) = replay {
            replay::play(replay, agent.clone(), cx);
        }
        cx.new(|_| RootView::new(agent, tools))
    })
    .context("opening the window")?;
    cx.activate(true);
    Ok(())
}
