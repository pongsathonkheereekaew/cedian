//! Evidence as first-class data (§53): every gate decision reads evidence
//! items already stored in the `WorkflowState` evidence map. Each item carries
//! provenance (§53 R2 fix): `AgentEdit` link or `unattributed`.

use serde::{Deserialize, Serialize};

/// Provenance: link to the §17 AgentEdit store, or unattributed.
///
/// Required gates REJECT unattributed evidence; optional gates accept it but
/// flag `unverified-origin` in the `GateResult`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Produced by a tracked tool call (`task_id` + `tool_call_id`).
    Attributed {
        task_id: String,
        tool_call_id: String,
    },
    /// No tracked origin (hand-added note, external paste). Never passes a
    /// required gate.
    Unattributed,
}
/// Evidence kind (§53): command < test < browser/screenshot < simulator <
/// debugger < file. `Custom` covers future kinds without schema churn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Command,
    Test,
    Browser,
    Screenshot,
    Simulator,
    Debugger,
    File,
    Custom,
}

/// One evidence item: what was observed, which gate(s) it supports, and
/// where it came from. `ok: false` records a FAILING observation (e.g. a
/// reproduction that still fails) — gates read it, they never re-run it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    /// Stable id within the workflow (dedup key for the continue rule).
    pub id: String,
    pub kind: EvidenceKind,
    /// Gate ids this item supports (empty = informational only).
    pub for_gates: Vec<String>,
    /// Human-readable observation ("repro fails with E0502", "cargo test
    /// auth: 14 passed"). Gates match on structured fields, not prose.
    pub summary: String,
    /// Whether the observation was a success. Failing evidence SATISFIES a
    /// `reproduction` gate (it proves the bug exists) but FAILS `test`/`build`
    /// gates.
    pub ok: bool,
    pub provenance: Provenance,
}

impl Evidence {
    /// New attributed item supporting the given gates.
    pub fn attributed(
        id: impl Into<String>,
        kind: EvidenceKind,
        for_gates: &[&str],
        summary: impl Into<String>,
        ok: bool,
        task_id: impl Into<String>,
        tool_call_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            for_gates: for_gates.iter().map(|s| s.to_string()).collect(),
            summary: summary.into(),
            ok,
            provenance: Provenance::Attributed {
                task_id: task_id.into(),
                tool_call_id: tool_call_id.into(),
            },
        }
    }

    /// New unattributed item (hand note / external paste).
    pub fn unattributed(
        id: impl Into<String>,
        kind: EvidenceKind,
        for_gates: &[&str],
        summary: impl Into<String>,
        ok: bool,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            for_gates: for_gates.iter().map(|s| s.to_string()).collect(),
            summary: summary.into(),
            ok,
            provenance: Provenance::Unattributed,
        }
    }

    pub fn is_attributed(&self) -> bool {
        matches!(self.provenance, Provenance::Attributed { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_flag() {
        let a = Evidence::attributed("e1", EvidenceKind::Test, &["g"], "ok", true, "t", "c");
        let u = Evidence::unattributed("e2", EvidenceKind::File, &["g"], "note", true);
        assert!(a.is_attributed());
        assert!(!u.is_attributed());
    }
}
