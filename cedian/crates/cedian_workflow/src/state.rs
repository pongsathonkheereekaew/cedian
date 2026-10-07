//! WorkflowState (§48): task + playbook + phase/gate progress + status.
//!
//! All transitions are pure: `attach` (evidence in), `advance` (phase
//! forward), `can_complete` (§55 completion gate). The continue budget
//! (§54: `MAX_CONTINUE` per gate) lives here — `record_continue` returns
//! `Blocked` on exhaustion so the caller force-escalates to the user.

use super::code_state::CurrentState;
use super::evidence::Evidence;
use super::gate::{GateResult, GateStatus, MAX_CONTINUE};
use super::playbook::{PhaseId, Playbook};
use super::profile::TaskProfile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Phase runtime state (§50).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Blocked,
    Skipped,
}

/// One phase + optional skip reason (§50: skips always carry a reason).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseState {
    pub id: PhaseId,
    pub status: PhaseStatus,
    pub skip_reason: Option<String>,
}

/// Workflow status (§48).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Planning,
    Running,
    Blocked,
    ReadyForReview,
    Complete,
    Failed,
}

/// Continue-budget outcome for one gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinueOutcome {
    /// Caller should ask OMP to produce the missing evidence.
    Continue { attempts_left: u32 },
    /// Budget exhausted: caller MUST escalate `{gate_id, missing_evidence,
    /// attempts}` to the user; status is now `blocked`.
    Blocked {
        gate_id: String,
        missing_evidence: String,
        attempts: u32,
    },
}

/// Errors from state transitions (caller misuse, never engine I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    UnknownPlaybook(String),
    UnknownPhase(String),
    DuplicateEvidence(String),
    NoSuchGate(String),
    /// Phase exit refused: required gates unpassed (gates block, not warn).
    BlockedByGates {
        phase: String,
        gates: Vec<String>,
    },
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPlaybook(id) => write!(f, "unknown playbook {id:?}"),
            Self::UnknownPhase(id) => write!(f, "unknown phase {id:?}"),
            Self::DuplicateEvidence(id) => write!(f, "duplicate evidence {id:?}"),
            Self::NoSuchGate(id) => write!(f, "no gate {id:?}"),
            Self::BlockedByGates { phase, gates } => {
                write!(f, "phase {phase:?} blocked by gates: {}", gates.join(", "))
            }
        }
    }
}

/// The workflow: profile + playbook snapshot + evidence map + progress.
///
/// The playbook is snapshotted at `start` (later playbook edits don't move a
/// running workflow). Evidence is keyed by id; `continue_used` counts OMP
/// continues per gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub task: TaskProfile,
    pub playbook: Playbook,
    pub current_phase: Option<PhaseId>,
    pub phases: Vec<PhaseState>,
    pub evidence: HashMap<String, Evidence>,
    pub continue_used: HashMap<String, u32>,
    pub status: WorkflowStatus,
}

impl WorkflowState {
    /// Start a workflow: snapshot the builtin playbook, apply risk-based
    /// skips (§56), enter the first non-skipped phase.
    pub fn start(task: TaskProfile) -> Result<Self, WorkflowError> {
        let playbook = Playbook::builtin(task.playbook_id())
            .ok_or_else(|| WorkflowError::UnknownPlaybook(task.playbook_id().to_string()))?;
        let skipped: Vec<&str> = playbook
            .skipped_for_risk(task.risk)
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        let mut phases: Vec<PhaseState> = playbook
            .phases
            .iter()
            .map(|p| {
                if skipped.contains(&p.id.as_str()) {
                    PhaseState {
                        id: p.id.clone(),
                        status: PhaseStatus::Skipped,
                        skip_reason: p.skip_when.clone(),
                    }
                } else {
                    PhaseState {
                        id: p.id.clone(),
                        status: PhaseStatus::Pending,
                        skip_reason: None,
                    }
                }
            })
            .collect();
        let current_phase = phases
            .iter()
            .find(|p| p.status == PhaseStatus::Pending)
            .map(|p| p.id.clone());
        if let Some(cur) = &current_phase {
            if let Some(ps) = phases.iter_mut().find(|p| &p.id == cur) {
                ps.status = PhaseStatus::Running;
            }
        }
        Ok(Self {
            task,
            playbook,
            current_phase,
            phases,
            evidence: HashMap::new(),
            continue_used: HashMap::new(),
            status: WorkflowStatus::Running,
        })
    }

    /// Attach evidence (the agent loop produced it; the engine only stores).
    /// Duplicate ids are rejected — resubmit under a new id.
    pub fn attach(&mut self, item: Evidence) -> Result<(), WorkflowError> {
        if self.evidence.contains_key(&item.id) {
            return Err(WorkflowError::DuplicateEvidence(item.id));
        }
        self.evidence.insert(item.id.clone(), item);
        Ok(())
    }

    /// All stored evidence as a vec (gate input).
    pub fn evidence_list(&self) -> Vec<Evidence> {
        self.evidence.values().cloned().collect()
    }

    /// Evaluate one gate over the stored map and the current code state.
    pub fn gate_result(
        &self,
        gate_id: &str,
        current: &CurrentState,
    ) -> Result<GateResult, WorkflowError> {
        let gate = self
            .playbook
            .gates
            .iter()
            .find(|g| g.id == gate_id)
            .ok_or_else(|| WorkflowError::NoSuchGate(gate_id.to_string()))?;
        Ok(gate.evaluate(&self.evidence_list(), current))
    }

    /// Evaluate every gate in playbook order.
    pub fn all_gates(&self, current: &CurrentState) -> Vec<(String, GateResult)> {
        self.playbook
            .gates
            .iter()
            .map(|g| (g.id.clone(), g.evaluate(&self.evidence_list(), current)))
            .collect()
    }

    /// Advance the current phase: `passed` moves to the next pending phase
    /// (or `ReadyForReview` past the last one); `failed`/`blocked` propagate
    /// to workflow status. Advancing past a REQUIRED phase whose gates are
    /// unpassed is refused — gates block, not warn.
    pub fn advance(&mut self, passed: bool, current: &CurrentState) -> Result<(), WorkflowError> {
        let cur = self
            .current_phase
            .clone()
            .ok_or_else(|| WorkflowError::UnknownPhase("<none>".to_string()))?;
        let idx = self
            .phases
            .iter()
            .position(|p| p.id == cur)
            .ok_or_else(|| WorkflowError::UnknownPhase(cur.clone()))?;
        if passed {
            let unpassed: Vec<String> = self
                .blocking_gates(&cur)
                .into_iter()
                .filter(|g| {
                    self.gate_result(&g.id, current)
                        .map(|r| r.status != GateStatus::Passed)
                        .unwrap_or(true)
                })
                .map(|g| g.id.clone())
                .collect();
            if !unpassed.is_empty() {
                return Err(WorkflowError::BlockedByGates {
                    phase: cur,
                    gates: unpassed,
                });
            }
            self.phases[idx].status = PhaseStatus::Passed;
        } else {
            self.phases[idx].status = PhaseStatus::Failed;
            self.status = WorkflowStatus::Failed;
            return Ok(());
        }
        let next = self
            .phases
            .iter()
            .skip(idx + 1)
            .find(|p| p.status == PhaseStatus::Pending);
        match next {
            Some(n) => {
                let id = n.id.clone();
                if let Some(ps) = self.phases.iter_mut().find(|p| p.id == id) {
                    ps.status = PhaseStatus::Running;
                }
                self.current_phase = Some(id);
            }
            None => {
                self.current_phase = None;
                self.status = WorkflowStatus::ReadyForReview;
            }
        }
        Ok(())
    }

    /// Gates blocking exit from `phase_id`: the required gate with the same
    /// id (name-matched, e.g. phase `reproduce` ↔ gate `reproduce`), plus at
    /// the terminal verify-like phases (`verify`/`report`/`work`) ALL
    /// required gates — nothing required may stay unpassed at completion.
    fn blocking_gates(&self, phase_id: &str) -> Vec<crate::gate::Gate> {
        let terminal = matches!(phase_id, "verify" | "report" | "work");
        self.playbook
            .gates
            .iter()
            .filter(|g| g.required && (g.id == phase_id || terminal))
            .cloned()
            .collect()
    }

    /// Completion gate (§55): `canComplete`. Required gates ALL passed (and
    /// required phases passed/skipped, never pending) → `Ok(true)`. Else the
    /// missing reasons, so OMP knows what evidence to produce next.
    pub fn can_complete(&self, current: &CurrentState) -> Result<bool, Vec<String>> {
        let mut missing = Vec::new();
        for gate in self.playbook.gates.iter().filter(|g| g.required) {
            let r = gate.evaluate(&self.evidence_list(), current);
            if r.status != GateStatus::Passed {
                missing.push(format!("required gate {:?}: {}", gate.id, r.reason));
            }
        }
        for ps in &self.phases {
            let decl = self.playbook.phases.iter().find(|p| p.id == ps.id);
            let required = decl.map(|d| d.required).unwrap_or(true);
            if required && !matches!(ps.status, PhaseStatus::Passed | PhaseStatus::Skipped) {
                missing.push(format!("phase {:?} is {:?}", ps.id, ps.status));
            }
        }
        if missing.is_empty() {
            Ok(true)
        } else {
            Err(missing)
        }
    }

    /// Record an OMP continue for a gate (§54 anti-loop). `new_evidence_ids`:
    /// ids attached since the last continue for this gate — empty advances
    /// the counter WITHOUT progress. On exhaustion the workflow blocks and
    /// the caller escalates.
    pub fn record_continue(
        &mut self,
        gate_id: &str,
        new_evidence_ids: &[String],
        current: &CurrentState,
    ) -> Result<ContinueOutcome, WorkflowError> {
        if self.playbook.gates.iter().all(|g| g.id != gate_id) {
            return Err(WorkflowError::NoSuchGate(gate_id.to_string()));
        }
        let used = self.continue_used.get(gate_id).copied().unwrap_or(0) + 1;
        self.continue_used.insert(gate_id.to_string(), used);
        if used >= MAX_CONTINUE {
            self.status = WorkflowStatus::Blocked;
            if let Some(ps) = self
                .phases
                .iter_mut()
                .find(|p| Some(&p.id) == self.current_phase.as_ref())
            {
                ps.status = PhaseStatus::Blocked;
            }
            let reason = self
                .gate_result(gate_id, current)
                .map(|r| r.reason)
                .unwrap_or_else(|_| "unknown".to_string());
            return Ok(ContinueOutcome::Blocked {
                gate_id: gate_id.to_string(),
                missing_evidence: reason,
                attempts: used,
            });
        }
        let _ = new_evidence_ids;
        Ok(ContinueOutcome::Continue {
            attempts_left: MAX_CONTINUE - used,
        })
    }

    /// Mark complete. Refused unless `can_complete` holds — never pretend
    /// success (§55: otherwise `status = blocked`).
    pub fn complete(&mut self, current: &CurrentState) -> Result<(), Vec<String>> {
        match self.can_complete(current) {
            Ok(true) => {
                self.status = WorkflowStatus::Complete;
                Ok(())
            }
            Ok(false) => unreachable!(),
            Err(missing) => {
                self.status = WorkflowStatus::Blocked;
                Err(missing)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{EvidenceKind, Outcome, Provenance};
    use crate::profile::{Complexity, Risk, Surface, TaskKind};

    fn bugfix_task(risk: Risk) -> TaskProfile {
        TaskProfile {
            title: "fix login redirect".to_string(),
            kind: TaskKind::BugFix,
            complexity: Complexity::Small,
            risk,
            surfaces: vec![Surface::Web],
            constraints: vec![],
            acceptance_criteria: vec![],
        }
    }

    fn ws() -> CurrentState {
        CurrentState::from_files([("a.rs".to_string(), b"1".as_slice())])
    }

    fn attributed(id: &str, gates: &[&str], ok: bool) -> Evidence {
        Evidence {
            id: id.to_string(),
            kind: EvidenceKind::Test,
            for_gates: gates.iter().map(|s| s.to_string()).collect(),
            summary: "evidence".to_string(),
            outcome: Outcome::from_ok(ok),
            provenance: Provenance::Attributed {
                task_id: "t".to_string(),
                tool_call_id: "c".to_string(),
            },
            code_state: Some(ws().bind(&[])),
            born_stale: None,
            measurement: None,
        }
    }

    #[test]
    fn start_enters_first_phase_and_skips_review_on_low_risk() {
        let w = WorkflowState::start(bugfix_task(Risk::Low)).unwrap();
        assert_eq!(w.current_phase.as_deref(), Some("reproduce"));
        let review = w.phases.iter().find(|p| p.id == "review").unwrap();
        assert_eq!(review.status, PhaseStatus::Skipped);
        assert!(review.skip_reason.is_some());
    }

    #[test]
    fn duplicate_evidence_rejected() {
        let mut w = WorkflowState::start(bugfix_task(Risk::High)).unwrap();
        w.attach(attributed("e1", &["repro"], false)).unwrap();
        assert!(matches!(
            w.attach(attributed("e1", &["repro"], false)),
            Err(WorkflowError::DuplicateEvidence(_))
        ));
    }

    #[test]
    fn continue_budget_blocks_on_exhaustion() {
        let mut w = WorkflowState::start(bugfix_task(Risk::High)).unwrap();
        assert!(matches!(
            w.record_continue("verify", &[], &ws()).unwrap(),
            ContinueOutcome::Continue { attempts_left: 2 }
        ));
        assert!(matches!(
            w.record_continue("verify", &[], &ws()).unwrap(),
            ContinueOutcome::Continue { attempts_left: 1 }
        ));
        let blocked = w.record_continue("verify", &[], &ws()).unwrap();
        assert!(matches!(blocked, ContinueOutcome::Blocked { .. }));
        assert_eq!(w.status, WorkflowStatus::Blocked);
    }

    #[test]
    fn cannot_complete_without_evidence() {
        let mut w = WorkflowState::start(bugfix_task(Risk::High)).unwrap();
        assert!(w.can_complete(&ws()).is_err());
        assert!(w.complete(&ws()).is_err());
        assert_eq!(w.status, WorkflowStatus::Blocked);
    }

    #[test]
    fn bugfix_run_reproduce_to_verify_gated() {
        // S2 exit: reproduce → investigate → implement → verify, gates block.
        let mut w = WorkflowState::start(bugfix_task(Risk::High)).unwrap();
        // reproduce phase exits only with reproduce-gate evidence.
        assert!(w.advance(true, &ws()).is_err());
        w.attach(Evidence {
            id: "r1".to_string(),
            kind: EvidenceKind::Command,
            for_gates: vec!["reproduce".to_string()],
            summary: "repro fails with E0502".to_string(),
            outcome: Outcome::Fail, // failing observation SATISFIES a repro gate
            provenance: Provenance::Attributed {
                task_id: "t".to_string(),
                tool_call_id: "c1".to_string(),
            },
            code_state: Some(ws().bind(&[])),
            born_stale: None,
            measurement: None,
        })
        .unwrap();
        w.advance(true, &ws()).unwrap();
        assert_eq!(w.current_phase.as_deref(), Some("investigate"));
        w.advance(true, &ws()).unwrap();
        w.advance(true, &ws()).unwrap();
        assert_eq!(w.current_phase.as_deref(), Some("verify"));
        // verify is terminal: ALL required gates (reproduce + verify) apply.
        // reproduce passed, verify missing → blocked.
        assert!(w.advance(true, &ws()).is_err());
        assert!(w.can_complete(&ws()).is_err());
        w.attach(attributed("v1", &["verify"], true)).unwrap();
        w.advance(true, &ws()).unwrap();
        w.advance(true, &ws()).unwrap(); // review (kept: high risk)
        w.complete(&ws()).unwrap();
        assert_eq!(w.status, WorkflowStatus::Complete);
    }
}
