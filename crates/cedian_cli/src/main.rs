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
//! cedian review reset                 # drop the review task (new baseline next prompt)
//! cedian accept <path> <hunk>         # accept one hunk (mark)
//! cedian reject <path> <hunk>         # reject one hunk (inverse patch)
//! cedian accept-all                   # bulk accept (skips unattributed)
//! cedian state                        # session snapshot (model, streaming, queue)
//! ```
//!
//! State lives in-process per invocation EXCEPT the OMP session (adopted via
//! `--session-dir` + `open_session`), workspace files, and the review task in
//! `.cedian/review.json` (baseline + AgentEdit records + resolutions, see
//! `session.rs`). `cedian review reset` starts a new review task.

mod browser_store;
mod session;
mod workflow_store;
mod workspace_files;

use cedian_agent_ui::Panel;
use cedian_omp::{ApprovalMode, OmpBinary, OmpRuntime, RuntimeConfig, SpawnPolicy, ToolPolicy};
use cedian_review::{AgentEdit, ReviewTracker};
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
        "review" => match args.get(1).map(|s| s.as_str()) {
            None => cmd_review(&workdir),
            Some("reset") => {
                session::reset(&workdir);
                println!("review task reset (next prompt takes a fresh baseline)");
                Ok(())
            }
            Some(other) => Err(format!("usage: cedian review [reset] (got {other:?})")),
        },
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
        "browser" => cmd_browser(&workdir, &args[1..]),
        "workflow" => cmd_workflow(&workdir, &args[1..]),
        "worker" => cmd_worker(&workdir, &args[1..]),
        _ => {
            eprintln!(
                "usage: cedian <prompt|review|accept|reject|accept-all|state|\
                palette|symbols|diagnostics|browser|workflow|worker> …"
            );
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

/// Map `[permissions]` onto the OMP spawn policy (ADR-0020). Only tightens:
/// `dangerous = allow` still leaves the exec floor at `prompt` (strict-wins,
/// ADR-0012), and yolo is unrepresentable.
fn spawn_policy(settings: &cedian_shell::Settings) -> SpawnPolicy {
    use cedian_shell::Verdict;
    let mut policy = SpawnPolicy::default();
    policy
        .host_tools
        .insert(cedian_workspace::APPLY_EDIT_TOOL.to_string());
    if settings.permissions.project_write != Verdict::Allow {
        policy.approval_mode = ApprovalMode::AlwaysAsk;
    }
    let mut deny = |tools: &[&str]| {
        for tool in tools {
            policy
                .tool_policies
                .insert((*tool).to_string(), ToolPolicy::Deny);
        }
    };
    if settings.permissions.project_write == Verdict::Deny {
        deny(&["edit", "write", "ast_edit"]);
    }
    if settings.permissions.dangerous == Verdict::Deny {
        deny(cedian_omp::spawn_profile::EXEC_TOOLS);
    }
    policy
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
        policy: spawn_policy(&load_workdir_settings(workdir)),
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

/// Tools whose successful completion may have changed files on disk.
const EDIT_TOOLS: &[&str] = &["edit", "write", "ast_edit", "cedian_apply_edit"];

fn cmd_prompt(session_dir: &Path, workdir: &Path, message: &str) -> Result<(), String> {
    let mut store = session::load(workdir)?.unwrap_or_default();
    let host = HostTools::shared(workdir);
    let keys = load_workspace(&host, workdir);

    // Pre-turn texts (disk == buffer right after load). The task baseline for
    // a file is its pre-turn text the FIRST time a turn changes it (§16), so
    // the store holds only files this task touched — user edits to other
    // files never enter review.
    let mut pre: HashMap<PathBuf, String> = HashMap::new();
    for key in &keys {
        if let Some(t) = host.read_buffer(key) {
            pre.insert(key.clone(), t);
        }
    }

    let mut rt = spawn(session_dir, workdir, &host)?;
    rt.open_session("cli").map_err(|e| e.to_string())?;

    let mut panel = Panel::new();
    let task_id = panel.new_task("cli", workdir.to_path_buf());
    let router = rt.router();
    let (sub, rx) = router.subscribe();

    // Pump router events into the panel on a thread while the turn runs.
    let pump = std::thread::spawn(move || {
        let mut panel = panel;
        for event in rx.iter() {
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
    let turn = rt.prompt(&full, vec![]);
    // Shutdown stops the reader; unsubscribing drops our sender so the pump
    // always finishes, even when the turn failed or no `Settled` arrived.
    let shutdown = rt.shutdown();
    router.unsubscribe(sub);
    drop(router);
    let panel = pump.join().map_err(|_| "pump thread died".to_string())?;
    let turn = turn.map_err(|e| e.to_string())?;
    shutdown.map_err(|e| e.to_string())?;

    // Render the turn: assistant text + tool cards.
    if let Some(text) = turn.assistant_text.as_deref() {
        println!("{text}");
    }
    let task = panel.get(&task_id).ok_or("task vanished")?;
    let (_messages, cards) = cedian_agent_ui::render_thread(task.thread().events());
    for card in &cards {
        println!("[{}] {}", card.status_glyph(), card.display_line());
    }

    // Write back ONLY buffers cedian itself changed (host-tool edits), and
    // never over a file that changed on disk during the turn: OMP's native
    // `edit`/`write` write the filesystem directly (plan §84 row G), so disk
    // is authoritative for them.
    let mut conflicts = Vec::new();
    let mut synced = 0;
    for key in &keys {
        let (Some(before), Some(buffer)) = (pre.get(key), host.read_buffer(key)) else {
            continue;
        };
        if &buffer == before {
            continue;
        }
        let Some(local) = workspace_files::local_path(workdir, key) else {
            continue;
        };
        let disk = std::fs::read_to_string(&local).unwrap_or_default();
        if &disk != before {
            conflicts.push(key.display().to_string());
            continue;
        }
        std::fs::write(&local, &buffer).map_err(|e| e.to_string())?;
        synced += 1;
    }
    if synced > 0 {
        eprintln!("(synced {synced} buffer(s) to disk)");
    }
    for c in &conflicts {
        eprintln!(
            "conflict: {c} changed on disk AND in the buffer during the turn — buffer NOT written"
        );
    }

    // Provenance (§17): snapshot every file the turn changed, read from disk
    // (new files included). Pin to a tool call when one edit-class card names
    // the file, or when the turn had exactly one; otherwise no record → the
    // hunks review as UNATTRIBUTED (never misattributed).
    let edit_cards: Vec<&cedian_agent_ui::ToolCard> = cards
        .iter()
        .filter(|c| {
            c.status == cedian_agent_ui::ToolCardStatus::Done
                && EDIT_TOOLS.contains(&c.name.as_str())
        })
        .collect();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut unattributed = 0;
    for path in workspace_files::scan_text_files(workdir) {
        let Some(key) = workspace_files::buffer_key(workdir, &path) else {
            continue;
        };
        let Ok(after) = std::fs::read_to_string(&path) else {
            continue;
        };
        let before = pre.get(&key).cloned().unwrap_or_default();
        if after == before {
            continue;
        }
        store.baseline_once(&key, &before); // first change in task (new file: empty)
        let rel = key.to_string_lossy().trim_start_matches('/').to_string();
        let pinned =
            edit_cards
                .iter()
                .find(|c| c.preview.contains(&rel))
                .or(if edit_cards.len() == 1 {
                    edit_cards.first()
                } else {
                    None
                });
        match pinned {
            Some(card) => store.record(AgentEdit {
                tool_call_id: card.call_id.clone(),
                task_id: session::CLI_TASK.to_string(),
                file: key.to_string_lossy().into_owned(),
                before,
                after,
                timestamp_ms: now_ms,
            }),
            None => unattributed += 1,
        }
    }
    if unattributed > 0 {
        eprintln!(
            "({unattributed} changed file(s) not pinned to a tool call → UNATTRIBUTED in review)"
        );
    }
    session::save(workdir, &store)
}

/// Load the review task + tracker, rebuilt against current disk state.
/// No task yet → empty tracker (review shows nothing).
fn load_tracker(
    workdir: &Path,
    host: &HostTools,
) -> Result<(session::ReviewStore, ReviewTracker), String> {
    load_workspace(host, workdir);
    let store = session::load(workdir)?.unwrap_or_default();
    let (baseline, texts) = store.tracker_inputs();
    // Files in the baseline that no longer exist on disk review as deletions.
    for key in texts.keys() {
        if host.read_buffer(key).is_none() {
            host.open(key, "");
        }
    }
    let mut tracker = ReviewTracker::new(&store.task_id, baseline, texts);
    tracker.attribute(&store.provenance);
    tracker.restore_statuses(store.statuses.clone());
    tracker.rebuild_now(host as &dyn WorkspaceHost);
    Ok((store, tracker))
}

/// Persist the tracker's resolutions back into the review task.
fn save_statuses(
    workdir: &Path,
    mut store: session::ReviewStore,
    tracker: &ReviewTracker,
) -> Result<(), String> {
    store.statuses = tracker.status_records();
    session::save(workdir, &store)
}

fn cmd_review(workdir: &Path) -> Result<(), String> {
    let host = HostTools::new(workdir);
    let (_store, tracker) = load_tracker(workdir, &host)?;
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
    let (store, mut tracker) = load_tracker(workdir, &host)?;
    tracker.accept_hunk(path, hunk).map_err(|e| e.to_string())?;
    save_statuses(workdir, store, &tracker)?;
    println!(
        "accepted {} hunk {hunk} (marked; code already in buffer)",
        path.display()
    );
    Ok(())
}

fn cmd_reject(workdir: &Path, path: &Path, hunk: usize) -> Result<(), String> {
    let host = HostTools::new(workdir);
    let (store, mut tracker) = load_tracker(workdir, &host)?;
    let v = tracker
        .reject_hunk(path, hunk, &host as &dyn WorkspaceHost)
        .map_err(|e| e.to_string())?;
    // Sync the restored buffer back to disk.
    if let Some(local) = workspace_files::local_path(workdir, path) {
        if let Some(text) = host.read_buffer(path) {
            std::fs::write(&local, text).map_err(|e| e.to_string())?;
        }
    }
    save_statuses(workdir, store, &tracker)?;
    println!(
        "rejected {} hunk {hunk} (inverse patch → v{})",
        path.display(),
        v.0
    );
    Ok(())
}

fn cmd_accept_all(workdir: &Path) -> Result<(), String> {
    let host = HostTools::new(workdir);
    let (store, mut tracker) = load_tracker(workdir, &host)?;
    let accepted = tracker.accept_all();
    save_statuses(workdir, store, &tracker)?;
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

/// Browser commands (S4 headless surface — one-shot per invocation):
/// ```text
/// cedian browser open <url>                             # fresh Chrome → navigate → save head
/// cedian browser dom                                    # fresh Chrome on saved url → title, url, html
/// cedian browser shot [--attach <gate>] [--note <text>] # PNG into .cedian/shots/ (+ gate evidence)
/// cedian browser close                                  # clear head + sweep chrome-* profiles
/// cedian browser status                                 # saved head or `no browser session`
/// ```
///
/// Every action command spawns its own fresh Chrome (see `browser_store`):
/// the child dies with the invocation, so `dom`/`shot` re-navigate to the
/// saved url instead of reconnecting.
fn cmd_browser(workdir: &Path, args: &[String]) -> Result<(), String> {
    // One-shot per invocation: each arm spawns its own fresh Chrome, does
    // exactly one action, and saves the head. The `_proc` binding keeps the
    // child alive through the action (drops at scope end).
    match args.first().map(|s| s.as_str()) {
        Some("open") => {
            let url = args
                .get(1)
                .ok_or("usage: cedian browser open <url>")?
                .to_string();
            let (proc, session, ws_url) = browser_store::spawn_fresh(workdir, &url)?;
            let seq = session.current_seq();
            browser_store::save(
                workdir,
                &browser_store::BrowserHead {
                    port: proc.port,
                    ws_url,
                    url: url.clone(),
                    seq,
                },
            )?;
            println!("opened {url} (seq {seq})");
            Ok(())
        }
        Some("dom") => {
            let head = browser_store::load(workdir)?;
            let (_proc, session, _) = browser_store::spawn_fresh(workdir, &head.url)?;
            let dom = session.dom(2000).map_err(|e| e.to_string())?;
            println!("{} {}", dom.title, dom.url);
            println!("{}", dom.html);
            Ok(())
        }
        Some("shot") => {
            let mut attach: Option<String> = None;
            let mut note: Option<String> = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--attach" => {
                        let gate = args.get(i + 1).ok_or(
                            "usage: cedian browser shot [--attach <gate>] [--note <text>]",
                        )?;
                        attach = Some(gate.to_string());
                        i += 2;
                    }
                    "--note" => {
                        let text = args.get(i + 1).ok_or(
                            "usage: cedian browser shot [--attach <gate>] [--note <text>]",
                        )?;
                        note = Some(text.to_string());
                        i += 2;
                    }
                    flag => return Err(format!("unknown flag {flag:?}")),
                }
            }
            let head = browser_store::load(workdir)?;
            let (proc, session, ws_url) = browser_store::spawn_fresh(workdir, &head.url)?;
            let shot = session
                .screenshot(&workdir.join(".cedian").join("shots"))
                .map_err(|e| e.to_string())?;
            browser_store::save(
                workdir,
                &browser_store::BrowserHead {
                    port: proc.port,
                    ws_url,
                    url: head.url,
                    seq: session.current_seq(),
                },
            )?;
            println!(
                "shot {} frame {} seq {}",
                shot.path.display(),
                shot.frame_id,
                shot.seq
            );
            if let Some(gate) = attach {
                let mut state = workflow_store::load(workdir)?;
                let n = state.evidence.len() + 1;
                let id = format!("e{n}");
                let mut summary = format!(
                    "shot {} frame {} seq {}",
                    shot.path.display(),
                    shot.frame_id,
                    shot.seq
                );
                if let Some(text) = note {
                    summary.push(' ');
                    summary.push_str(&text);
                }
                let item = cedian_workflow::Evidence::attributed(
                    &id,
                    cedian_workflow::EvidenceKind::Screenshot,
                    &[gate.as_str()],
                    summary,
                    true,
                    "cli",
                    format!("cli-browser-{n}"),
                );
                state.attach(item).map_err(|e| e.to_string())?;
                workflow_store::save(workdir, &state)?;
                match state.gate_result(&gate) {
                    Ok(r) => println!(
                        "evidence {id} → gate {gate:?}: {:?} ({})",
                        r.status, r.reason
                    ),
                    Err(e) => println!("evidence {id} attached ({e})."),
                }
            }
            Ok(())
        }
        Some("close") => {
            browser_store::clear(workdir);
            // Children died with their invocations; sweep leftover profiles.
            if let Ok(entries) = std::fs::read_dir(workdir.join(".cedian")) {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().starts_with("chrome-") {
                        let _ = std::fs::remove_dir_all(entry.path());
                    }
                }
            }
            println!("browser closed");
            Ok(())
        }
        Some("status") => match browser_store::load(workdir) {
            Ok(head) => {
                println!("{} (seq {})", head.url, head.seq);
                Ok(())
            }
            Err(_) => {
                println!("no browser session");
                Ok(())
            }
        },
        _ => Err("usage: cedian browser <open|dom|shot|close|status> …".to_string()),
    }
}

/// Read `--base <branch>` from `args[from..]`; defaults to `HEAD`.
fn worker_base(args: &[String], from: usize) -> Result<String, String> {
    let mut base = "HEAD".to_string();
    let mut i = from;
    while i < args.len() {
        match args[i].as_str() {
            "--base" => {
                base = args
                    .get(i + 1)
                    .cloned()
                    .ok_or("usage: cedian worker … [--base <branch>]")?;
                i += 2;
            }
            flag => return Err(format!("unknown flag {flag:?}")),
        }
    }
    Ok(base)
}

/// Worker commands (S5 headless surface — one-shot per invocation):
/// ```text
/// cedian worker spawn <id> <kind> <title> [--base <branch>]
/// cedian worker list
/// cedian worker steer <id> <note...>
/// cedian worker preview <id> [--base <branch>]
/// cedian worker merge-back <id> [--base <branch>]
/// cedian worker remove <id>
/// ```
///
/// `CEDIAN_WORKDIR` must be the repo root: the registry lives at
/// `<repo>/.cedian/workers.json`, worktrees at `<repo>/.worktrees/<id>`.
/// Base defaults to `HEAD` unless `--base <branch>` is given. `steer`
/// only records the note (status Running); the actual agent turn in the
/// worktree is a follow-up invocation.
fn cmd_worker(workdir: &Path, args: &[String]) -> Result<(), String> {
    match args.first().map(|s| s.as_str()) {
        Some("spawn") => {
            let usage = "usage: cedian worker spawn <id> <kind> <title> [--base B]";
            let id = args.get(1).ok_or(usage)?;
            let kind = args.get(2).ok_or(usage)?;
            let title = args.get(3).ok_or(usage)?;
            let base = worker_base(args, 4)?;
            let (mut reg, _) = cedian_worker::Registry::open(workdir);
            let mut head = cedian_worker::spawn(workdir, id, &base).map_err(|e| e.to_string())?;
            head.status = cedian_worker::WorkerStatus::Running;
            head.task_title = title.clone();
            head.kind = kind.clone();
            let (worktree, branch) = (head.worktree.clone(), head.branch.clone());
            reg.insert(head).map_err(|e| e.to_string())?;
            reg.save(workdir).map_err(|e| e.to_string())?;
            println!("worker {id} → {worktree} (branch {branch})");
            Ok(())
        }
        Some("list") => {
            if args.len() > 1 {
                return Err("usage: cedian worker list".to_string());
            }
            let (reg, _) = cedian_worker::Registry::open(workdir);
            let mut any = false;
            for head in reg.all() {
                any = true;
                let status = format!("{:?}", head.status).to_lowercase();
                println!(
                    "{} {} {} {}",
                    head.id, status, head.worktree, head.task_title
                );
            }
            if !any {
                println!("no workers");
            }
            Ok(())
        }
        Some("steer") => {
            let usage = "usage: cedian worker steer <id> <note...>";
            let id = args.get(1).ok_or(usage)?;
            if args.len() < 3 {
                return Err(usage.to_string());
            }
            let note = args[2..].join(" ");
            let (mut reg, _) = cedian_worker::Registry::open(workdir);
            let head = reg
                .get(id)
                .cloned()
                .ok_or_else(|| cedian_worker::WorkerError::NoSuch(id.clone()).to_string())?;
            reg.set_status(id, cedian_worker::WorkerStatus::Running, note.clone())
                .map_err(|e| e.to_string())?;
            reg.save(workdir).map_err(|e| e.to_string())?;
            let wt = workdir.join(&head.worktree);
            println!("steer {id}: {note}");
            println!(
                "hint: run the turn with CEDIAN_WORKDIR={} cedian prompt ...",
                wt.display()
            );
            Ok(())
        }
        Some("preview") => {
            let id = args
                .get(1)
                .ok_or("usage: cedian worker preview <id> [--base <branch>]")?;
            let base = worker_base(args, 2)?;
            let (reg, _) = cedian_worker::Registry::open(workdir);
            let head = reg
                .get(id)
                .cloned()
                .ok_or_else(|| cedian_worker::WorkerError::NoSuch(id.clone()).to_string())?;
            let plan =
                cedian_worker::merge_preview(workdir, &head, &base).map_err(|e| e.to_string())?;
            println!("clean:");
            for f in &plan.clean {
                println!("  {f}");
            }
            println!("conflicted(STALE):");
            for f in &plan.conflicted {
                println!("  {f}");
            }
            Ok(())
        }
        Some("merge-back") => {
            let id = args
                .get(1)
                .ok_or("usage: cedian worker merge-back <id> [--base <branch>]")?;
            let base = worker_base(args, 2)?;
            let (mut reg, _) = cedian_worker::Registry::open(workdir);
            let head = reg
                .get(id)
                .cloned()
                .ok_or_else(|| cedian_worker::WorkerError::NoSuch(id.clone()).to_string())?;
            match cedian_worker::merge_back(workdir, &head, &base) {
                Ok(()) => {
                    reg.set_status(id, cedian_worker::WorkerStatus::Done, String::new())
                        .map_err(|e| e.to_string())?;
                    reg.save(workdir).map_err(|e| e.to_string())?;
                    println!("merged {} → {base}", head.branch);
                    Ok(())
                }
                Err(cedian_worker::WorkerError::Conflicted(files)) => {
                    println!("refused (STALE):");
                    for f in &files {
                        println!("  {f}");
                    }
                    Err(format!("refused (STALE): {}", files.join(", ")))
                }
                Err(e) => Err(e.to_string()),
            }
        }
        Some("remove") => {
            let id = args.get(1).ok_or("usage: cedian worker remove <id>")?;
            if args.len() > 2 {
                return Err("usage: cedian worker remove <id>".to_string());
            }
            let (mut reg, _) = cedian_worker::Registry::open(workdir);
            let head = reg
                .get(id)
                .cloned()
                .ok_or_else(|| cedian_worker::WorkerError::NoSuch(id.clone()).to_string())?;
            cedian_worker::remove(workdir, &head).map_err(|e| e.to_string())?;
            reg.remove(id);
            reg.save(workdir).map_err(|e| e.to_string())?;
            println!("removed {id}");
            Ok(())
        }
        _ => Err("usage: cedian worker <spawn|list|steer|preview|merge-back|remove> …".into()),
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
