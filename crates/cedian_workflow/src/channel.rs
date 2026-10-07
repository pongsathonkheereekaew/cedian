//! Host-tool channel (P5, ADR-0022): OMP drives the workflow through
//! `cedian_workflow_update` and claims completion through `cedian_complete`.
//!
//! The engine stays pure: this module only adapts JSON args to
//! `WorkflowState` calls. Two seams keep it free of the runtime crates:
//! - [`WorkflowStore`]: every call loads, mutates and saves, so the store on
//!   disk stays the one truth even when CLI verbs run between turns.
//! - `current: Fn() -> CurrentState`: the workspace hashed now (ADR-0024).
//!   Evidence binds to the files its call named, else the whole tree; gates
//!   compare against a fresh `CurrentState` on every evaluation.
//! - the resolver `Fn(tool, needle) -> Option<BoundCall>`: the agent names
//!   the tool that produced the evidence (`from_tool`, optional `match` on
//!   its args); the caller binds that to the most recent call the router log
//!   saw finish successfully, excluding [`is_channel_call`]s (ADR-0031: the
//!   model never sees `tool_call_id`s). The id always comes from the log, so
//!   the agent cannot attribute evidence by saying so.

use crate::{
    state::ContinueOutcome, Claim, CompletionAttempt, CurrentState, Evidence, EvidenceKind,
    GateStatus, Outcome, Risk, TaskKind, TaskProfile, WorkflowState, WorkflowStatus,
};
use omp_rpc::HostTool;
use serde_json::{json, Map, Value};
use std::sync::{Arc, Mutex};

/// Host tool: start / evidence / advance.
pub const WORKFLOW_UPDATE_TOOL: &str = "cedian_workflow_update";
/// Host tool: `can_complete` check at the completion boundary.
pub const COMPLETE_TOOL: &str = "cedian_complete";
/// Every tool that reports TO cedian. Citing one as evidence would be
/// self-certification, so verifiers must reject them.
pub const CHANNEL_TOOLS: &[&str] = &[
    WORKFLOW_UPDATE_TOOL,
    COMPLETE_TOOL,
    "cedian_worktree_request",
];

/// True when a logged call is a channel report: called by name, or through
/// OMP's `xd://<tool>` device form (`read`/`write` with that preview).
pub fn is_channel_call(tool_name: &str, args_preview: &str) -> bool {
    CHANNEL_TOOLS
        .iter()
        .any(|t| tool_name == *t || args_preview.contains(&format!("xd://{t}")))
}

/// OMP tools that only observe. Any other finished call may have changed
/// files, so evidence bound to an earlier call is stale on arrival.
pub const READ_ONLY_TOOLS: &[&str] = &[
    "read", "grep", "glob", "find", "ast_grep", "lsp", "ask", "todo",
];

/// True when a logged call may have changed workspace files (conservative:
/// unknown tools, `bash` and `eval` count). Channel reports never do.
pub fn may_mutate(tool_name: &str, args_preview: &str) -> bool {
    !READ_ONLY_TOOLS.contains(&tool_name) && !is_channel_call(tool_name, args_preview)
}

/// Where the workflow lives between calls (`.cedian/workflow.json` in the CLI).
pub trait WorkflowStore: Send + Sync {
    /// `Ok(None)` when no workflow was ever started.
    fn load(&self) -> Result<Option<WorkflowState>, String>;
    fn save(&self, state: &WorkflowState) -> Result<(), String>;
}

/// A logged call evidence was bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCall {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args_preview: String,
    /// A call that may have changed files finished after this one (its name
    /// and args): evidence reported now would describe code the call never
    /// saw, so it is stored stale.
    pub mutated_after: Option<String>,
}

type Resolver = dyn Fn(&str, &str) -> Option<BoundCall> + Send + Sync;
type Current = dyn Fn() -> CurrentState + Send + Sync;

/// The two workflow host tools over one store + one resolver.
pub struct WorkflowChannel {
    task_id: String,
    store: Box<dyn WorkflowStore>,
    resolve: Box<Resolver>,
    current: Box<Current>,
    /// Serializes load → mutate → save across handler threads.
    lock: Mutex<()>,
}

impl WorkflowChannel {
    pub fn new(
        task_id: impl Into<String>,
        store: Box<dyn WorkflowStore>,
        resolve: impl Fn(&str, &str) -> Option<BoundCall> + Send + Sync + 'static,
        current: impl Fn() -> CurrentState + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            task_id: task_id.into(),
            store,
            resolve: Box::new(resolve),
            current: Box::new(current),
            lock: Mutex::new(()),
        })
    }

    /// `cedian_workflow_update`: `op` is `start`, `evidence` or `advance`.
    pub fn update(&self, args: &Map<String, Value>) -> Result<String, String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let op = str_arg(args, "op")?;
        match op {
            "start" => {
                if let Some(cur) = self.store.load()? {
                    if matches!(
                        cur.status,
                        WorkflowStatus::Running | WorkflowStatus::Blocked
                    ) {
                        return Err(format!(
                            "a workflow is already active: {:?} ({:?}); finish it with {COMPLETE_TOOL}",
                            cur.task.title, cur.status
                        ));
                    }
                }
                let kind: TaskKind = enum_arg(args, "kind")?.ok_or(
                    "missing `kind` (investigation|bug_fix|feature|refactor|performance|prototype)",
                )?;
                let mut profile = TaskProfile::new(str_arg(args, "title")?, kind);
                if let Some(risk) = enum_arg::<Risk>(args, "risk")? {
                    profile.risk = risk;
                }
                let state = WorkflowState::start(profile).map_err(|e| e.to_string())?;
                self.store.save(&state)?;
                Ok(format!(
                    "workflow started\n{}",
                    summary(&state, &(self.current)())
                ))
            }
            "evidence" => {
                let mut state = self.active()?;
                let gate = str_arg(args, "gate")?;
                let kind = enum_arg::<EvidenceKind>(args, "kind")?.unwrap_or(EvidenceKind::Command);
                let outcome = match (enum_arg::<Outcome>(args, "outcome")?, args.get("ok")) {
                    (Some(outcome), _) => outcome,
                    (None, Some(Value::Bool(ok))) => Outcome::from_ok(*ok),
                    _ => return Err("missing `outcome` (pass|fail|inconclusive)".to_string()),
                };
                let text = str_arg(args, "summary")?;
                let id = format!("e{}", state.evidence.len() + 1);
                let text_arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
                let (from_tool, needle) = (text_arg("from_tool"), text_arg("match"));
                let bound = match from_tool {
                    "" => None,
                    tool => (self.resolve)(tool, needle),
                };
                let current = (self.current)();
                let (item, origin) = match &bound {
                    Some(call) => {
                        let mut item = Evidence::attributed(
                            &id,
                            kind,
                            &[gate],
                            format!("{text} [{} {}]", call.tool_name, call.args_preview),
                            outcome,
                            &self.task_id,
                            &call.tool_call_id,
                        )
                        .with_code_state(current.bind(&named_paths(&call.args_preview, &current)));
                        let mut origin = format!(
                            "attributed to {} call `{}`",
                            call.tool_name, call.args_preview
                        );
                        if let Some(later) = &call.mutated_after {
                            let reason = format!("{later} ran after it, before this report");
                            origin.push_str(&format!("; STALE: {reason} (re-run it)"));
                            item.born_stale = Some(reason);
                        }
                        (item, origin)
                    }
                    None => (
                        Evidence::unattributed(&id, kind, &[gate], text, outcome)
                            .with_code_state(current.bind(&[])),
                        if from_tool.is_empty() {
                            "UNATTRIBUTED: no from_tool given".to_string()
                        } else {
                            format!(
                                "UNATTRIBUTED: no finished, successful {from_tool:?} call{} in this session",
                                if needle.is_empty() { String::new() } else { format!(" matching {needle:?}") }
                            )
                        },
                    ),
                };
                let mut item = item;
                if let Some(m) = args.get("measurement") {
                    item.measurement = Some(
                        serde_json::from_value(m.clone())
                            .map_err(|_| format!("bad `measurement`: {m}"))?,
                    );
                }
                state.attach(item).map_err(|e| e.to_string())?;
                self.store.save(&state)?;
                let gate_line = match state.gate_result(gate, &current) {
                    Ok(r) => format!("gate {gate}: {:?} — {}", r.status, r.reason),
                    Err(e) => e.to_string(),
                };
                Ok(format!("evidence {id} {origin}\n{gate_line}"))
            }
            "advance" => {
                let mut state = self.active()?;
                let passed = args
                    .get("passed")
                    .and_then(Value::as_bool)
                    .ok_or("missing `passed`")?;
                let current = (self.current)();
                state.advance(passed, &current).map_err(|e| e.to_string())?;
                self.store.save(&state)?;
                Ok(summary(&state, &current))
            }
            other => Err(format!("unknown op {other:?} (start|evidence|advance)")),
        }
    }

    /// `cedian_complete {claims?}`. No workflow → nothing to check (fast
    /// lane, ADR-0026). The claims ledger is checked and stored with the
    /// result either way (ADR-0024); flagged claims never block, gates do.
    /// Unmet required gates → error with what is missing; each counts one
    /// continue, and at `MAX_CONTINUE` the workflow is `blocked`.
    pub fn complete(&self, args: &Map<String, Value>) -> Result<String, String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut state) = self.store.load()? else {
            return Ok("no cedian workflow is active; nothing to check".to_string());
        };
        match state.status {
            WorkflowStatus::Complete => return Ok("workflow already complete".to_string()),
            WorkflowStatus::Blocked => {
                return Err(
                    "workflow is BLOCKED: stop and report the missing gates to the user"
                        .to_string(),
                )
            }
            _ => {}
        }
        let claims: Vec<Claim> = match args.get("claims") {
            None | Some(Value::Null) => Vec::new(),
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| {
                format!("bad `claims` ({e}): each is {{text, label: measured|inferred|guess, evidence: [ids]}}")
            })?,
        };
        let current = (self.current)();
        let checked = state.check_claims(claims, &current);
        let ledger = ledger_lines(&checked);
        let missing = match state.can_complete(&current) {
            Ok(_) => {
                state.complete(&current).map_err(|m| m.join("; "))?;
                state.last_completion = Some(CompletionAttempt {
                    claims: checked,
                    accepted: true,
                    missing: Vec::new(),
                    turn_ended: false,
                });
                self.store.save(&state)?;
                return Ok(format!("complete\n{}{ledger}", summary(&state, &current)));
            }
            Err(missing) => missing,
        };
        state.last_completion = Some(CompletionAttempt {
            claims: checked,
            accepted: false,
            missing: missing.clone(),
            turn_ended: false,
        });
        let failing: Vec<String> = state
            .all_gates(&current)
            .into_iter()
            .filter(|(id, r)| {
                r.status != GateStatus::Passed
                    && state
                        .playbook
                        .gates
                        .iter()
                        .any(|g| &g.id == id && g.required)
            })
            .map(|(id, _)| id)
            .collect();
        let mut budget = Vec::new();
        for gate in &failing {
            match state
                .record_continue(gate, &[], &current)
                .map_err(|e| e.to_string())?
            {
                ContinueOutcome::Continue { attempts_left } => {
                    budget.push(format!("{gate}: {attempts_left} attempt(s) left"));
                }
                ContinueOutcome::Blocked { attempts, .. } => {
                    budget.push(format!("{gate}: BLOCKED after {attempts} attempts"));
                }
            }
        }
        self.store.save(&state)?;
        let next = if state.status == WorkflowStatus::Blocked {
            "workflow is now BLOCKED: stop and report the missing gates to the user"
        } else {
            "not complete: produce the missing evidence, report it, then call cedian_complete again"
        };
        Err(format!(
            "{next}\n  - {}\n{}{ledger}",
            missing.join("\n  - "),
            budget.join("\n")
        ))
    }

    fn active(&self) -> Result<WorkflowState, String> {
        self.store.load()?.ok_or_else(|| {
            format!("no cedian workflow is active; start one with {WORKFLOW_UPDATE_TOOL} op=start")
        })
    }

    /// Both tools, for one `set_host_tools` call with the rest of the set.
    pub fn host_tools(self: &Arc<Self>) -> Vec<HostTool> {
        let update = Arc::clone(self);
        let complete = Arc::clone(self);
        vec![
            HostTool::new(
                WORKFLOW_UPDATE_TOOL,
                "cedian's own host tool (trusted). Report workflow progress to the cedian IDE. \
                 op=start {kind,title,risk?} begins a workflow; op=evidence {gate,summary,outcome,from_tool,match?,kind?} \
                 records what a tool call you already ran showed: from_tool = that tool's name (e.g. bash, read), \
                 match = a substring of its arguments; cedian binds the evidence to your most recent successful \
                 matching call (none found → stored unattributed, which cannot pass a required gate); \
                 outcome = pass | fail | inconclusive (could not run); evidence goes stale when a file it saw changes; \
                 performance evidence adds measurement {runs,median,range,limiter,build_profile} (any missing → inconclusive); \
                 op=advance {passed} moves to the next phase.",
                object(json!({
                    "op": {"type": "string", "enum": ["start", "evidence", "advance"]},
                    "kind": {"type": "string"},
                    "title": {"type": "string"},
                    "risk": {"type": "string", "enum": ["low", "medium", "high"]},
                    "gate": {"type": "string"},
                    "summary": {"type": "string"},
                    "outcome": {"type": "string", "enum": ["pass", "fail", "inconclusive"]},
                    "ok": {"type": "boolean"},
                    "from_tool": {"type": "string"},
                    "match": {"type": "string"},
                    "passed": {"type": "boolean"},
                    "measurement": {"type": "object", "properties": {
                        "runs": {"type": "integer"}, "median": {"type": "number"},
                        "range": {"type": "array", "items": {"type": "number"}},
                        "limiter": {"type": "string"}, "build_profile": {"type": "string"}
                    }}
                }), &["op"]),
                move |args, _ctx| update.update(&args).map(Into::into).map_err(Into::into),
            ),
            HostTool::new(
                COMPLETE_TOOL,
                "cedian's own host tool (trusted). Call before saying a task under a cedian workflow is done. \
                 claims = what you say is true, each {text, label, evidence}: label measured (you ran it and \
                 reported the evidence), inferred (follows from evidence) or guess; evidence = the evidence ids \
                 (e1, e2, ...) cedian returned. Returns an error listing missing gates when it is not done; keep \
                 working on those, or stop and report when it says BLOCKED.",
                object(json!({
                    "summary": {"type": "string"},
                    "claims": {"type": "array", "items": {"type": "object", "properties": {
                        "text": {"type": "string"},
                        "label": {"type": "string", "enum": ["measured", "inferred", "guess"]},
                        "evidence": {"type": "array", "items": {"type": "string"}}
                    }, "required": ["text", "label"]}}
                }), &[]),
                move |args, _ctx| complete.complete(&args).map(Into::into).map_err(Into::into),
            ),
        ]
    }
}

/// The claims ledger as the completion view shows it: every claim with its
/// label, evidence and flag (never hidden).
pub fn ledger_lines(claims: &[crate::CheckedClaim]) -> String {
    if claims.is_empty() {
        return "\nclaims: none given".to_string();
    }
    let mut out = String::from("\nclaims:");
    for c in claims {
        let label = format!("{:?}", c.claim.label).to_lowercase();
        let ids = if c.claim.evidence.is_empty() {
            "-".to_string()
        } else {
            c.claim.evidence.join(",")
        };
        let flag = c
            .flag
            .as_deref()
            .map(|f| format!("  ⚑ {f}"))
            .unwrap_or_default();
        out.push_str(&format!("\n  [{label}] {} ({ids}){flag}", c.claim.text));
    }
    out
}

/// One-screen status the agent reads back after each call.
fn summary(state: &WorkflowState, current: &CurrentState) -> String {
    let phase = state.current_phase.as_deref().unwrap_or("-");
    let gates: Vec<String> = state
        .all_gates(current)
        .into_iter()
        .map(|(id, r)| format!("{id}={:?}", r.status))
        .collect();
    format!(
        "{:?} · {:?} · phase {phase} · gates {}",
        state.task.kind,
        state.status,
        gates.join(" ")
    )
}

/// Workspace files a call's args name (`read notes.txt`, `path=src/a.rs`).
/// None named → the evidence binds to the whole tree.
pub fn named_paths(args_preview: &str, current: &CurrentState) -> Vec<String> {
    args_preview
        .split(|c: char| c.is_whitespace() || "\"'`,=:()[]{}".contains(c))
        .map(|t| t.trim_start_matches("./").trim_start_matches('/'))
        .filter(|t| current.files.contains_key(*t))
        .map(str::to_string)
        .collect()
}

fn str_arg<'a>(args: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing `{key}`"))
}

fn enum_arg<T: serde::de::DeserializeOwned>(
    args: &Map<String, Value>,
    key: &str,
) -> Result<Option<T>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone())
            .map(Some)
            .map_err(|_| format!("bad `{key}`: {v}")),
    }
}

fn object(properties: Value, required: &[&str]) -> Map<String, Value> {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
    .as_object()
    .cloned()
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_CONTINUE;

    #[derive(Default)]
    struct MemStore(Mutex<Option<WorkflowState>>);

    impl WorkflowStore for Arc<MemStore> {
        fn load(&self) -> Result<Option<WorkflowState>, String> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn save(&self, state: &WorkflowState) -> Result<(), String> {
            *self.0.lock().unwrap() = Some(state.clone());
            Ok(())
        }
    }

    type Ws = Arc<Mutex<CurrentState>>;

    fn files(text: &str) -> CurrentState {
        CurrentState::from_files([("notes.txt".to_string(), text.as_bytes())])
    }

    /// Channel whose log holds two good calls: `bash-1` (`cargo test`) and
    /// `read-1` (`notes.txt`); `ws` is the workspace the gates see.
    fn channel() -> (Arc<WorkflowChannel>, Arc<MemStore>, Ws) {
        let store = Arc::new(MemStore::default());
        let ws: Ws = Arc::new(Mutex::new(files("beta")));
        let now = Arc::clone(&ws);
        let ch = WorkflowChannel::new(
            "t1",
            Box::new(Arc::clone(&store)),
            |tool, needle| {
                let (id, preview) = match tool {
                    "bash" => ("bash-1", "cargo test"),
                    "read" => ("read-1", "notes.txt"),
                    _ => return None,
                };
                preview.contains(needle).then(|| BoundCall {
                    tool_call_id: id.into(),
                    tool_name: tool.into(),
                    args_preview: preview.into(),
                    mutated_after: None,
                })
            },
            move || now.lock().unwrap().clone(),
        );
        (ch, store, ws)
    }

    fn args(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn start(ch: &WorkflowChannel) {
        ch.update(&args(
            json!({"op": "start", "kind": "bug_fix", "title": "fix it", "risk": "low"}),
        ))
        .unwrap();
    }

    fn evidence(ch: &WorkflowChannel, gate: &str, ok: bool, tool: &str, needle: &str) -> String {
        // `ok` is the P5 wire alias for `outcome`.
        ch.update(&args(json!({
            "op": "evidence", "gate": gate, "summary": "ran it", "ok": ok,
            "from_tool": tool, "match": needle
        })))
        .unwrap()
    }

    #[test]
    fn no_workflow_completes_and_evidence_needs_one() {
        let (ch, _, _) = channel();
        assert!(ch
            .complete(&Map::new())
            .unwrap()
            .contains("nothing to check"));
        assert!(ch
            .update(&args(json!({"op": "advance", "passed": true})))
            .unwrap_err()
            .contains("no cedian workflow"));
    }

    #[test]
    fn evidence_is_attributed_only_through_the_verifier() {
        let (ch, store, _) = channel();
        start(&ch);
        assert!(evidence(&ch, "reproduce", false, "bash", "cargo").contains("attributed to bash"));
        assert!(evidence(&ch, "reproduce", false, "bash", "npm").contains("UNATTRIBUTED"));
        assert!(evidence(&ch, "reproduce", false, "grep", "").contains("UNATTRIBUTED"));
        assert!(ch
            .update(&args(
                json!({"op": "evidence", "gate": "verify", "summary": "s", "ok": true})
            ))
            .unwrap()
            .contains("no from_tool"));
        let state = store.load().unwrap().unwrap();
        let attributed: Vec<_> = state
            .evidence
            .values()
            .filter(|e| e.is_attributed())
            .collect();
        assert_eq!(attributed.len(), 1);
        assert_eq!(attributed[0].id, "e1");
        assert_eq!(
            attributed[0].provenance,
            crate::Provenance::Attributed {
                task_id: "t1".into(),
                tool_call_id: "bash-1".into()
            }
        );
    }

    #[test]
    fn second_start_is_refused_while_running() {
        let (ch, _, _) = channel();
        start(&ch);
        let err = ch
            .update(&args(
                json!({"op": "start", "kind": "feature", "title": "x"}),
            ))
            .unwrap_err();
        assert!(err.contains("already active"), "{err}");
    }

    #[test]
    fn complete_lists_missing_then_blocks_at_max_continue() {
        let (ch, store, _) = channel();
        start(&ch);
        // Unattributed support only: required gates still fail.
        evidence(&ch, "verify", true, "bash", "npm");
        for attempt in 1..=MAX_CONTINUE {
            let err = ch.complete(&Map::new()).unwrap_err();
            assert!(err.contains("required gate \"verify\""), "{err}");
            if attempt < MAX_CONTINUE {
                assert!(err.contains("not complete"), "{err}");
            } else {
                assert!(err.contains("now BLOCKED"), "{err}");
            }
        }
        assert_eq!(
            store.load().unwrap().unwrap().status,
            WorkflowStatus::Blocked
        );
        assert!(ch.complete(&Map::new()).unwrap_err().contains("BLOCKED"));
    }

    #[test]
    fn attributed_evidence_and_phases_complete() {
        let (ch, store, _) = channel();
        start(&ch);
        evidence(&ch, "reproduce", false, "bash", "");
        evidence(&ch, "verify", true, "bash", "test");
        // reproduce → investigate → implement → verify; review skipped (low risk).
        for _ in 0..4 {
            ch.update(&args(json!({"op": "advance", "passed": true})))
                .unwrap();
        }
        let done = ch.complete(&Map::new()).unwrap();
        assert!(done.starts_with("complete"), "{done}");
        assert_eq!(
            store.load().unwrap().unwrap().status,
            WorkflowStatus::Complete
        );
    }

    #[test]
    fn verify_evidence_goes_stale_when_a_file_it_saw_changes() {
        let (ch, store, ws) = channel();
        start(&ch);
        evidence(&ch, "reproduce", false, "read", "notes");
        // Repo-wide test run: binds to the whole tree.
        evidence(&ch, "verify", true, "bash", "cargo test");
        let state = store.load().unwrap().unwrap();
        assert_eq!(
            state.evidence["e1"].code_state,
            Some(crate::CodeState::Files(
                [("notes.txt".to_string(), crate::content_hash(b"beta"))].into()
            ))
        );
        assert!(matches!(
            state.evidence["e2"].code_state,
            Some(crate::CodeState::Tree(_))
        ));
        *ws.lock().unwrap() = files("BETA");
        for _ in 0..4 {
            ch.update(&args(json!({"op": "advance", "passed": true})))
                .unwrap_or_else(|e| panic!("reproduce counts stale; only verify blocks: {e}"));
            if store.load().unwrap().unwrap().current_phase.as_deref() == Some("verify") {
                break;
            }
        }
        let err = ch.complete(&Map::new()).unwrap_err();
        assert!(err.contains("1 stale"), "{err}");
        // Re-capture after the edit: fresh, completes.
        evidence(&ch, "verify", true, "bash", "cargo test");
        ch.update(&args(json!({"op": "advance", "passed": true})))
            .unwrap();
        assert!(ch.complete(&Map::new()).unwrap().starts_with("complete"));
    }

    #[test]
    fn outcome_arg_and_born_stale_report() {
        let store = Arc::new(MemStore::default());
        let ch = WorkflowChannel::new(
            "t1",
            Box::new(Arc::clone(&store)),
            |_, _| {
                Some(BoundCall {
                    tool_call_id: "bash-1".into(),
                    tool_name: "bash".into(),
                    args_preview: "cargo test".into(),
                    mutated_after: Some("edit notes.txt".into()),
                })
            },
            || files("x"),
        );
        start(&ch);
        let out = ch
            .update(&args(json!({
                "op": "evidence", "gate": "verify", "summary": "s",
                "outcome": "inconclusive", "from_tool": "bash"
            })))
            .unwrap();
        assert!(out.contains("STALE: edit notes.txt ran after it"), "{out}");
        let e = &store.load().unwrap().unwrap().evidence["e1"];
        assert_eq!(e.outcome, Outcome::Inconclusive);
        assert!(e.born_stale.is_some());
        let err = ch
            .update(&args(
                json!({"op": "evidence", "gate": "verify", "summary": "s"}),
            ))
            .unwrap_err();
        assert!(err.contains("missing `outcome`"), "{err}");
    }

    #[test]
    fn complete_stores_and_shows_the_claims_ledger() {
        let (ch, store, _) = channel();
        start(&ch);
        evidence(&ch, "verify", true, "bash", "cargo test");
        let claims = json!({"claims": [
            {"text": "tests pass", "label": "measured", "evidence": ["e1"]},
            {"text": "no other callers", "label": "guess"}
        ]});
        let err = ch.complete(&args(claims)).unwrap_err();
        assert!(err.contains("[measured] tests pass (e1)\n"), "{err}");
        assert!(
            err.contains("[guess] no other callers (-)  ⚑ no evidence cited"),
            "{err}"
        );
        let last = store.load().unwrap().unwrap().last_completion.unwrap();
        assert!(!last.accepted);
        assert_eq!(last.claims.len(), 2);
        assert!(
            last.missing.iter().any(|m| m.contains("reproduce")),
            "{last:?}"
        );
        let bad = ch
            .complete(&args(json!({"claims": [{"text": "x", "label": "sure"}]})))
            .unwrap_err();
        assert!(bad.contains("bad `claims`"), "{bad}");
    }

    #[test]
    fn named_paths_finds_workspace_files_in_args() {
        let now = CurrentState::from_files([
            ("notes.txt".to_string(), b"".as_slice()),
            ("src/a.rs".to_string(), b"".as_slice()),
        ]);
        assert_eq!(named_paths("notes.txt", &now), ["notes.txt"]);
        assert_eq!(named_paths("{\"path\":\"./src/a.rs\"}", &now), ["src/a.rs"]);
        assert!(named_paths("cargo test", &now).is_empty());
    }

    #[test]
    fn channel_calls_are_recognized_by_name_and_device() {
        assert!(is_channel_call("cedian_complete", ""));
        assert!(is_channel_call("write", "xd://cedian_workflow_update"));
        assert!(!is_channel_call("write", "xd://cedian_apply_edit"));
        assert!(!is_channel_call("bash", "cargo test"));
        assert!(may_mutate("edit", "notes.txt"));
        assert!(may_mutate("bash", "ls"));
        assert!(!may_mutate("read", "notes.txt"));
        assert!(!may_mutate("write", "xd://cedian_complete"));
    }
}
