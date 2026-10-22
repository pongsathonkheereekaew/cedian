//! A block dropped outside the window opens in a new window (Interface, Blocks: Moving with the
//! mouse), for a second display. The new window has its own tree, the block alone at first, and
//! its own connection to the same daemon; it shows the thread the block showed. Its tree is not
//! saved: `desktop.layout` keeps the first window's, and a closed window is rebuilt by dragging
//! out again. A block's own state that lives in the window goes with nothing: a Terminal starts
//! a new shell in the same directory, the Editor opens empty.

use std::path::PathBuf;

use cedian_layout::{BlockId, Kind, Workspace};
use gpui::{
    AppContext as _, Bounds, Context, Pixels, Point, Window, WindowBounds, WindowOptions, point,
    px, size,
};

use super::RootView;
use crate::{layout::Layout, view_mode::ViewMode};

/// The smallest a torn-off window gets, and its size when the block's own is not known.
const MIN_WIDTH: f32 = 360.;
const MIN_HEIGHT: f32 = 240.;
const FALLBACK: (f32, f32) = (720., 560.);
/// Where the mouse holds the new window: about where it held the block's header.
const GRIP: (f32, f32) = (48., 18.);

/// Where a block dropped at `at` (in the window, possibly outside it) opens: its window's
/// top-left on the screen, keeping the block's size.
pub fn torn_bounds(
    window: Bounds<Pixels>,
    at: Point<Pixels>,
    block: Option<(f32, f32)>,
) -> Bounds<Pixels> {
    let (w, h) = block.unwrap_or(FALLBACK);
    let origin = window.origin + at - point(px(GRIP.0), px(GRIP.1));
    Bounds::new(origin, size(px(w.max(MIN_WIDTH)), px(h.max(MIN_HEIGHT))))
}

/// Whether `at`, in the window, is outside it.
pub fn outside(viewport: gpui::Size<Pixels>, at: Point<Pixels>) -> bool {
    !Bounds::new(Point::default(), viewport).contains(&at)
}

impl RootView {
    /// The view of a torn-off window: `kind` alone, on `thread` of the project at `cwd`.
    pub fn torn_off(
        cwd: PathBuf,
        kind: Kind,
        thread: Option<String>,
        view_mode: ViewMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut layout = Layout::default();
        if let Some(thread) = thread {
            // The first list opens the thread named here.
            layout.open.insert(cwd.display().to_string(), thread);
        }
        // Tests have no daemon to connect to.
        let mut view = if cfg!(test) {
            let agent = cx.new(|_| crate::agent_pane::AgentPane::new("new thread"));
            let mut view = Self::new(vec![cwd], 0, agent, window, cx);
            view.layout = layout;
            view
        } else {
            Self::live(vec![cwd], 0, layout, view_mode, window, cx)
        };
        view.torn = true;
        view.project_mut().workspace = Workspace::alone(kind);
        view.focused = view.ws().find(kind);
        view.refocus = true;
        view.set_search_actions(cx);
        view
    }

    /// Opens block `id` in a new window where the mouse let go of it, at `at`, and closes it
    /// here. The thread list stays: it is the one block that cannot close.
    pub(super) fn tear_off(
        &mut self,
        id: BlockId,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(kind) = self.ws().kind(id).filter(|k| k.closable()) else {
            return;
        };
        if matches!(self.connection, super::Connection::Replay) {
            return;
        }
        let block = self
            .shown_rects()
            .into_iter()
            .find(|(shown, _)| *shown == id)
            .map(|(_, rect)| (rect.w, rect.h));
        let bounds = torn_bounds(window.bounds(), at, block);
        let cwd = self.project().cwd.clone();
        let thread = self.project().current.clone();
        let view_mode = self.agent.read(cx).view_mode();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(MIN_WIDTH), px(MIN_HEIGHT))),
            titlebar: Some(crate::titlebar()),
            ..Default::default()
        };
        let opened = cx.open_window(options, move |window, cx| {
            cx.new(|cx| RootView::torn_off(cwd, kind, thread, view_mode, window, cx))
        });
        match opened {
            Ok(_) => self.close_block(id, cx),
            Err(error) => log::error!("opening a window for the block: {error:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_torn_block_keeps_its_size_under_the_mouse() {
        let window = Bounds::new(point(px(100.), px(50.)), size(px(1280.), px(800.)));
        let bounds = torn_bounds(window, point(px(1400.), px(300.)), Some((500., 400.)));
        assert_eq!(bounds.origin, point(px(1452.), px(332.)));
        assert_eq!(bounds.size, size(px(500.), px(400.)));
        // A sliver of a block opens at the smallest size.
        let small = torn_bounds(window, point(px(0.), px(0.)), Some((120., 90.)));
        assert_eq!(small.size, size(px(MIN_WIDTH), px(MIN_HEIGHT)));
    }

    #[test]
    fn only_a_point_past_the_edge_is_outside() {
        let viewport = size(px(1280.), px(800.));
        assert!(!outside(viewport, point(px(10.), px(10.))));
        assert!(outside(viewport, point(px(-4.), px(10.))));
        assert!(outside(viewport, point(px(1300.), px(10.))));
    }
}
