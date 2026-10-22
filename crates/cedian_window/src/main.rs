mod spike_view;

use anyhow::Context as _;
use gpui::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use settings::KeymapFile;

use crate::spike_view::SpikeView;

fn main() {
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            if let Err(error) = open(cx) {
                log::error!("{error:#}");
                eprintln!("cedian: {error:#}");
                cx.quit();
            }
        });
}

fn open(cx: &mut App) -> anyhow::Result<()> {
    assets::Assets.load_fonts(cx).context("loading fonts")?;
    settings::init(cx);
    theme_settings::init(theme::LoadThemes::JustBase, cx);
    release_channel::init(semver::Version::new(0, 1, 0), cx);
    editor::init(cx);
    let bindings = KeymapFile::load_asset_allow_partial_failure(settings::DEFAULT_KEYMAP_PATH, cx)
        .context("loading the default keymap")?;
    cx.bind_keys(bindings);

    let bounds = Bounds::centered(None, size(px(960.), px(640.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| SpikeView::new(window, cx))
    })
    .context("opening the window")?;
    cx.activate(true);
    Ok(())
}
