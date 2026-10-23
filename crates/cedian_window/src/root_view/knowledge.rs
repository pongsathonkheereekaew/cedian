//! The Outputs and Memory blocks' data (Knowledge): asked of the daemon when either block
//! comes into view, when the front project changes, and whenever the project's threads change,
//! since a run is what changes files and keeps notes. The Skills block's list too, asked when
//! it comes into view and when the front project changes.

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

    /// Asks for what the shown Skills block lists: the project's commands, as `/` does. Not
    /// with every change to the threads, since a run does not change the skills.
    pub(super) fn refresh_skills(&self) {
        if self.showing(Kind::Skills) {
            self.request(Request::Commands {
                cwd: self.project().cwd.clone(),
            });
        }
    }

    /// The front project changed: the blocks forget the last one's and ask again.
    pub(super) fn reset_knowledge(&mut self, cx: &mut Context<Self>) {
        self.outputs.update(cx, |pane, cx| pane.clear(cx));
        self.memory.update(cx, |pane, cx| pane.clear(cx));
        self.skills.update(cx, |pane, cx| pane.clear(cx));
        self.refresh_knowledge();
        self.refresh_skills();
    }
}
