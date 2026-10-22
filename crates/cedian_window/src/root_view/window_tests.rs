//! The window drawn offline, with real fonts and no daemon. At the sizes it opens at, each
//! part of the composer's bottom row and each menu stays inside what holds it: a part that
//! leaks out (clipped, over a neighbour block, off the window) fails here before a screenshot
//! shows it. And `⌘K` finds layouts and blocks as well as threads.

use std::path::PathBuf;

use cedian_protocol::Line;
use cedian_wire::Notice;
use gpui::{
    AppContext as _, Bounds, Focusable as _, Pixels, TestAppContext, TestDispatcher,
    VisualContext as _, VisualTestContext, px, size,
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

#[test]
fn a_cut_label_shows_its_whole_text_on_hover_and_one_that_fits_does_not() {
    let mut app = app();
    let (view, cx) = window(&mut app);
    cx.simulate_resize(size(px(1280.), px(800.)));
    let hover_model = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        let chip = bounds(cx, "run-model");
        cx.simulate_mouse_move(chip.center(), None, gpui::Modifiers::none());
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();
        let shown = cx.debug_bounds("tooltip").is_some();
        cx.simulate_mouse_move(gpui::Point::default(), None, gpui::Modifiers::none());
        cx.run_until_parked();
        shown
    };
    assert!(!hover_model(cx), "Faux (scripted) fits");
    view.update(cx, |view, cx| {
        view.choices.pick_mut(Some("t-00000001")).model =
            Some("anthropic/claude-opus-5-5-fast".into());
        cx.notify();
    });
    assert!(hover_model(cx), "the long model name is cut");
}

#[test]
fn the_file_finder_matches_letters_in_order_and_opens_the_best() {
    use crate::file_finder::{FileFinder, FinderEvent, Pick};
    let mut app = app();
    let files: Vec<PathBuf> = [
        "README.md",
        "crates/cedian_window/src/root_view.rs",
        "crates/cedian_window/src/root_view/blocks.rs",
        "crates/cedian_layout/src/block.rs",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    let (finder, cx) = app
        .add_window_view(|window, cx| FileFinder::new(PathBuf::from("/repo"), &files, window, cx));
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    let seen = opened.clone();
    cx.update(|window, cx| {
        window.focus(&finder.focus_handle(cx), cx);
        cx.subscribe(&finder, move |_, event, _| {
            if let FinderEvent::Open(path) = event {
                *seen.borrow_mut() = Some(path.clone());
            }
        })
        .detach();
    });
    cx.simulate_input("rvblk");
    cx.run_until_parked();
    cx.dispatch_action(Pick);
    assert_eq!(
        opened.borrow().as_deref(),
        Some(std::path::Path::new(
            "/repo/crates/cedian_window/src/root_view/blocks.rs"
        ))
    );
}

#[test]
fn at_lists_files_and_a_pick_puts_the_path_in_the_text() {
    use crate::composer::{CommandPick, Composer};
    let mut app = app();
    let files: Vec<PathBuf> = ["README.md", "crates/cedian_window/src/root_view/blocks.rs"]
        .iter()
        .map(PathBuf::from)
        .collect();
    let (composer, cx) = app.add_window_view(|window, cx| {
        let mut composer = Composer::new(window, cx);
        composer.set_files(std::path::Path::new("/repo"), &files);
        composer
    });
    composer.update_in(cx, |composer, window, cx| composer.focus(window, cx));
    cx.simulate_input("look at @rvblk");
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("mention-list").is_some(),
        "the files are listed"
    );
    cx.dispatch_action(CommandPick);
    cx.run_until_parked();
    assert_eq!(
        composer.read_with(cx, |composer, cx| composer.text(cx)),
        "look at @crates/cedian_window/src/root_view/blocks.rs "
    );
    assert!(cx.debug_bounds("mention-list").is_none(), "the list closes");
}

#[test]
fn a_block_let_go_outside_the_window_opens_in_a_new_one() {
    let mut app = app();
    let (view, cx) = window(&mut app);
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    let agent = view.read_with(cx, |view, _| view.ws().find(Kind::Agent));
    let block = bounds(cx, "agent-block");
    let grip = gpui::point(block.center().x, block.top() + px(18.));
    let outside = gpui::point(px(1400.), px(300.));
    let button = Some(gpui::MouseButton::Left);
    cx.simulate_mouse_down(grip, gpui::MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(
        grip + gpui::point(px(40.), px(0.)),
        button,
        gpui::Modifiers::none(),
    );
    cx.simulate_mouse_move(outside, button, gpui::Modifiers::none());
    cx.simulate_mouse_up(outside, gpui::MouseButton::Left, gpui::Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.windows().len(), 2, "a second window opened");
    view.read_with(cx, |view, _| {
        assert!(
            view.ws().find(Kind::Agent).is_none(),
            "the block left this window"
        );
        assert!(agent.is_some_and(|id| view.ws().kind(id).is_none()));
    });
    let here = cx.window_handle().window_id();
    let torn = cx
        .windows()
        .into_iter()
        .filter(|w| w.window_id() != here)
        .find_map(|w| w.downcast::<RootView>());
    let torn = torn.expect("the new window holds a RootView");
    cx.update(|_, cx| {
        torn.read_with(cx, |torn, _| {
            assert!(torn.torn);
            assert!(torn.ws().find(Kind::Agent).is_some());
            assert!(torn.ws().find(Kind::Threads).is_none());
            assert_eq!(
                torn.layout.open.get("/fixture/repo").map(String::as_str),
                Some("t-00000001")
            );
        })
        .expect("the torn window is open");
    });
}

#[test]
fn a_block_let_go_inside_the_window_stays() {
    let mut app = app();
    let (view, cx) = window(&mut app);
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    let block = bounds(cx, "agent-block");
    let grip = gpui::point(block.center().x, block.top() + px(18.));
    // The title bar: in the window, over no block.
    let above = gpui::point(px(600.), px(8.));
    let button = Some(gpui::MouseButton::Left);
    cx.simulate_mouse_down(grip, gpui::MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(
        grip + gpui::point(px(40.), px(0.)),
        button,
        gpui::Modifiers::none(),
    );
    cx.simulate_mouse_move(above, button, gpui::Modifiers::none());
    cx.simulate_mouse_up(above, gpui::MouseButton::Left, gpui::Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.windows().len(), 1);
    assert!(view.read_with(cx, |view, _| view.ws().find(Kind::Agent).is_some()));
}
