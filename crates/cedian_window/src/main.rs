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

/// `--replay <stream.jsonl>`, read before the window opens so a bad file fails fast.
fn replay_arg() -> anyhow::Result<Option<(PathBuf, Vec<cedian_protocol::Line>)>> {
    let mut args = std::env::args_os().skip(1);
    match args.next() {
        None => Ok(None),
        Some(flag) if flag == "--replay" => {
            let path = PathBuf::from(args.next().context("--replay needs a stream.jsonl path")?);
            let lines = replay::read(&path)?;
            Ok(Some((path, lines)))
        }
        Some(other) => {
            anyhow::bail!("unknown argument {other:?}; usage: cedian [--replay <stream.jsonl>]")
        }
    }
}

fn open(replay: Option<(PathBuf, Vec<cedian_protocol::Line>)>, cx: &mut App) -> anyhow::Result<()> {
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
            .map(|(path, _)| format!("replay {}", path.display()))
            .unwrap_or_else(|| "no thread".to_owned());
        let agent = cx.new(|_| AgentPane::new(title));
        if let Some((_, lines)) = replay {
            replay::play(lines, agent.clone(), cx);
        }
        cx.new(|_| RootView::new(agent, tools))
    })
    .context("opening the window")?;
    cx.activate(true);
    Ok(())
}
