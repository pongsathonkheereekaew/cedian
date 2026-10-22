//! The tools and their toggles (Interface, Polish: the rail became 30 px icon toggles in the
//! sheet's header, same keys and order). A click toggles a tool; the open ones stack in the
//! panel in rail order. `⌘⌥1`–`⌘⌥7` toggle them by their place in the spec's rail (Browser, not built yet,
//! keeps `⌘⌥5`) and `⌘⌥0` closes them all.

use gpui::{
    App, ClickEvent, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Stateful, StatefulInteractiveElement as _, Styled as _, Window, actions, div,
    prelude::FluentBuilder as _, px, svg,
};

/// What a rail button does when clicked.
pub type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
use theme::ActiveTheme as _;

actions!(
    tools,
    [
        ToggleEditor,
        ToggleTerminal,
        ToggleReview,
        ToggleFiles,
        ToggleGit,
        ToggleDetails,
        HideTools
    ]
);

pub const PANEL_WIDTH: f32 = 456.;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tool {
    Editor,
    Terminal,
    Review,
    /// The open thread's directory as a tree.
    Files,
    /// The directory's branch, changes and latest commits, read only.
    Git,
    /// Thread details: where it runs, its sessions, cost and context.
    Details,
}

impl Tool {
    /// Rail order, which is also the order open tools stack in.
    pub const ALL: [Tool; 6] = [
        Tool::Editor,
        Tool::Terminal,
        Tool::Review,
        Tool::Files,
        Tool::Git,
        Tool::Details,
    ];

    /// The name in `desktop.layout.tools`.
    pub fn key(self) -> &'static str {
        match self {
            Tool::Editor => "editor",
            Tool::Terminal => "terminal",
            Tool::Review => "review",
            Tool::Files => "files",
            Tool::Git => "git",
            Tool::Details => "details",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.key() == key)
    }

    pub fn label(self) -> &'static str {
        match self {
            Tool::Editor => "Editor",
            Tool::Terminal => "Terminal",
            Tool::Review => "Review",
            Tool::Files => "Files",
            Tool::Git => "Git",
            Tool::Details => "รายละเอียด thread",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Tool::Editor => "icons/file_code.svg",
            Tool::Terminal => "icons/terminal.svg",
            Tool::Review => "icons/file_diff.svg",
            Tool::Files => "icons/file_tree.svg",
            Tool::Git => "icons/git_branch.svg",
            Tool::Details => "icons/info.svg",
        }
    }
}

/// A divider between two open tools being dragged: the one under the `n`th tool.
#[derive(Clone, Copy, Debug)]
pub struct DividerDrag(pub usize);

impl Render for DividerDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // The divider itself moves; nothing follows the pointer.
        div()
    }
}

/// The smallest share of a pair a drag leaves either tool.
const MIN_SHARE: f32 = 0.15;

/// Moves the divider under tool `divider` to `at`, a fraction of the panel's height, taking
/// the space from the tool on its other side; the rest keep theirs.
pub fn resize(weights: &mut [f32], divider: usize, at: f32) {
    if divider + 1 >= weights.len() {
        return;
    }
    let total: f32 = weights.iter().sum();
    if total <= 0. {
        return;
    }
    let before: f32 = weights[..divider].iter().sum::<f32>() / total;
    let pair = weights[divider] + weights[divider + 1];
    let span = pair / total;
    let share = ((at - before) / span).clamp(MIN_SHARE, 1. - MIN_SHARE);
    weights[divider] = pair * share;
    weights[divider + 1] = pair * (1. - share);
}

/// Opens `tool` if it is closed and closes it if open, keeping rail order.
pub fn toggle(open: &mut Vec<Tool>, tool: Tool) {
    if let Some(at) = open.iter().position(|t| *t == tool) {
        open.remove(at);
    } else {
        open.push(tool);
        open.sort();
    }
}

/// The toggles: a button per tool, lit when open, Review with its count of hunks waiting.
pub fn toggles(open: &[Tool], waiting: usize, cx: &App, click: impl Fn(Tool) -> OnClick) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap_0p5()
        .children(Tool::ALL.into_iter().map(|tool| {
            let on = open.contains(&tool);
            let badge = (tool == Tool::Review && waiting > 0).then_some(waiting);
            button(tool, on, badge, cx).on_click(click(tool))
        }))
}

fn button(tool: Tool, on: bool, badge: Option<usize>, cx: &App) -> Stateful<Div> {
    let colors = cx.theme().colors();
    div()
        .id(tool.key())
        .relative()
        .size(px(30.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .cursor_pointer()
        .when(on, |b| b.bg(colors.element_selected))
        .hover(|b| b.bg(colors.element_hover))
        .child(svg().path(tool.icon()).size(px(16.)).text_color(if on {
            colors.text
        } else {
            colors.text_muted
        }))
        .children(badge.map(|count| {
            div()
                .absolute()
                .top(px(-3.))
                .right(px(-3.))
                .min_w(px(15.))
                .h(px(15.))
                .px(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(colors.text)
                .text_color(colors.background)
                .text_size(px(10.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(count.to_string())
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_divider_moves_between_its_two_tools() {
        let mut weights = [1., 1., 1.];
        resize(&mut weights, 0, 0.5);
        assert!((weights[0] - 1.5).abs() < 1e-4 && (weights[1] - 0.5).abs() < 1e-4);
        assert_eq!(weights[2], 1.);
        // Never past the other tool's floor.
        resize(&mut weights, 1, 1.);
        assert!((weights[2] - 1.5 * MIN_SHARE).abs() < 1e-4);
        resize(&mut weights, 2, 0.5);
        assert!((weights.iter().sum::<f32>() - 3.).abs() < 1e-4);
    }

    #[test]
    fn toggling_keeps_rail_order() {
        let mut open = vec![Tool::Terminal];
        toggle(&mut open, Tool::Details);
        toggle(&mut open, Tool::Editor);
        assert_eq!(open, [Tool::Editor, Tool::Terminal, Tool::Details]);
        toggle(&mut open, Tool::Terminal);
        assert_eq!(open, [Tool::Editor, Tool::Details]);
        for tool in Tool::ALL {
            assert_eq!(Tool::from_key(tool.key()), Some(tool));
        }
    }
}
