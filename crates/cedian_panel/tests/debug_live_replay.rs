//! S1's exit (ARCHITECTURE §22) on a live recording: `debug_live.jsonl`
//! is one model turn on the pinned OMP 18.8.6 (cursor/default, 2026-10-15,
//! `cedian_debug_live/tests/live_turn.rs`), asked to debug a C program
//! without being told which tool to use. OMP picked `cedian_debug` for all
//! 6 debugger calls. Here fake-omp replays it hermetically through the real
//! panel beside Zed's Debug panel, and Zed's fake debug adapter answers in
//! place of the real `lldb-dap`. Every dialog is approved, as it was live.
//! Checked:
//!
//! 1. the agent's breakpoint `main.c:4` is in Zed's `BreakpointStore` and
//!    reaches each session's SetBreakpoints;
//! 2. `start main` asks cedian's own dialog, launches the scenario, and its
//!    reply is the stop at `main.c:4` with `x = 42`; `continue` runs it to
//!    its exit; twice, as the model did;
//! 3. the workflow's debugger evidence is accepted, `cedian_complete`
//!    completes it, and the turn ends;
//! 4. the gate rows for the 6 calls in the audit log.
//!
//! fake-omp checks each host reply's text whole against the recording and
//! exits on any difference, so the replies cedian builds here are the ones
//! it built live. The ids in them came from the real lldb-dap (thread
//! 4916584 then 4916845, frame 524288); the fake adapter answers with those
//! same ids, as debug_round_trip's adapter answers with its own.
//!
//! Harness off: invoked with `--mode` (or `config`) this binary is fake-omp.

// Zed's fake adapter handlers return `dap::ErrorResponse`, as its own tests do.
#![expect(clippy::result_large_err)]

use cedian_panel::{CedianPanel, Connection, Turn};
use dap::client::DebugAdapterClient;
use debugger_ui::debugger_panel::DebugPanel;
use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use project::Project;
use project::debugger::session::Session;
use settings::SettingsStore;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use workspace::Workspace;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/debug_live.jsonl"
);
/// The recorded prompt, byte for byte (fake-omp checks it).
const PROMPT: &str = "This workspace is hosted by the cedian IDE, and the person is watching \
its debugger. main.c is built with debug info as ./main, and the IDE has a debug.json scenario \
labelled 'main' that launches it. Find the value of x at the printf line of main.c by debugging \
the program in the IDE's debugger, then let it run to its end. Report the value.";
const MAIN_C: &str = "#include <stdio.h>\n\
int main(void) {\n\
    int x = 42;\n\
    printf(\"%d\\n\", x);\n\
    return 0;\n\
}\n";
const DEBUG_JSON: &str =
    r#"[{"label": "main", "adapter": "fake-adapter", "request": "launch", "program": "main"}]"#;
/// The `printf` line.
const LINE: u64 = 4;
/// The value of `x` there, as lldb-dap gave it.
const X: &str = "42";
/// lldb-dap's thread id in each of the two sessions, and the top frame's id.
const THREADS: [i64; 2] = [4916584, 4916845];
const FRAME: u64 = 524288;
/// OMP's approval selects in the recording (three `bash` calls).
const OMP_DIALOGS: usize = 3;
/// cedian's own Asks: the two `start main`.
const CEDIAN_DIALOGS: usize = 2;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    cedian_fake_omp::test_main::enter(&args);
    let root = std::env::temp_dir().join(format!("cedian-s1-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("ws/.zed")).unwrap();
    std::fs::write(root.join("ws/main.c"), MAIN_C).unwrap();
    std::fs::write(root.join("ws/.zed/debug.json"), DEBUG_JSON).unwrap();
    std::fs::write(root.join("cedian.toml"), "schema = 1\n").unwrap();
    let root = root.canonicalize().unwrap();
    // SAFETY: single-threaded here; nothing else reads the environment yet.
    unsafe {
        std::env::set_var("CEDIAN_CONFIG", root.join("cedian.toml"));
        std::env::set_var("CEDIAN_STATE_DIR", root.join("state"));
        std::env::set_var("HOME", root.join("home"));
        std::env::set_var("CEDIAN_OMP_BINARY", std::env::current_exe().unwrap());
    }
    print!("test live_debug_turn_replays_through_zeds_debugger ... ");
    gpui::run_test_once(
        0,
        Box::new(move |dispatcher| {
            let exec = std::sync::Arc::new(dispatcher.clone());
            let mut cx = TestAppContext::build(dispatcher.clone(), Some("debug_live_replay"));
            gpui::ForegroundExecutor::new(exec).block_test(scenario(&mut cx, &root));
            cx.run_until_parked();
            cx.update(|cx| cx.quit());
            cx.run_until_parked();
            dispatcher.drain_tasks();
            let _ = std::fs::remove_dir_all(&root);
        }),
    );
    println!("ok");
}

/// What the fake adapters saw, by session order.
#[derive(Default)]
struct Seen {
    /// Lines the last SetBreakpoints for `main.c` carried.
    breakpoints: [parking_lot::Mutex<Vec<u64>>; 2],
    continues: [AtomicUsize; 2],
    sessions: AtomicUsize,
}

async fn scenario(cx: &mut TestAppContext, root: &Path) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        let store = SettingsStore::test(cx);
        cx.set_global(store);
        theme_settings::init(theme::LoadThemes::JustBase, cx);
        editor::init(cx);
        command_palette_hooks::init(cx);
        terminal_view::init(cx);
        debugger_ui::init(cx);
        dap_adapters::init(cx);
    });
    let ws = root.join("ws");
    let main_c = ws.join("main.c");
    let state = cedian_shell::state::dir(&ws).unwrap();
    cedian_fake_omp::install_replay(&state.join("omp"), &checked(root)).unwrap();

    let seen = Arc::new(Seen::default());
    let _adapters = project::debugger::test::intercept_debug_sessions(cx, {
        let seen = seen.clone();
        let main_c = main_c.clone();
        move |client| fake_adapter(client, &seen, &main_c)
    });

    let project = Project::test(fs::RealFs::new(None, cx.executor()), [ws.as_path()], cx).await;
    let window = cx.add_window(|window, cx| Workspace::test_new(project.clone(), window, cx));
    let debug_panel = window
        .update(cx, |_, window, cx| {
            cx.spawn_in(window, async move |this, cx| {
                DebugPanel::load(this, cx).await
            })
        })
        .unwrap()
        .await
        .unwrap();
    let panel = window
        .update(cx, |workspace, window, cx| {
            workspace.add_panel(debug_panel, window, cx);
            let handle = cx.entity().downgrade();
            let panel = cx.new(|cx| {
                let mut panel = CedianPanel::new(project.clone(), window, cx);
                panel.set_workspace(handle, cx);
                panel
            });
            workspace.add_panel(panel.clone(), window, cx);
            workspace.open_panel::<CedianPanel>(window, cx);
            panel
        })
        .unwrap();
    let mut vcx = VisualTestContext::from_window(window.into(), cx);
    wait(cx, &panel, "cedian agent ready", |p| {
        matches!(p.connection(), Connection::Ready { .. })
    });
    cx.update_window(window.into(), |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.set_prompt(PROMPT, window, cx);
            panel.submit(window, cx);
        })
    })
    .unwrap();

    let mut omp = 0;
    let mut asked = 0;
    let mut stopped = [false; 2];
    let mut ended = [false; 2];
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        cx.run_until_parked();
        let (idle, dialogs) = panel.read_with(cx, |p, _| (p.turn() == &Turn::Idle, p.dialog_ids()));
        if idle && omp == OMP_DIALOGS && asked == CEDIAN_DIALOGS && ended == [true; 2] {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "stuck after {omp} OMP dialogs, {asked} cedian dialogs, stops {stopped:?}, exits \
             {ended:?}: {}",
            panel.read_with(cx, |p, _| format!(
                "{:?} · {:?}",
                p.connection(),
                p.notice()
            ))
        );

        // lldb-dap stops each launch at the agent's breakpoint, and the
        // program runs to its end after the agent's continue.
        // Indexed by session id: an ended session leaves the store.
        let mut acted = false;
        for session in sessions(&project, cx) {
            let i = session
                .read_with(cx, |s, _| s.session_id().0 as usize)
                .min(1);
            let session = &session;
            if !stopped[i] && seen.breakpoints[i].lock().contains(&LINE) {
                stop(session, THREADS[i], cx).await;
                stopped[i] = true;
                acted = true;
            } else if stopped[i] && !ended[i] && seen.continues[i].load(Ordering::SeqCst) == 1 {
                exit(session, cx).await;
                ended[i] = true;
                acted = true;
            }
        }
        if acted {
            continue;
        }

        let Some(id) = dialogs.first().cloned() else {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        };
        if id.starts_with("cedian-debug-") {
            asked += 1;
            assert_eq!(
                seen.sessions.load(Ordering::SeqCst),
                asked - 1,
                "no adapter starts before the person approves `start`"
            );
        } else {
            omp += 1;
        }
        let button: &'static str = format!("cedian-dialog-{id}-Approve").leak();
        click(&mut vcx, button);
    }

    let (connection, notice) = panel.read_with(cx, |p, _| {
        (p.connection().clone(), p.notice().map(str::to_string))
    });
    assert!(
        matches!(connection, Connection::Ready { .. }) && notice.is_none(),
        "fake-omp replayed to the end (no divergence): {connection:?} · {notice:?}"
    );

    // 1. The breakpoint is the editor's, and each launch got it.
    let store = project.read_with(cx, |p, _| p.breakpoint_store());
    let rows: Vec<u32> = store.read_with(cx, |s, cx| {
        s.all_source_breakpoints(cx)
            .get(main_c.as_path())
            .map(|bps| bps.iter().map(|b| b.row).collect())
            .unwrap_or_default()
    });
    assert_eq!(
        rows,
        [LINE as u32 - 1],
        "main.c:4 is in Zed's breakpoint store"
    );
    for (i, lines) in seen.breakpoints.iter().enumerate() {
        assert_eq!(*lines.lock(), [LINE], "session {i}'s SetBreakpoints");
    }
    // 2. Two launches, each stopped, continued once, and exited.
    assert_eq!(seen.sessions.load(Ordering::SeqCst), 2, "two launches");
    assert_eq!(
        seen.continues.each_ref().map(|c| c.load(Ordering::SeqCst)),
        [1, 1],
        "one continue per session"
    );
    for session in sessions(&project, cx) {
        assert!(
            session.read_with(cx, |s, _| s.is_terminated()),
            "ran to its end"
        );
    }

    // 3. The workflow took the debugger evidence and completed.
    let workflow: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(state.join("workflow.json")).expect("workflow.json"),
    )
    .unwrap();
    assert_eq!(
        workflow["status"], "complete",
        "cedian_complete completed it"
    );
    let evidence = workflow.to_string();
    assert!(
        evidence.contains("\"debugger\"") && evidence.contains("x = 42"),
        "the debugger evidence is stored: {evidence}"
    );

    // 4. One gate row per cedian_debug call: set_breakpoint and continue
    // allowed by cedian, start approved by the person; twice.
    let audit = std::fs::read_to_string(state.join(cedian_shell::audit::AUDIT_FILE)).unwrap();
    let gate: Vec<(String, String)> = audit
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|row| row["item"].clone())
        .filter(|row| {
            row["kind"] == "gate"
                && row["command"]
                    .as_str()
                    .is_some_and(|c| c.starts_with("cedian_debug "))
        })
        .map(|row| {
            (
                row["decision"].as_str().unwrap_or_default().to_string(),
                row["answered_by"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    let want = [
        ("allow", "cedian"),
        ("allow", "user"),
        ("allow", "cedian"),
        ("allow", "cedian"),
        ("allow", "user"),
        ("allow", "cedian"),
    ]
    .map(|(d, a)| (d.to_string(), a.to_string()));
    assert_eq!(gate, want, "cedian's gate rows in the audit log");
}

/// The recording as recorded, with every host reply opted into fake-omp's
/// whole-text check (`checkResult`) and the host tools into `checkTools`.
/// A recording opts into nothing, and the fixture stays byte for byte what
/// the live run wrote.
fn checked(root: &Path) -> PathBuf {
    let lines: Vec<String> = std::fs::read_to_string(FIXTURE)
        .unwrap()
        .lines()
        .map(|line| {
            let mut v: serde_json::Value = serde_json::from_str(line).unwrap();
            match (v["dir"].as_str(), v["frame"]["type"].as_str()) {
                (Some("in"), Some("host_tool_result")) => v["checkResult"] = true.into(),
                (Some("in"), Some("set_host_tools")) => v["checkTools"] = true.into(),
                _ => {}
            }
            v.to_string()
        })
        .collect();
    let path = root.join("debug_live.checked.jsonl");
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    path
}

/// Zed's fake adapter, answering as lldb-dap did: `main` stopped at
/// `main.c:4` with `x = 42`, under lldb-dap's own ids.
fn fake_adapter(client: &Arc<DebugAdapterClient>, seen: &Arc<Seen>, main_c: &Path) {
    let order = seen.sessions.fetch_add(1, Ordering::SeqCst).min(1);
    let thread = THREADS[order];
    let main_c: PathBuf = main_c.to_path_buf();
    client.on_request::<dap::requests::Initialize, _>(|_, _| Ok(dap::Capabilities::default()));
    client.on_request::<dap::requests::SetBreakpoints, _>({
        let seen = seen.clone();
        let main_c = main_c.clone();
        move |_, args| {
            if args.source.path.as_deref() == Some(main_c.to_str().unwrap()) {
                *seen.breakpoints[order].lock() = args
                    .breakpoints
                    .unwrap_or_default()
                    .iter()
                    .map(|b| b.line)
                    .collect();
            }
            Ok(dap::SetBreakpointsResponse {
                breakpoints: Vec::new(),
            })
        }
    });
    client.on_request::<dap::requests::Threads, _>(move |_, _| {
        Ok(dap::ThreadsResponse {
            threads: vec![dap::Thread {
                id: thread,
                name: "main".into(),
            }],
        })
    });
    client.on_request::<dap::requests::Continue, _>({
        let seen = seen.clone();
        move |_, _| {
            seen.continues[order].fetch_add(1, Ordering::SeqCst);
            Ok(dap::ContinueResponse {
                all_threads_continued: Some(true),
            })
        }
    });
    client.on_request::<dap::requests::Disconnect, _>(|_, _| Ok(()));
    client.on_request::<dap::requests::StackTrace, _>(move |_, _| {
        Ok(dap::StackTraceResponse {
            stack_frames: vec![dap::StackFrame {
                id: FRAME,
                name: "main".into(),
                source: Some(dap::Source {
                    name: Some("main.c".into()),
                    path: Some(main_c.to_string_lossy().into_owned()),
                    source_reference: None,
                    presentation_hint: None,
                    origin: None,
                    sources: None,
                    adapter_data: None,
                    checksums: None,
                }),
                line: LINE,
                column: 1,
                end_line: None,
                end_column: None,
                can_restart: None,
                instruction_pointer_reference: None,
                module_id: None,
                presentation_hint: None,
            }],
            total_frames: None,
        })
    });
    client.on_request::<dap::requests::Scopes, _>(|_, _| {
        Ok(dap::ScopesResponse {
            scopes: vec![dap::Scope {
                name: "Locals".into(),
                presentation_hint: None,
                variables_reference: 2,
                named_variables: None,
                indexed_variables: None,
                expensive: false,
                source: None,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
            }],
        })
    });
    client.on_request::<dap::requests::Variables, _>(|_, _| {
        Ok(dap::VariablesResponse {
            variables: vec![dap::Variable {
                name: "x".into(),
                value: X.into(),
                type_: Some("int".into()),
                presentation_hint: None,
                evaluate_name: None,
                variables_reference: 0,
                named_variables: None,
                indexed_variables: None,
                memory_reference: None,
                declaration_location_reference: None,
                value_location_reference: None,
            }],
        })
    });
}

async fn stop(session: &Entity<Session>, thread: i64, cx: &mut TestAppContext) {
    let client = session.read_with(cx, |s, _| s.adapter_client().unwrap());
    client
        .fake_event(dap::messages::Events::Stopped(dap::StoppedEvent {
            reason: dap::StoppedEventReason::Breakpoint,
            description: None,
            thread_id: Some(thread),
            preserve_focus_hint: None,
            text: None,
            all_threads_stopped: Some(true),
            hit_breakpoint_ids: None,
        }))
        .await;
}

/// The program returns 0 and the adapter ends the session.
async fn exit(session: &Entity<Session>, cx: &mut TestAppContext) {
    let client = session.read_with(cx, |s, _| s.adapter_client().unwrap());
    client
        .fake_event(dap::messages::Events::Exited(dap::ExitedEvent {
            exit_code: 0,
        }))
        .await;
    client
        .fake_event(dap::messages::Events::Terminated(None))
        .await;
}

/// The project's debug sessions, oldest first.
fn sessions(project: &Entity<Project>, cx: &mut TestAppContext) -> Vec<Entity<Session>> {
    project.read_with(cx, |p, cx| {
        let mut sessions: Vec<_> = p.dap_store().read(cx).sessions().cloned().collect();
        sessions.sort_by_key(|s| s.read(cx).session_id().0);
        sessions
    })
}

fn click(vcx: &mut VisualTestContext, selector: &'static str) {
    vcx.run_until_parked();
    let bounds = vcx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not on screen"));
    vcx.simulate_click(bounds.center(), Modifiers::none());
}

/// Pump the test executor while real OS threads (OMP) make progress.
fn wait(
    cx: &mut TestAppContext,
    panel: &Entity<CedianPanel>,
    what: &str,
    done: impl Fn(&CedianPanel) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        let (finished, state) = panel.read_with(cx, |p, _| {
            (
                done(p),
                format!("{:?} · {:?} · {:?}", p.connection(), p.turn(), p.notice()),
            )
        });
        if finished {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {state}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
