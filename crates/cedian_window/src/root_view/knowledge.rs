//! The Outputs and Memory blocks' data (Knowledge): asked of the daemon when either block
//! comes into view, when the front project changes, and whenever the project's threads change,
//! since a run is what changes files and keeps notes.

use cedian_layout::Kind;
use cedian_wire::Request;
use gpui::Context;

use super::RootView;

impl RootView {
    /// Asks for what the shown Outputs and Memory blocks list.
    pub(super) fn refresh_knowledge(&self) {
        let cwd = self.project().cwd.clone();
        if self.showing(Kind::Outputs) {
            self.request(Request::Outputs { cwd: cwd.clone() });
        }
        if self.showing(Kind::Memory) {
            self.request(Request::Memory { cwd });
        }
    }

    /// The front project changed: the blocks forget the last one's and ask again.
    pub(super) fn reset_knowledge(&mut self, cx: &mut Context<Self>) {
        self.outputs.update(cx, |pane, cx| pane.clear(cx));
        self.memory.update(cx, |pane, cx| pane.clear(cx));
        self.refresh_knowledge();
    }
}
