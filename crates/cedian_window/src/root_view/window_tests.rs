//! The window drawn offline, with real fonts and no daemon. At the sizes it opens at, each
//! part of the composer's bottom row and each menu stays inside what holds it: a part that
//! leaks out (clipped, over a neighbour block, off the window) fails here before a screenshot
//! shows it. And `⌘K` finds layouts and blocks as well as threads.

use std::path::PathBuf;

use cedian_protocol::Line;
use cedian_wire::Notice;
use gpui::{
    AppContext as _, Bounds, Pixels, TestAppContext, TestDispatcher, VisualTestContext, px, size,
};

use super::{Kind, RootView};
use crate::{agent_pane::AgentPane, thread_search};

const MODELS: &str = r#"{"default":"faux/scripted","models":[
  {"ref":"faux/scripted","label":"Faux (scripted)","provider":"faux","available":true,"contextWindow":200000,"effort":false},
  {"ref":"anthropic/claude-opus-5-5-fast","alias":"opus-fast","label":"Claude Opus 5.5 (fast, a long name to shrink)","provider":"anthropic","available":true,"contextWindow":1000000,"effort":true},
  {"ref":"opencode-go/deepseek-v4.1-flash","label":"DeepSeek V4.1 Flash","provider":"opencode-go","available":false,"reason":"OPENCODE_API_KEY is not set","contextWindow":1000000,"effort":false}]}"#;
const PERMISSIONS: &str = r#"{"default":"workspace","modes":[
  {"mode":"read-only","label":"Read only","description":"Reads and runs nothing that writes."},
  {"mode":"plan","label":"Plan","description":"Reads, then ends with a plan and asks for the go-ahead."},
  {"mode":"workspace","label":"Workspace","description":"Edits and runs commands in the project."}]}"#;
const STREAM: &str =
    include_str!("../../../cedian_protocol/cedia/sessions/edit-and-check/stream.jsonl");

/// The sizes the window takes: its smallest, the default, and a wide one.
const SIZES: [(f32, f32); 7] = [
    (1024., 640.),
    (1280., 800.),
    (1440., 900.),
    // The Agent block just past the width it shows the chips from.
    (1550., 960.),
    (1600., 1000.),
    (1800., 1100.),
    (2560., 1440.),
];

fn app() -> TestAppContext {
    let text_system = gpui_platform::current_platform(true).text_system();
    let cx = TestAppContext::build_with_text_system(TestDispatcher::new(0), None, text_system);
    cx.update(|cx| {
        crate::assets::Assets.load_fonts(cx).expect("fonts");
        settings::init(cx);
        theme_settings::init(theme::LoadThemes::All(Box::new(crate::assets::Assets)), cx);
        release_channel::init(semver::Version::new(0, 1, 0), cx);
        editor::init(cx);
        crate::desktop::Desktop::from_value(&serde_json::Value::Null).apply(cx);
    });
    cx
}

/// A window on one project with a thread open, as the daemon would leave it, but with no
/// daemon: notices go straight to the view.
fn window(cx: &mut TestAppContext) -> (gpui::Entity<RootView>, &mut VisualTestContext) {
    let cwd = PathBuf::from("/fixture/repo");
    let (view, cx) = cx.add_window_view(|window, cx| {
        let agent = cx.new(|_| AgentPane::new("fixture"));
        let mut view = RootView::new(vec![cwd.clone()], 0, agent, window, cx);
        let mut projects = std::mem::take(&mut view.projects);
        for project in &mut projects {
            view.load_blocks(project);
        }
        view.projects = projects;
        view.focused = view.ws().find(Kind::Agent);
        view.set_search_actions(cx);
        view
    });
    view.update_in(cx, |view, _, cx| {
        view.on_notice(
            Notice::Choices {
                models: Some(serde_json::from_str(MODELS).expect("models")),
                permissions: Some(serde_json::from_str(PERMISSIONS).expect("permissions")),
            },
            cx,
        );
        view.projects[0].current = Some("t-00000001".into());
        view.agent.update(cx, |agent, cx| {
            for line in STREAM.lines() {
                agent.apply(&Line::parse(line).expect("fixture line"), cx);
            }
        });
    });
    (view, cx)
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} was not drawn"))
}

#[track_caller]
fn inside(part: &str, inner: Bounds<Pixels>, outer_name: &str, outer: Bounds<Pixels>) {
    // Half a pixel for rounding.
    let slack = px(0.5);
    assert!(
        inner.left() >= outer.left() - slack
            && inner.right() <= outer.right() + slack
            && inner.top() >= outer.top() - slack
            && inner.bottom() <= outer.bottom() + slack,
        "{part} {inner:?} leaks out of {outer_name} {outer:?}"
    );
}

#[track_caller]
fn apart(a_name: &str, a: Bounds<Pixels>, b_name: &str, b: Bounds<Pixels>) {
    assert!(!a.intersects(&b), "{a_name} {a:?} overlaps {b_name} {b:?}");
}

#[test]
fn the_composer_row_keeps_its_chips_inside_at_every_size() {
    let mut app = app();
    let (_, cx) = window(&mut app);
    for (width, height) in SIZES {
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();
        let block = bounds(cx, "agent-block");
        let row = bounds(cx, "composer-row");
        inside("the composer row", row, "the Agent block", block);
        let model = bounds(cx, "run-model");
        let permission = bounds(cx, "run-permission");
        inside("the model chip", model, "the composer row", row);
        inside("the permission chip", permission, "the composer row", row);
        apart("the model chip", model, "the permission chip", permission);
        // A narrow block gives the context and the view mode up to the run's choices.
        if let Some(chips) = cx.debug_bounds("agent-chips") {
            inside("the agent chips", chips, "the composer row", row);
            for part in ["context-chip", "view-modes"] {
                let chip = bounds(cx, part);
                inside(part, chip, "the agent chips", chips);
                apart(part, chip, "the permission chip", permission);
            }
        }
    }
}

#[test]
fn the_run_menus_open_inside_the_agent_block_and_the_window() {
    let mut app = app();
    let (view, cx) = window(&mut app);
    for (width, height) in SIZES {
        cx.simulate_resize(size(px(width), px(height)));
        for menu in [
            super::run_choices::Menu::Model,
            super::run_choices::Menu::Permission,
        ] {
            view.update(cx, |view, cx| {
                view.choices.menu = Some(menu);
                cx.notify();
            });
            cx.run_until_parked();
            let viewport = cx.update(|window, _| window.viewport_size());
            let window = Bounds::new(gpui::Point::default(), viewport);
            let menu_bounds = bounds(cx, "run-choice-menu");
            inside(
                &format!("the {menu:?} menu"),
                menu_bounds,
                "the window",
                window,
            );
            let block = bounds(cx, "agent-block");
            inside(
                &format!("the {menu:?} menu"),
                menu_bounds,
                "the Agent block",
                block,
            );
        }
    }
}

#[test]
fn search_adds_a_block_and_switches_layouts_by_name() {
    let mut app = app();
    let (view, cx) = window(&mut app);
    let pick = |cx: &mut VisualTestContext, query: &str| {
        view.update_in(cx, |view, window, cx| {
            view.search
                .update(cx, |search, cx| search.focus(window, cx))
        });
        cx.simulate_input(query);
        cx.dispatch_action(thread_search::Confirm);
        cx.run_until_parked();
    };
    assert!(view.read_with(cx, |view, _| view.ws().find(Kind::Git).is_none()));
    pick(cx, "ใหม่ git");
    assert!(view.read_with(cx, |view, _| view.ws().find(Kind::Git).is_some()));
    pick(cx, "layout เขียน");
    assert_eq!(view.read_with(cx, |view, _| view.ws().current), 1);
}
