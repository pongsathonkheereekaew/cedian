//! `cedian_workflow`: TaskProfile + Playbook + Evidence + Gate + WorkflowState.
//!
//! Plan §§46–56 (Phase 10): the workflow layer defines REQUIREMENTS; OMP's
//! normal agent/tool loop satisfies them. `Gate::evaluate` is a pure function
//! over the stored evidence map — no spawn, no browser, no test run, no RPC
//! (§54 R2 fix). Predicates needing fresh I/O are rejected at registration:
//! the caller gets `GateError::NeedsIo`, never a hidden side effect.
//!
//! Anti-loop (§54): continues are bounded by `MAX_CONTINUE` (3) per gate.
//! Each continue must attach ≥1 NEW evidence item referencing the missing
//! gate, or the counter still advances. On exhaustion: `blocked` + the caller
//! force-escalates `{gate_id, missing_evidence, attempts}` to the user.
//!
//! Evidence carries provenance (§53 R2 fix): every item links to the §17
//! `AgentEdit` store via `tool_call_id`, or is `unattributed`. A `required`
//! gate REJECTS unattributed evidence; optional gates accept it but surface
//! `unverified-origin` in the result.
//!
//! V1 playbooks (§49): investigation, bug_fix, feature, refactor, generic.

pub mod channel;
pub mod evidence;
pub mod gate;
pub mod playbook;
pub mod profile;
pub mod state;

pub use channel::{
    is_channel_call, WorkflowChannel, WorkflowStore, CHANNEL_TOOLS, COMPLETE_TOOL,
    WORKFLOW_UPDATE_TOOL,
};
pub use evidence::{Evidence, EvidenceKind, Provenance};
pub use gate::{Gate, GateError, GateId, GateKind, GateResult, GateStatus, MAX_CONTINUE};
pub use playbook::{Phase, PhaseId, Playbook, BUILTINS};
pub use profile::{AcceptanceCriterion, Complexity, Risk, Surface, TaskKind, TaskProfile};
pub use state::{
    ContinueOutcome, PhaseState, PhaseStatus, WorkflowError, WorkflowState, WorkflowStatus,
};
