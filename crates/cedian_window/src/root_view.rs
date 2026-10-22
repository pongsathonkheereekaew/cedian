//! The window's layout while phase A has one thread: the agent pane beside the editor and
//! terminal.

use gpui::{Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window, div};
use theme::ActiveTheme as _;

use crate::{agent_pane::AgentPane, spike_view::SpikeView};

pub struct RootView {
    agent: Entity<AgentPane>,
    tools: Entity<SpikeView>,
}

impl RootView {
    pub fn new(agent: Entity<AgentPane>, tools: Entity<SpikeView>) -> Self {
        Self { agent, tools }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .bg(cx.theme().colors().background)
            .child(
                div()
                    .w_1_2()
                    .h_full()
                    .border_r_1()
                    .border_color(cx.theme().colors().border)
                    .child(self.agent.clone()),
            )
            .child(div().w_1_2().h_full().child(self.tools.clone()))
    }
}
