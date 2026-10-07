//! Host-tool channel (P5, ADR-0022): OMP drives the workflow through
//! `cedian_workflow_update` and claims completion through `cedian_complete`.
//!
//! The engine stays pure: this module only adapts JSON args to
//! `WorkflowState` calls. Two seams keep it free of the runtime crates:
//! - [`WorkflowStore`]: every call loads, mutates and saves, so the store on
//!   disk stays the one truth even when CLI verbs run between turns.
//! - the resolver `Fn(tool, needle) -> Option<BoundCall>`: the agent names
//!   the tool that produced the evidence (`from_tool`, optional `match` on
//!   its args); the caller binds that to the most recent call the router log
//!   saw finish successfully, excluding [`is_channel_call`]s (ADR-0031: the
//!   model never sees `tool_call_id`s). The id always comes from the log, so
//!   the agent cannot attribute evidence by saying so.

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

/// A logged call evidence was bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCall {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args_preview: String,
}

type Resolver = dyn Fn(&str, &str) -> Option<BoundCall> + Send + Sync;

/// The two workflow host tools over one store + one resolver.
pub struct WorkflowChannel {
    task_id: String,
    store: Box<dyn WorkflowStore>,
    resolve: Box<Resolver>,
    /// Serializes load → mutate → save across handler threads.
    lock: Mutex<()>,
}

impl WorkflowChannel {
    pub fn new(
        task_id: impl Into<String>,
        store: Box<dyn WorkflowStore>,
        resolve: impl Fn(&str, &str) -> Option<BoundCall> + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            task_id: task_id.into(),
            store,
            resolve: Box::new(resolve),
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
                let text_arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
                let (from_tool, needle) = (text_arg("from_tool"), text_arg("match"));
                let bound = match from_tool {
                    "" => None,
                    tool => (self.resolve)(tool, needle),
                };
                let (item, origin) = match &bound {
                    Some(call) => (
                        Evidence::attributed(
                            &id,
                            kind,
                            &[gate],
                            format!("{text} [{} {}]", call.tool_name, call.args_preview),
                            ok,
                            &self.task_id,
                            &call.tool_call_id,
                        ),
                        format!(
                            "attributed to {} call `{}`",
                            call.tool_name, call.args_preview
                        ),
                    ),
                    None => (
                        Evidence::unattributed(&id, kind, &[gate], text, ok),
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
                state.attach(item).map_err(|e| e.to_string())?;
                self.store.save(&state)?;
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
                 op=start {kind,title,risk?} begins a workflow; op=evidence {gate,summary,ok,from_tool,match?,kind?} \
                 records what a tool call you already ran showed: from_tool = that tool's name (e.g. bash, read), \
                 match = a substring of its arguments; cedian binds the evidence to your most recent successful \
                 matching call (none found → stored unattributed, which cannot pass a required gate); \
                 op=advance {passed} moves to the next phase.",
                object(json!({
                    "op": {"type": "string", "enum": ["start", "evidence", "advance"]},
                    "kind": {"type": "string"},
                    "title": {"type": "string"},
                    "risk": {"type": "string", "enum": ["low", "medium", "high"]},
                    "gate": {"type": "string"},
                    "summary": {"type": "string"},
                    "ok": {"type": "boolean"},
                    "from_tool": {"type": "string"},
                    "match": {"type": "string"},
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

    /// Channel whose log holds exactly one good call: `bash-1` (`cargo test`).
    fn channel() -> (Arc<WorkflowChannel>, Arc<MemStore>) {
        let store = Arc::new(MemStore::default());
        let ch = WorkflowChannel::new("t1", Box::new(Arc::clone(&store)), |tool, needle| {
            (tool == "bash" && "cargo test".contains(needle)).then(|| BoundCall {
                tool_call_id: "bash-1".into(),
                tool_name: "bash".into(),
                args_preview: "cargo test".into(),
            })
        });
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

    fn evidence(ch: &WorkflowChannel, gate: &str, ok: bool, tool: &str, needle: &str) -> String {
        ch.update(&args(json!({
            "op": "evidence", "gate": gate, "summary": "ran it", "ok": ok,
            "from_tool": tool, "match": needle
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
        assert!(evidence(&ch, "reproduce", false, "bash", "cargo").contains("attributed to bash"));
        assert!(evidence(&ch, "reproduce", false, "bash", "npm").contains("UNATTRIBUTED"));
        assert!(evidence(&ch, "reproduce", false, "read", "").contains("UNATTRIBUTED"));
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
        let (ch, store) = channel();
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
    fn channel_calls_are_recognized_by_name_and_device() {
        assert!(is_channel_call("cedian_complete", ""));
        assert!(is_channel_call("write", "xd://cedian_workflow_update"));
        assert!(!is_channel_call("write", "xd://cedian_apply_edit"));
        assert!(!is_channel_call("bash", "cargo test"));
    }
}
