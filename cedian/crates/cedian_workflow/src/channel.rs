//! Host-tool channel (P5, ADR-0022): OMP drives the workflow through
//! `cedian_workflow_update` and claims completion through `cedian_complete`.
//!
//! The engine stays pure: this module only adapts JSON args to
//! `WorkflowState` calls. Two seams keep it free of the runtime crates:
//! - [`WorkflowStore`]: every call loads, mutates and saves, so the store on
//!   disk stays the one truth even when CLI verbs run between turns.
//! - the verifier `Fn(&str) -> bool`: true only for a `tool_call_id` the
//!   router log saw finish successfully (built by the caller over
//!   `EventRouter::finished_tool_call`, excluding [`is_channel_call`]s).
//!   The agent cannot attribute evidence by saying so.

use crate::{
    state::ContinueOutcome, Evidence, EvidenceKind, GateStatus, Risk, TaskKind, TaskProfile,
    WorkflowState, WorkflowStatus,
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

/// Where the workflow lives between calls (`.cedian/workflow.json` in the CLI).
pub trait WorkflowStore: Send + Sync {
    /// `Ok(None)` when no workflow was ever started.
    fn load(&self) -> Result<Option<WorkflowState>, String>;
    fn save(&self, state: &WorkflowState) -> Result<(), String>;
}

type Verifier = dyn Fn(&str) -> bool + Send + Sync;

/// The two workflow host tools over one store + one verifier.
pub struct WorkflowChannel {
    task_id: String,
    store: Box<dyn WorkflowStore>,
    verify: Box<Verifier>,
    /// Serializes load → mutate → save across handler threads.
    lock: Mutex<()>,
}

impl WorkflowChannel {
    pub fn new(
        task_id: impl Into<String>,
        store: Box<dyn WorkflowStore>,
        verify: impl Fn(&str) -> bool + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            task_id: task_id.into(),
            store,
            verify: Box::new(verify),
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
                Ok(format!("workflow started\n{}", summary(&state)))
            }
            "evidence" => {
                let mut state = self.active()?;
                let gate = str_arg(args, "gate")?;
                let kind = enum_arg::<EvidenceKind>(args, "kind")?.unwrap_or(EvidenceKind::Command);
                let ok = args
                    .get("ok")
                    .and_then(Value::as_bool)
                    .ok_or("missing `ok` (true = the observation succeeded)")?;
                let text = str_arg(args, "summary")?;
                let id = format!("e{}", state.evidence.len() + 1);
                let cited = args
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let attributed = !cited.is_empty() && (self.verify)(cited);
                let item = if attributed {
                    Evidence::attributed(&id, kind, &[gate], text, ok, &self.task_id, cited)
                } else {
                    Evidence::unattributed(&id, kind, &[gate], text, ok)
                };
                state.attach(item).map_err(|e| e.to_string())?;
                self.store.save(&state)?;
                let origin = if attributed {
                    format!("attributed to {cited}")
                } else if cited.is_empty() {
                    "UNATTRIBUTED: no tool_call_id given".to_string()
                } else {
                    format!(
                        "UNATTRIBUTED: {cited} is not a finished, successful tool call in this session"
                    )
                };
                let gate_line = match state.gate_result(gate) {
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
                state.advance(passed).map_err(|e| e.to_string())?;
                self.store.save(&state)?;
                Ok(summary(&state))
            }
            other => Err(format!("unknown op {other:?} (start|evidence|advance)")),
        }
    }

    /// `cedian_complete`. No workflow → nothing to check (fast lane,
    /// ADR-0026). Unmet required gates → error with what is missing; each
    /// counts one continue, and at `MAX_CONTINUE` the workflow is `blocked`.
    pub fn complete(&self, _args: &Map<String, Value>) -> Result<String, String> {
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
        let missing = match state.can_complete() {
            Ok(_) => {
                state.complete().map_err(|m| m.join("; "))?;
                self.store.save(&state)?;
                return Ok(format!("complete\n{}", summary(&state)));
            }
            Err(missing) => missing,
        };
        let failing: Vec<String> = state
            .all_gates()
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
                .record_continue(gate, &[])
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
            "{next}\n  - {}\n{}",
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
                 op=start {kind,title,risk?} begins a workflow; op=evidence {gate,summary,ok,tool_call_id,kind?} \
                 records what a tool call you already ran showed (cite that call's tool_call_id — evidence without \
                 a real finished call is stored unattributed and cannot pass a required gate); \
                 op=advance {passed} moves to the next phase.",
                object(json!({
                    "op": {"type": "string", "enum": ["start", "evidence", "advance"]},
                    "kind": {"type": "string"},
                    "title": {"type": "string"},
                    "risk": {"type": "string", "enum": ["low", "medium", "high"]},
                    "gate": {"type": "string"},
                    "summary": {"type": "string"},
                    "ok": {"type": "boolean"},
                    "tool_call_id": {"type": "string"},
                    "passed": {"type": "boolean"}
                }), &["op"]),
                move |args, _ctx| update.update(&args).map(Into::into).map_err(Into::into),
            ),
            HostTool::new(
                COMPLETE_TOOL,
                "cedian's own host tool (trusted). Call before saying a task under a cedian workflow is done. \
                 Returns an error listing missing gates when it is not; keep working on those, or stop and \
                 report when it says BLOCKED.",
                object(json!({"summary": {"type": "string"}}), &[]),
                move |args, _ctx| complete.complete(&args).map(Into::into).map_err(Into::into),
            ),
        ]
    }
}

/// One-screen status the agent reads back after each call.
fn summary(state: &WorkflowState) -> String {
    let phase = state.current_phase.as_deref().unwrap_or("-");
    let gates: Vec<String> = state
        .all_gates()
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

    /// Channel whose verifier knows exactly one good call: `bash-1`.
    fn channel() -> (Arc<WorkflowChannel>, Arc<MemStore>) {
        let store = Arc::new(MemStore::default());
        let ch = WorkflowChannel::new("t1", Box::new(Arc::clone(&store)), |id| id == "bash-1");
        (ch, store)
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

    fn evidence(ch: &WorkflowChannel, gate: &str, ok: bool, call: &str) -> String {
        ch.update(&args(json!({
            "op": "evidence", "gate": gate, "summary": "ran it", "ok": ok, "tool_call_id": call
        })))
        .unwrap()
    }

    #[test]
    fn no_workflow_completes_and_evidence_needs_one() {
        let (ch, _) = channel();
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
        let (ch, store) = channel();
        start(&ch);
        assert!(evidence(&ch, "reproduce", false, "bash-1").contains("attributed to bash-1"));
        assert!(evidence(&ch, "reproduce", false, "made-up").contains("UNATTRIBUTED"));
        assert!(ch
            .update(&args(
                json!({"op": "evidence", "gate": "verify", "summary": "s", "ok": true})
            ))
            .unwrap()
            .contains("no tool_call_id"));
        let state = store.load().unwrap().unwrap();
        let attributed: Vec<_> = state
            .evidence
            .values()
            .filter(|e| e.is_attributed())
            .collect();
        assert_eq!(attributed.len(), 1);
        assert_eq!(attributed[0].id, "e1");
    }

    #[test]
    fn second_start_is_refused_while_running() {
        let (ch, _) = channel();
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
        let (ch, store) = channel();
        start(&ch);
        // Unattributed support only: required gates still fail.
        evidence(&ch, "verify", true, "made-up");
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
        let (ch, store) = channel();
        start(&ch);
        evidence(&ch, "reproduce", false, "bash-1");
        evidence(&ch, "verify", true, "bash-1");
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
    fn channel_calls_are_recognized_by_name_and_device() {
        assert!(is_channel_call("cedian_complete", ""));
        assert!(is_channel_call("write", "xd://cedian_workflow_update"));
        assert!(!is_channel_call("write", "xd://cedian_apply_edit"));
        assert!(!is_channel_call("bash", "cargo test"));
    }
}
