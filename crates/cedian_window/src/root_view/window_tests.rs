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
  {"ref":"anthropic/claude-opus-5-5-fast","alias":"opus-fast","label":"Claude Opus 5.5 (fast, a long name to shrink)","provider":"anthropic","available":true,"contextWindow":1000000,"effort":true,"effortLevels":["low","medium","high","xhigh","max"],"defaultEffort":"medium"},
  {"ref":"opencode-go/deepseek-v4.1-flash","label":"DeepSeek V4.1 Flash","provider":"opencode-go","available":false,"reason":"OPENCODE_API_KEY is not set","contextWindow":1000000,"effort":false}]}"#;
const PERMISSIONS: &str = r#"{"default":"workspace","modes":[
  {"mode":"read-only","label":"Read only","description":"Reads and runs nothing that writes."},
  {"mode":"plan","label":"Plan","description":"Reads, then ends with a plan and asks for the go-ahead."},
  {"mode":"workspace","label":"Workspace","description":"Edits and runs commands in the project."}]}"#;
const STREAM: &str =
    include_str!("../../../cedian_protocol/cedia/sessions/edit-and-check/stream.jsonl");

/// The sizes the window takes: its smallest, the default, and a wide one.
pub(super) const SIZES: [(f32, f32); 7] = [
    (1024., 640.),
    (1280., 800.),
    (1440., 900.),
    // The Agent block just past the width it shows the chips from.
    (1550., 960.),
    (1600., 1000.),
    (1800., 1100.),
    (2560., 1440.),
];

pub(super) fn app() -> TestAppContext {
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
pub(super) fn window(cx: &mut TestAppContext) -> (gpui::Entity<RootView>, &mut VisualTestContext) {
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

pub(super) fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} was not drawn"))
}

#[track_caller]
pub(super) fn inside(part: &str, inner: Bounds<Pixels>, outer_name: &str, outer: Bounds<Pixels>) {
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
pub(super) fn apart(a_name: &str, a: Bounds<Pixels>, b_name: &str, b: Bounds<Pixels>) {
    assert!(!a.intersects(&b), "{a_name} {a:?} overlaps {b_name} {b:?}");
}

#[test]
fn the_composer_row_keeps_its_chips_inside_at_every_size() {
    let mut app = app();
    let (view, cx) = window(&mut app);
    // The faux writer takes no effort; a model that takes one shows it after its name.
    // The second pass also turns helpers on, which the permission chip counts.
    for model in [None, Some("anthropic/claude-opus-5-5-fast")] {
        view.update(cx, |view, cx| {
            let pick = view.choices.pick_mut(Some("t-00000001"));
            pick.model = model.map(str::to_owned);
            // With helpers on, the permission chip says how many.
            pick.roles[0] = model.map(|_| "none".to_owned());
            pick.goal = model.is_some();
            cx.notify();
        });
        for (width, height) in SIZES {
            cx.simulate_resize(size(px(width), px(height)));
            cx.run_until_parked();
            let block = bounds(cx, "agent-block");
            let row = bounds(cx, "composer-row");
            inside("the composer row", row, "the Agent block", block);
            let chip = bounds(cx, "run-model");
            let permission = bounds(cx, "run-permission");
            inside("the model chip", chip, "the composer row", row);
            inside("the permission chip", permission, "the composer row", row);
            apart("the model chip", chip, "the permission chip", permission);
            let effort = cx.debug_bounds("run-effort");
            assert_eq!(effort.is_some(), model.is_some(), "the effort");
            if let Some(effort) = effort {
                inside("the effort", effort, "the model chip", chip);
            }
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
}

#[test]
fn the_run_menus_open_inside_the_agent_block_and_the_window() {
    let mut app = app();
    let (view, cx) = window(&mut app);
    view.update(cx, |view, _| {
        view.choices.pick_mut(Some("t-00000001")).model =
            Some("anthropic/claude-opus-5-5-fast".into());
    });
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
fn an_image_goes_with_the_next_run_and_another_file_as_a_mention() {
    use crate::composer::{Composer, ComposerEvent, Submit};
    let dir = tempfile::tempdir().expect("dir");
    let shot = dir.path().join("shot.png");
    let notes = dir.path().join("notes.md");
    std::fs::write(&shot, b"not decoded here").expect("shot");
    std::fs::write(&notes, "# notes").expect("notes");
    let mut app = app();
    let (composer, cx) = app.add_window_view(|window, cx| {
        let mut composer = Composer::new(window, cx);
        composer.set_files(dir.path(), &[]);
        composer.set_can_attach(true, cx);
        composer
    });
    let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = sent.clone();
    cx.update(|_, cx| {
        cx.subscribe(&composer, move |_, event, _| {
            if let ComposerEvent::Submit(text, images) = event {
                seen.borrow_mut().push((text.clone(), images.clone()));
            }
        })
        .detach();
    });
    composer.update_in(cx, |composer, window, cx| {
        composer.focus(window, cx);
        composer.add_files(&[shot.clone(), notes.clone()], window, cx);
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("attached-image-0").is_some(),
        "a tile for the image"
    );
    assert_eq!(
        composer.read_with(cx, |composer, cx| composer.text(cx)),
        "@notes.md "
    );

    // While the run works, a send would be a message, which carries no image.
    composer.update(cx, |composer, cx| composer.set_working(true, cx));
    cx.simulate_input("why is it cut off");
    cx.dispatch_action(Submit);
    cx.run_until_parked();
    assert!(sent.borrow().is_empty(), "held until the run ends");
    assert_eq!(composer.read_with(cx, |c, _| c.images().len()), 1);

    composer.update(cx, |composer, cx| composer.set_working(false, cx));
    cx.dispatch_action(Submit);
    cx.run_until_parked();
    assert_eq!(
        sent.borrow().as_slice(),
        [("@notes.md why is it cut off".to_owned(), vec![shot])]
    );
    assert!(composer.read_with(cx, |c, _| c.images().is_empty()));
    assert!(cx.debug_bounds("attached-image-0").is_none());
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

#[test]
fn the_outputs_and_memory_blocks_keep_their_rows_inside_at_every_size() {
    use cedian_index::{MemoryNote, Output};
    let mut app = app();
    let (view, cx) = window(&mut app);
    let output = |path: &str, title: &str, exists: bool| Output {
        cwd: PathBuf::from("/fixture/repo"),
        path: path.to_owned(),
        thread: "t-00000001".to_owned(),
        title: title.to_owned(),
        at: "2026-10-22T09:42:57.225Z".to_owned(),
        exists,
    };
    let note = |id: &str, text: &str, from: &str| MemoryNote {
        id: id.to_owned(),
        text: text.to_owned(),
        from: from.to_owned(),
        at: "2026-10-21T10:00:00Z".to_owned(),
    };
    view.update(cx, |view, cx| {
        for kind in [Kind::Outputs, Kind::Memory] {
            view.open_kind(kind, super::blocks::ToolTarget { against: None }, cx);
        }
        let cwd = view.project().cwd.clone();
        view.on_notice(
            Notice::Outputs {
                cwd: cwd.clone(),
                outputs: vec![
                    output("f.txt", "make f.txt say good", true),
                    output(
                        "crates/cedian_window/src/root_view/a_very_long_file_name_that_will_not_fit.rs",
                        "แก้บล็อก Memory และ Outputs ให้แสดงไฟล์ทั้งหมดที่ thread ในโปรเจกต์สร้างขึ้นมา",
                        false,
                    ),
                ],
            },
            cx,
        );
        view.on_notice(
            Notice::Memory {
                cwd,
                notes: vec![
                    note("a1b2c3d4", "bun test runs the whole suite; cargo test -p cedian_window runs the leak test", "user"),
                    note(
                        "e5f6a7b8",
                        "หน้าต่างเป็นแค่ตัวแสดงผล daemon เป็นเจ้าของสถานะทั้งหมด และไฟล์ thread ของ cedia เขียนต่อท้ายอย่างเดียว ~/.cedia/projects/an-unbroken-path-that-is-long-enough-to-overflow-a-narrow-block/memory.jsonl",
                        "3f2a9c1e",
                    ),
                ],
            },
            cx,
        );
    });
    let mut drawn = Vec::new();
    for (width, height) in SIZES {
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();
        for (block, rows) in [
            (
                "outputs-block",
                &["output-0", "output-1", "output-path-1", "output-meta-1"][..],
            ),
            ("memory-block", &["note-0", "note-1", "note-text-1"][..]),
        ] {
            // A small window folds these blocks into a neighbour's tabs.
            let Some(outer) = cx.debug_bounds(block) else {
                continue;
            };
            drawn.push((width, block));
            let first = bounds(cx, rows[0]);
            inside(rows[0], first, block, outer);
            for &row in rows {
                // Rows under the fold scroll; across, every one stays in.
                let row_bounds = bounds(cx, row);
                assert!(
                    row_bounds.left() >= outer.left() - px(0.5)
                        && row_bounds.right() <= outer.right() + px(0.5),
                    "{row} {row_bounds:?} leaks out of {block} {outer:?} at {width}"
                );
            }
        }
    }
    for block in ["outputs-block", "memory-block"] {
        assert!(
            drawn.contains(&(2560., block)),
            "{block} is drawn in the widest window: {drawn:?}"
        );
    }
    assert_eq!(
        view.read_with(cx, |view, cx| view.memory.read(cx).count()),
        2
    );
}

#[test]
fn the_usage_block_draws_the_report_inside_itself_and_toggles_a_table() {
    let mut app = app();
    let (view, cx) = window(&mut app);
    cx.simulate_resize(size(px(1280.), px(800.)));
    view.update(cx, |view, cx| {
        view.open_kind(Kind::Usage, super::blocks::ToolTarget { against: None }, cx);
    });
    cx.run_until_parked();
    let today = chrono::Local::now().date_naive();
    let report = format!(
        r#"{{"sessions":3,"requests":12,"costUsd":1.5,"keepaliveUsd":0,"compactUsd":0,
        "tokens":{{"input":1,"cacheRead":0,"output":1}},"sources":[],"largest":[],"misses":[],
        "overPriceLine":0,"maxEffort":0,"handoffs":0,"unpriced":0,
        "days":[{{"day":"{}","costUsd":1.5}}],
        "models":[{{"model":"anthropic/claude-opus-5-5","costUsd":1.2,"requests":9}},{{"model":"faux/scripted","costUsd":0.3,"requests":3}}],
        "projects":[{{"project":"fixture-repo","costUsd":1.5}}],
        "threads":[{{"thread":"t-1","title":"a thread whose title is long enough to be cut short in a narrow block","project":"fixture-repo","costUsd":1.5}}]}}"#,
        today.format("%Y-%m-%d")
    );
    view.update(cx, |view, cx| {
        view.on_notice(
            Notice::Usage {
                cwd: Some(PathBuf::from("/fixture/repo")),
                since: "7d".into(),
                report: Box::new(serde_json::from_str(&report).expect("report")),
            },
            cx,
        );
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, cx| view.usage.read(cx).report().is_some()));
    let block = bounds(cx, "usage-block");
    let chart = bounds(cx, "usage-chart");
    inside("the chart", chart, "the Usage block", block);
    let table = bounds(cx, "usage-table-toggle");
    cx.simulate_click(table.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("usage-table").is_some(), "the table shows");
    assert!(cx.debug_bounds("usage-chart").is_none());
}

#[test]
fn the_receipt_undoes_its_turn_and_a_message_rewinds_to_before_it() {
    let mut app = app();
    let (view, cx) = window(&mut app);
    cx.simulate_resize(size(px(1280.), px(800.)));
    view.update(cx, |view, cx| {
        view.can_restore = true;
        let info: cedian_wire::ThreadInfo = serde_json::from_value(serde_json::json!({
            "id": "t-00000001", "title": "f", "task": "f", "state": "done", "result": null,
            "parent": null, "archived": false, "sessions": [], "messages": [], "pending": [],
            "updated_at": "2026-10-22T01:00:00.000Z", "status": "done",
            "runs": [{"id": "r-1", "prompt": "make f.txt say good"}],
            "last_turn": {"cwd": "/repo", "before": "t0", "after": "t1", "sessions": [], "at": "x"}
        }))
        .expect("info");
        view.projects[0].set_threads(vec![info]);
        view.show_offers(cx);
    });
    cx.run_until_parked();
    let undo = bounds(cx, "receipt-undo");
    let block = bounds(cx, "agent-block");
    inside("the undo button", undo, "the Agent block", block);
    // cedia left a file the person changed since: the card says so and offers to force it.
    view.update(cx, |view, cx| {
        let result =
            serde_json::from_str(r#"{"action":"undo","restored":["g.txt"],"skipped":["f.txt"]}"#)
                .expect("restored");
        view.on_notice(
            Notice::Restored {
                cwd: view.project().cwd.clone(),
                thread: "t-00000001".into(),
                result,
            },
            cx,
        );
    });
    cx.run_until_parked();
    inside(
        "the restore card",
        bounds(cx, "restore-card"),
        "the Agent block",
        block,
    );
    assert!(cx.debug_bounds("restore-force").is_some());
    // A whole undo turns the receipt's button into redo.
    view.update(cx, |view, cx| {
        let result = serde_json::from_str(r#"{"action":"undo","restored":["f.txt"],"skipped":[]}"#)
            .expect("restored");
        view.on_notice(
            Notice::Restored {
                cwd: view.project().cwd.clone(),
                thread: "t-00000001".into(),
                result,
            },
            cx,
        );
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("restore-force").is_none());
    assert_eq!(
        view.read_with(cx, |view, cx| view.agent.read(cx).offers().undone),
        Some(true)
    );
    // The message that started the run asks before it rewinds.
    view.update(cx, |view, cx| view.ask_rewind("make f.txt say good", cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("restore-go").is_some());
    let close = bounds(cx, "restore-close").center();
    cx.simulate_click(close, gpui::Modifiers::none());
    assert!(cx.debug_bounds("restore-card").is_none());
}

#[test]
fn the_skills_block_lists_the_commands_and_a_click_puts_one_in_the_composer() {
    let mut app = app();
    let (view, cx) = window(&mut app);
    view.update(cx, |view, cx| {
        view.open_kind(Kind::Skills, super::blocks::ToolTarget { against: None }, cx);
        let commands = serde_json::from_str(
            r#"[{"name":"review","description":"Review a diff against the repository's rules, and say what to change before it merges","kind":"skill","source":"repository","argumentHint":"<pr>"},
                {"name":"a-skill-with-a-name-long-enough-to-be-cut-in-a-narrow-block","description":"x","kind":"skill","source":"user"}]"#,
        )
        .expect("commands");
        view.on_notice(
            Notice::Commands {
                cwd: view.project().cwd.clone(),
                commands,
            },
            cx,
        );
    });
    for (width, height) in SIZES {
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();
        let block = bounds(cx, "skills-block");
        for row in ["skill-0", "skill-1"] {
            inside(row, bounds(cx, row), "the Skills block", block);
        }
    }
    let row = bounds(cx, "skill-0").center();
    cx.simulate_click(row, gpui::Modifiers::none());
    assert_eq!(
        view.read_with(cx, |view, cx| view.composer.read(cx).text(cx)),
        "/review "
    );
}
