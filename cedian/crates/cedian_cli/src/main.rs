//! `cedian` CLI harness (throwaway): the full agent loop without GPUI.
//!
//! Wires the headless crates end-to-end in one process:
//! `OmpRuntime` + `HostTools` + `Panel` + `ReviewTracker`. Commands mirror the
//! future palette entries (§2.5) so the wiring transfers to the app shell.
//!
//! Usage:
//! ```text
//! cedian prompt "fix the typo"        # one turn: prompt → stream → cards → review pending
//! cedian review                       # show pending hunks (baseline → current)
//! cedian accept <path> <hunk>         # accept one hunk (mark)
//! cedian reject <path> <hunk>         # reject one hunk (inverse patch)
//! cedian accept-all                   # bulk accept (skips unattributed)
//! cedian state                        # session snapshot (model, streaming, queue)
//! ```
//!
//! State lives in-process per invocation EXCEPT the OMP session (adopted via
//! `--session-dir` + `open_session`) and workspace files. Review baseline is
//! rebuilt from file mtimes per invocation (headless limitation — the app
//! shell will persist it; see plan §17 AgentEdit store design).

mod session;
mod workflow_store;
mod workspace_files;

use cedian_agent_ui::Panel;
use cedian_omp::{OmpBinary, OmpRuntime, RuntimeConfig};
use cedian_review::{Baseline, ReviewTracker};
use cedian_workspace::{HostTools, WorkspaceHost};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn main() {
    let result = run(std::env::args().skip(1).collect());
    if let Err(e) = result {
        eprintln!("cedian: {e}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let session_dir = std::env::var("CEDIAN_SESSION_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("cedian-cli-session"));
    // Canonicalize: `/tmp` → `/private/tmp` on macOS (else rootUri,
    // buffer keys, and didOpen URIs disagree and diagnostics never match).
    let workdir: PathBuf = std::env::var("CEDIAN_WORKDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let workdir = workdir.canonicalize().unwrap_or(workdir);

    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");
    // Settings gate: dangerous tier Deny refuses prompt (the shell rule —
    // headless reads the same file the future UI will edit).
    let settings = load_workdir_settings(&workdir);
    match cmd {
        "prompt" => {
            let message = args.get(1).ok_or("usage: cedian prompt <message>")?;
            if settings.permissions.dangerous == cedian_shell::Verdict::Deny {
                return Err("refused: settings [permissions] dangerous = deny".to_string());
            }
            cmd_prompt(&session_dir, &workdir, message)
        }
        "review" => cmd_review(&workdir),
        "accept" => {
            let path = args.get(1).ok_or("usage: cedian accept <path> <hunk>")?;
            let hunk: usize = args
                .get(2)
                .ok_or("usage: cedian accept <path> <hunk>")?
                .parse()
                .map_err(|_| "bad hunk index")?;
            cmd_accept(&workdir, Path::new(path), hunk)
        }
        "reject" => {
            let path = args.get(1).ok_or("usage: cedian reject <path> <hunk>")?;
            let hunk: usize = args
                .get(2)
                .ok_or("usage: cedian reject <path> <hunk>")?
                .parse()
                .map_err(|_| "bad hunk index")?;
            cmd_reject(&workdir, Path::new(path), hunk)
        }
        "accept-all" => cmd_accept_all(&workdir),
        "state" => cmd_state(&session_dir, &workdir),
        "palette" => {
            let query = args.get(1).map(|s| s.as_str()).unwrap_or("");
            for action in cedian_shell::Palette::filter(query) {
                println!("{} — {} ({})", action.id, action.title, action.hint);
            }
            Ok(())
        }
        "symbols" => {
            let query = args.get(1).ok_or("usage: cedian symbols <query>")?;
            cmd_symbols(&workdir, query)
        }
        "diagnostics" => cmd_diagnostics(&workdir),
        "workflow" => cmd_workflow(&workdir, &args[1..]),
        _ => {
            eprintln!("usage: cedian <prompt|review|accept|reject|accept-all|state|palette|symbols|diagnostics|workflow> …");
            eprintln!("env: CEDIAN_SESSION_DIR, CEDIAN_WORKDIR");
            Ok(())
        }
    }
}

/// Load workdir settings (`cedian.json` beside `.cedian/`), falling back to
/// defaults when absent. Invalid files fail the command (fail closed).
fn load_workdir_settings(workdir: &Path) -> cedian_shell::Settings {
    let path = workdir.join("cedian.json");
    match std::fs::read_to_string(&path) {
        Ok(raw) => cedian_shell::load_settings(&raw).unwrap_or_else(|e| {
            eprintln!("cedian: bad {path:?}: {e}");
            std::process::exit(2);
        }),
        Err(_) => cedian_shell::Settings::default(),
    }
}

/// Spawn the runtime with workspace host tools + cedian:// wired.
fn spawn(
    session_dir: &Path,
    workdir: &Path,
    host: &std::sync::Arc<HostTools>,
) -> Result<OmpRuntime, String> {
    let rt = OmpRuntime::spawn(RuntimeConfig {
        binary: OmpBinary::Path("omp".to_string()),
        session_dir: session_dir.to_path_buf(),
        cwd: workdir.to_path_buf(),
        ask_dialog: true,
        prompt_timeout: Duration::from_secs(600),
    })
    .map_err(|e| e.to_string())?;
    rt.set_host_tools(vec![host.apply_edit_tool()])
        .map_err(|e| e.to_string())?;
    rt.set_host_uris(vec![host.cedian_uri_scheme()])
        .map_err(|e| e.to_string())?;
    Ok(rt)
}

/// Load every text file under workdir into the host buffers (headless scan).
fn load_workspace(host: &HostTools, workdir: &Path) -> Vec<PathBuf> {
    workspace_files::scan_text_files(workdir)
        .into_iter()
        .filter_map(|path| {
            let key = workspace_files::buffer_key(workdir, &path)?;
            let text = std::fs::read_to_string(&path).ok()?;
            host.open(&key, &text);
            Some(key)
        })
        .collect()
}

fn cmd_prompt(session_dir: &Path, workdir: &Path, message: &str) -> Result<(), String> {
    let host = HostTools::shared(workdir);
    let keys = load_workspace(&host, workdir);

    // Baseline = current versions (review shows only THIS turn's edits;
    // cross-invocation baselines need the shell store — §2.5).
    let mut baseline = Baseline::new();
    let mut texts = HashMap::new();
    for key in &keys {
        if let Some(v) = host.buffer_version(key) {
            baseline.snapshot(key, v);
        }
        if let Some(t) = host.read_buffer(key) {
            texts.insert(key.clone(), t);
        }
    }

    let mut rt = spawn(session_dir, workdir, &host)?;
    rt.open_session("cli").map_err(|e| e.to_string())?;

    let mut panel = Panel::new();
    let task_id = panel.new_task("cli", workdir.to_path_buf());
    let router = rt.router();
    let (_sub, rx) = router.subscribe();

    // Pump router events into the panel on a thread while the turn runs.
    let pump = std::thread::spawn(move || {
        let mut panel = panel;
        for event in rx.iter().take(2000) {
            let done = matches!(event, cedian_omp::RouterEvent::Settled);
            panel.dispatch(&event);
            if done {
                break;
            }
        }
        panel
    });

    // Ambient context travels with the prompt (§39).
    let ambient =
        cedian_workspace::render_snapshot(&cedian_workspace::capture_ambient(host.as_ref()));
    let full = if ambient.is_empty() {
        message.to_string()
    } else {
        format!("{ambient}\n{message}")
    };
    let turn = rt.prompt(&full, vec![]).map_err(|e| e.to_string())?;
    let panel = pump.join().map_err(|_| "pump thread died".to_string())?;

    // Render the turn: assistant text + tool cards + review hunks.
    if let Some(text) = turn.assistant_text.as_deref() {
        println!("{text}");
    }
    let task = panel.get(&task_id).ok_or("task vanished")?;
    let (_messages, cards) = cedian_agent_ui::render_thread(task.thread().events());
    for card in &cards {
        println!("[{}] {}", card.status_glyph(), card.display_line());
    }

    // Persist review inputs for the next invocation (baseline + texts).
    session::save_review_inputs(workdir, &texts).map_err(|e| e.to_string())?;
    // Sync edited buffers back to disk (headless save: buffers are the source
    // of truth during the turn; Agent Sync marks them saved).
    let dirty = host.agent_sync();
    for key in keys {
        if let Some(local) = workspace_files::local_path(workdir, &key) {
            if let Some(text) = host.read_buffer(&key) {
                let _ = std::fs::write(&local, text);
            }
        }
    }
    if !dirty.is_empty() {
        eprintln!("(synced {} buffer(s) to disk)", dirty.len());
    }

    rt.shutdown().map_err(|e| e.to_string())?;
    let _ = panel;
    Ok(())
}

fn load_tracker(workdir: &Path, host: &HostTools) -> Result<ReviewTracker, String> {
    let keys = load_workspace(host, workdir);
    let (baseline, texts) =
        session::load_review_inputs(workdir, &keys, host).map_err(|e| e.to_string())?;
    let mut tracker = ReviewTracker::new("cli", baseline, texts);
    tracker.request_rebuild();
    std::thread::sleep(Duration::from_millis(60));
    tracker.rebuild_due(host as &dyn WorkspaceHost);
    Ok(tracker)
}

fn cmd_review(workdir: &Path) -> Result<(), String> {
    let host = HostTools::new(workdir);
    let tracker = load_tracker(workdir, &host)?;
    let mut any = false;
    for path in tracker.paths() {
        let diff = tracker.diff(&path).map_err(|e| e.to_string())?;
        if diff.is_empty() {
            continue;
        }
        any = true;
        println!("{} ({} hunk(s))", diff.path, diff.len());
        for (i, hunk) in diff.hunks.iter().enumerate() {
            println!(
                "  [{}] hunk {i}: -{}+{} → +{}+{} [{}]",
                i,
                hunk.before_start + 1,
                hunk.before_count,
                hunk.after_start + 1,
                hunk.after_count,
                status_glyph(diff.statuses[i])
            );
        }
    }
    if !any {
        println!("no pending changes (baseline == current)");
    }
    Ok(())
}

fn cmd_accept(workdir: &Path, path: &Path, hunk: usize) -> Result<(), String> {
    let host = HostTools::new(workdir);
    let mut tracker = load_tracker(workdir, &host)?;
    tracker.accept_hunk(path, hunk).map_err(|e| e.to_string())?;
    println!(
        "accepted {} hunk {hunk} (marked; code already in buffer)",
        path.display()
    );
    Ok(())
}

fn cmd_reject(workdir: &Path, path: &Path, hunk: usize) -> Result<(), String> {
    let host = HostTools::new(workdir);
    let mut tracker = load_tracker(workdir, &host)?;
    let v = tracker
        .reject_hunk(path, hunk, &host as &dyn WorkspaceHost)
        .map_err(|e| e.to_string())?;
    // Sync the restored buffer back to disk.
    if let Some(local) = workspace_files::local_path(workdir, path) {
        if let Some(text) = host.read_buffer(path) {
            std::fs::write(&local, text).map_err(|e| e.to_string())?;
        }
    }
    println!(
        "rejected {} hunk {hunk} (inverse patch → v{})",
        path.display(),
        v.0
    );
    Ok(())
}

fn cmd_accept_all(workdir: &Path) -> Result<(), String> {
    let host = HostTools::new(workdir);
    let mut tracker = load_tracker(workdir, &host)?;
    let accepted = tracker.accept_all();
    println!("accepted {} hunk(s) (unattributed skipped)", accepted.len());
    for (path, i) in accepted {
        println!("  {} hunk {i}", path.display());
    }
    Ok(())
}

fn cmd_state(session_dir: &Path, workdir: &Path) -> Result<(), String> {
    let host = HostTools::shared(workdir);
    let rt = spawn(session_dir, workdir, &host)?;
    let state = rt.get_state().map_err(|e| e.to_string())?;
    let (model, thinking) = cedian_agent::state::model_from_state(&state);
    println!("model: {}/{}", model.provider, model.id);
    println!("thinking: {:?}", thinking);
    println!("streaming: {}", state.is_streaming);
    println!("settled: {}", state.is_settled);
    println!("messages: {}", state.message_count);
    rt.shutdown().map_err(|e| e.to_string())?;
    Ok(())
}

/// Spawn rust-analyzer for the workdir, attach to a host, sync open buffers.
/// Returns (host, bridge) — bridge owns the server child.
fn spawn_lsp(
    workdir: &Path,
) -> Result<(std::sync::Arc<HostTools>, cedian_workspace::LspBridge), String> {
    let host = HostTools::shared(workdir);
    for path in crate::workspace_files::scan_text_files(workdir) {
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let key = crate::workspace_files::buffer_key(workdir, &path)
                    .ok_or_else(|| "path escapes workspace".to_string())?;
                host.open(&key, &text);
            }
        }
    }
    let bridge = cedian_workspace::LspBridge::spawn("rust-analyzer", workdir, &host)
        .map_err(|e| e.to_string())?;
    // Sync every open buffer into the server.
    for key in host.open_keys() {
        if let Some(text) = host.read_buffer(&key) {
            let local = workdir.join(key.strip_prefix("/").unwrap_or(&key));
            bridge.sync_buffer(&local, &text);
        }
    }
    Ok((host, bridge))
}

fn cmd_symbols(workdir: &Path, query: &str) -> Result<(), String> {
    let (_host, bridge) = spawn_lsp(workdir)?;
    // Workspace index builds async — retry empty answers until the deadline.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let symbols = bridge.workspace_symbols(query).map_err(|e| e.to_string())?;
        if !symbols.is_empty() || std::time::Instant::now() >= deadline {
            if symbols.is_empty() {
                println!("no symbols for {query:?}");
            }
            for line in cedian_workspace::lsp_bridge::render_workspace_symbols(&symbols).lines() {
                println!("{line}");
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

fn cmd_diagnostics(workdir: &Path) -> Result<(), String> {
    let (host, _bridge) = spawn_lsp(workdir)?;
    // Wait for the first publishDiagnostics wave. Cold rust-analyzer init
    // (cargo metadata + first check) can take 5+ min on first launch of the
    // day; warm restarts answer in seconds. The bridge caches nothing — every
    // CLI invocation pays cold start once (S9 keeps one warm server).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
    loop {
        let mut any = false;
        for key in host.open_keys() {
            if !host.diagnostics(&key).is_empty() {
                any = true;
                break;
            }
        }
        if any || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
    let mut shown = false;
    for key in host.open_keys() {
        for d in host.diagnostics(&key) {
            shown = true;
            let sev = match d.severity {
                cedian_workspace::DiagnosticSeverity::Error => "error",
                cedian_workspace::DiagnosticSeverity::Warning => "warning",
                cedian_workspace::DiagnosticSeverity::Info => "info",
            };
            println!("{sev} {}:{} {}", d.path.display(), d.line, d.message);
        }
    }
    if !shown {
        println!("no diagnostics (clean)");
    }
    Ok(())
}

/// Workflow commands (S2 headless surface):
/// ```text
/// cedian workflow run <kind> <title> [--risk low|medium|high]  # start (overwrites)
/// cedian workflow status                                       # §51 render + gates
/// cedian workflow evidence <gate> <summary> [--fail] [--unattributed]
/// cedian workflow advance [--fail]                             # pass/fail current phase
/// cedian workflow complete                                     # §55 completion gate
/// ```
fn cmd_workflow(workdir: &Path, args: &[String]) -> Result<(), String> {
    match args.first().map(|s| s.as_str()) {
        Some("run") => {
            let kind = args
                .get(1)
                .ok_or("usage: cedian workflow run <kind> <title> [--risk R]")?;
            let title = args
                .get(2)
                .ok_or("usage: cedian workflow run <kind> <title> [--risk R]")?;
            let kind = match kind.as_str() {
                "investigation" => cedian_workflow::TaskKind::Investigation,
                "bug_fix" => cedian_workflow::TaskKind::BugFix,
                "feature" => cedian_workflow::TaskKind::Feature,
                "refactor" => cedian_workflow::TaskKind::Refactor,
                "performance" => cedian_workflow::TaskKind::Performance,
                "prototype" => cedian_workflow::TaskKind::Prototype,
                _ => return Err(format!("unknown kind {kind:?} (investigation|bug_fix|feature|refactor|performance|prototype)")),
            };
            let mut profile = cedian_workflow::TaskProfile::new(title, kind);
            let mut i = 3;
            while i < args.len() {
                match args[i].as_str() {
                    "--risk" => {
                        let r = args.get(i + 1).ok_or("usage: --risk low|medium|high")?;
                        profile.risk = match r.as_str() {
                            "low" => cedian_workflow::Risk::Low,
                            "medium" => cedian_workflow::Risk::Medium,
                            "high" => cedian_workflow::Risk::High,
                            _ => return Err(format!("unknown risk {r:?}")),
                        };
                        i += 2;
                    }
                    flag => return Err(format!("unknown flag {flag:?}")),
                }
            }
            let state =
                cedian_workflow::WorkflowState::start(profile).map_err(|e| e.to_string())?;
            workflow_store::save(workdir, &state)?;
            render_workflow(&state);
            Ok(())
        }
        Some("status") => {
            let state = workflow_store::load(workdir)?;
            render_workflow(&state);
            Ok(())
        }
        Some("evidence") => {
            let gate = args.get(1).ok_or(
                "usage: cedian workflow evidence <gate> <summary> [--fail] [--unattributed]",
            )?;
            let summary = args.get(2).ok_or(
                "usage: cedian workflow evidence <gate> <summary> [--fail] [--unattributed]",
            )?;
            let mut ok = true;
            let mut attributed = true;
            for flag in &args[3..] {
                match flag.as_str() {
                    "--fail" => ok = false,
                    "--unattributed" => attributed = false,
                    _ => return Err(format!("unknown flag {flag:?}")),
                }
            }
            let mut state = workflow_store::load(workdir)?;
            // Attributed items link the CLI turn as the producing tool call
            // (headless stand-in for the §17 AgentEdit link).
            let id = format!("e{}", state.evidence.len() + 1);
            let item = if attributed {
                cedian_workflow::Evidence::attributed(
                    &id,
                    cedian_workflow::EvidenceKind::Command,
                    &[gate],
                    summary,
                    ok,
                    "cli",
                    format!("cli-turn-{}", state.evidence.len() + 1),
                )
            } else {
                cedian_workflow::Evidence::unattributed(
                    &id,
                    cedian_workflow::EvidenceKind::File,
                    &[gate],
                    summary,
                    ok,
                )
            };
            state.attach(item).map_err(|e| e.to_string())?;
            workflow_store::save(workdir, &state)?;
            match state.gate_result(gate) {
                Ok(r) => println!(
                    "evidence {id} → gate {gate:?}: {:?} ({})",
                    r.status, r.reason
                ),
                Err(e) => println!("evidence {id} attached ({}).", e),
            }
            Ok(())
        }
        Some("advance") => {
            let passed = !args[1..].contains(&"--fail".to_string());
            let mut state = workflow_store::load(workdir)?;
            state.advance(passed).map_err(|e| e.to_string())?;
            if !passed {
                println!("phase failed — workflow failed");
            }
            workflow_store::save(workdir, &state)?;
            render_workflow(&state);
            Ok(())
        }
        Some("complete") => {
            let mut state = workflow_store::load(workdir)?;
            match state.complete() {
                Ok(()) => {
                    workflow_store::save(workdir, &state)?;
                    println!("complete");
                    Ok(())
                }
                Err(missing) => {
                    workflow_store::save(workdir, &state)?;
                    Err(format!("blocked:\n  - {}", missing.join("\n  - ")))
                }
            }
        }
        _ => Err("usage: cedian workflow <run|status|evidence|advance|complete> …".to_string()),
    }
}

/// §51 render: title, kind · risk, phase checklist, gate states.
fn render_workflow(state: &cedian_workflow::WorkflowState) {
    let kind = match state.task.kind {
        cedian_workflow::TaskKind::Investigation => "Investigation",
        cedian_workflow::TaskKind::BugFix => "Bug Fix",
        cedian_workflow::TaskKind::Feature => "Feature",
        cedian_workflow::TaskKind::Refactor => "Refactor",
        cedian_workflow::TaskKind::Performance => "Performance",
        cedian_workflow::TaskKind::Prototype => "Prototype",
    };
    let risk = match state.task.risk {
        cedian_workflow::Risk::Low => "Low",
        cedian_workflow::Risk::Medium => "Medium",
        cedian_workflow::Risk::High => "High",
    };
    println!("{}", state.task.title);
    println!("{kind} · {risk} risk · {:?}", state.status);
    println!();
    for ps in &state.phases {
        let glyph = match ps.status {
            cedian_workflow::PhaseStatus::Pending => "○",
            cedian_workflow::PhaseStatus::Running => "●",
            cedian_workflow::PhaseStatus::Passed => "✓",
            cedian_workflow::PhaseStatus::Failed => "✗",
            cedian_workflow::PhaseStatus::Blocked => "!",
            cedian_workflow::PhaseStatus::Skipped => "–",
        };
        if ps.status == cedian_workflow::PhaseStatus::Skipped {
            let reason = ps.skip_reason.as_deref().unwrap_or("conditional");
            println!("{glyph} {} (skipped: {reason})", ps.id);
            continue;
        }
        println!("{glyph} {}", ps.id);
    }
    println!();
    for (id, r) in state.all_gates() {
        let glyph = match r.status {
            cedian_workflow::GateStatus::Pending => "○",
            cedian_workflow::GateStatus::Passed => "✓",
            cedian_workflow::GateStatus::Failed => "✗",
            cedian_workflow::GateStatus::Blocked => "!",
            cedian_workflow::GateStatus::Skipped => "–",
        };
        let unverified = if r.unverified_origin {
            " [unverified-origin]"
        } else {
            ""
        };
        println!(
            "{glyph} gate {id}: {:?}{unverified} — {}",
            r.status, r.reason
        );
    }
}

fn status_glyph(s: cedian_review::HunkStatus) -> &'static str {
    match s {
        cedian_review::HunkStatus::Pending => "pending",
        cedian_review::HunkStatus::Interrupted => "interrupted",
        cedian_review::HunkStatus::Unattributed => "unattributed",
        cedian_review::HunkStatus::Stale => "stale",
        cedian_review::HunkStatus::Accepted => "accepted",
        cedian_review::HunkStatus::Rejected => "rejected",
    }
}

/// Card status glyph for the turn render.
trait CardGlyph {
    fn status_glyph(&self) -> &'static str;
}

impl CardGlyph for cedian_agent_ui::ToolCard {
    fn status_glyph(&self) -> &'static str {
        match self.status {
            cedian_agent_ui::ToolCardStatus::Running => "…",
            cedian_agent_ui::ToolCardStatus::Done => "✓",
            cedian_agent_ui::ToolCardStatus::Error => "✗",
            cedian_agent_ui::ToolCardStatus::Interrupted => "!",
        }
    }
}

/// Re-export for tests.
use cedian_workspace::Version as _Version;
#[allow(unused)]
fn _keep_version(_: _Version) {}
