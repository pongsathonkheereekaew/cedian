//! Gate engine (§§52/54): pure predicates over the stored evidence map.
//!
//! A `Gate` declares what evidence it needs; `evaluate` reads items only.
//! Registration REJECTS predicates that need fresh I/O (`GateError::NeedsIo`)
//! — the agent produces evidence first, the gate only reads it.

use super::evidence::{Evidence, EvidenceKind};
use serde::{Deserialize, Serialize};

/// Max OMP continues per gate before `blocked` + force-escalate (§54).
pub const MAX_CONTINUE: u32 = 3;

/// Stable gate id (also the `for_gates` key evidence references).
pub type GateId = String;

/// Gate kind (§52). `Review` gates read approval evidence; `Custom` carries a
/// free-form predicate id in `predicate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    Build,
    Test,
    Lint,
    Reproduction,
    Behavior,
    Visual,
    Performance,
    Review,
    Custom(String),
}

/// Predicate: what stored evidence satisfies this gate. Pure data — the
/// engine matches items, it never fetches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatePredicate {
    /// Evidence kinds accepted (empty = any kind).
    pub kinds: Vec<EvidenceKind>,
    /// Minimum number of ATTRIBUTED supporting items (`ok` as required).
    pub min_items: usize,
    /// When true the supporting items must have `ok: true` (passing result).
    /// `false` for `reproduction` gates: a FAILING repro proves the bug.
    pub require_ok: bool,
}

/// A verification gate (§52).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gate {
    pub id: GateId,
    pub kind: GateKind,
    pub required: bool,
    pub predicate: GatePredicate,
}

/// Gate evaluation outcome (§52 results).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Pending,
    Passed,
    Failed,
    Blocked,
    Skipped,
}

/// Why a gate is not passing: drives the OMP continue / user escalation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateResult {
    pub status: GateStatus,
    /// Ids of evidence items that decided this evaluation.
    pub deciding: Vec<String>,
    /// Machine-readable reason (`missing 2 test evidence for gate "verify"`).
    pub reason: String,
    /// True when an optional gate passed on unattributed evidence only —
    /// the UI must badge it `unverified-origin` (§53 R2 fix).
    pub unverified_origin: bool,
}

/// Registration-time rejection (§54): predicates that cannot be evaluated
/// purely MUST NOT be registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    /// Predicate needs fresh I/O (run tests, open browser, spawn, RPC).
    /// Produce the evidence via the agent loop first.
    NeedsIo(String),
}

impl Gate {
    /// Register a gate. `needs_io: true` rejects with `GateError::NeedsIo`
    /// — the plan forbids scheduling fetches inside the engine.
    pub fn register(
        id: impl Into<String>,
        kind: GateKind,
        required: bool,
        predicate: GatePredicate,
        needs_io: bool,
    ) -> Result<Self, GateError> {
        if needs_io {
            return Err(GateError::NeedsIo(
                "gate predicate requires fresh I/O; produce evidence first".to_string(),
            ));
        }
        Ok(Self {
            id: id.into(),
            kind,
            required,
            predicate,
        })
    }

    /// Pure evaluation over stored evidence. `items`: ALL evidence in the
    /// workflow (the engine filters by `for_gates`). Required gates ignore
    /// unattributed items; a required gate with ONLY unattributed support is
    /// `Failed` (provenance rejection), never passed.
    pub fn evaluate(&self, items: &[Evidence]) -> GateResult {
        let supporting: Vec<&Evidence> = items
            .iter()
            .filter(|e| e.for_gates.iter().any(|g| g == &self.id))
            .filter(|e| self.predicate.kinds.is_empty() || self.predicate.kinds.contains(&e.kind))
            .collect();
        let attributed: Vec<&Evidence> = supporting
            .iter()
            .filter(|e| e.is_attributed())
            .copied()
            .collect();
        // Required gates read attributed evidence only.
        let usable: &[&Evidence] = if self.required {
            &attributed
        } else {
            &supporting
        };
        let relevant: Vec<&Evidence> = if self.predicate.require_ok {
            usable.iter().filter(|e| e.ok).copied().collect()
        } else {
            usable.to_vec()
        };
        if relevant.len() >= self.predicate.min_items {
            let unverified = !self.required && relevant.iter().all(|e| !e.is_attributed());
            return GateResult {
                status: GateStatus::Passed,
                deciding: relevant.iter().map(|e| e.id.clone()).collect(),
                reason: format!("gate {:?} satisfied", self.id),
                unverified_origin: unverified,
            };
        }
        // Distinguish provenance rejection from plain absence.
        if self.required && !supporting.is_empty() && attributed.is_empty() {
            return GateResult {
                status: GateStatus::Failed,
                deciding: supporting.iter().map(|e| e.id.clone()).collect(),
                reason: format!(
                    "gate {:?} rejects unattributed evidence (reproduce inside a tracked edit)",
                    self.id
                ),
                unverified_origin: false,
            };
        }
        GateResult {
            status: GateStatus::Pending,
            deciding: supporting.iter().map(|e| e.id.clone()).collect(),
            reason: format!(
                "gate {:?} needs {} attributed {} evidence, has {}",
                self.id,
                self.predicate.min_items,
                if self.predicate.require_ok {
                    "passing"
                } else {
                    "observed"
                },
                relevant.len(),
            ),
            unverified_origin: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pred(kinds: Vec<EvidenceKind>, min_items: usize, require_ok: bool) -> GatePredicate {
        GatePredicate {
            kinds,
            min_items,
            require_ok,
        }
    }

    fn gate(id: &str, required: bool, p: GatePredicate) -> Gate {
        Gate::register(id, GateKind::Test, required, p, false).unwrap()
    }

    #[test]
    fn needs_io_rejected_at_registration() {
        let r = Gate::register("g", GateKind::Test, true, pred(vec![], 1, true), true);
        assert!(matches!(r, Err(GateError::NeedsIo(_))));
    }

    #[test]
    fn required_gate_passes_on_attributed_ok() {
        let g = gate("verify", true, pred(vec![EvidenceKind::Test], 1, true));
        let e = Evidence::attributed(
            "e1",
            EvidenceKind::Test,
            &["verify"],
            "14 passed",
            true,
            "t",
            "c",
        );
        let r = g.evaluate(&[e]);
        assert_eq!(r.status, GateStatus::Passed);
        assert!(!r.unverified_origin);
    }

    #[test]
    fn required_gate_rejects_unattributed() {
        let g = gate("verify", true, pred(vec![EvidenceKind::Test], 1, true));
        let e = Evidence::unattributed("e1", EvidenceKind::Test, &["verify"], "trust me", true);
        let r = g.evaluate(&[e]);
        assert_eq!(r.status, GateStatus::Failed);
        assert!(r.reason.contains("unattributed"));
    }

    #[test]
    fn repro_gate_accepts_failing_observation() {
        let g = Gate::register(
            "repro",
            GateKind::Reproduction,
            true,
            pred(vec![EvidenceKind::Command], 1, false),
            false,
        )
        .unwrap();
        let e = Evidence::attributed(
            "e1",
            EvidenceKind::Command,
            &["repro"],
            "repro fails with E0502",
            false,
            "t",
            "c",
        );
        assert_eq!(g.evaluate(&[e]).status, GateStatus::Passed);
    }

    #[test]
    fn optional_gate_flags_unverified_origin() {
        let g = gate("live", false, pred(vec![], 1, true));
        let e = Evidence::unattributed("e1", EvidenceKind::Browser, &["live"], "looks fine", true);
        let r = g.evaluate(&[e]);
        assert_eq!(r.status, GateStatus::Passed);
        assert!(r.unverified_origin);
    }

    #[test]
    fn pending_when_below_min() {
        let g = gate("verify", true, pred(vec![EvidenceKind::Test], 2, true));
        let e = Evidence::attributed(
            "e1",
            EvidenceKind::Test,
            &["verify"],
            "part",
            true,
            "t",
            "c",
        );
        let r = g.evaluate(&[e]);
        assert_eq!(r.status, GateStatus::Pending);
        assert!(r.reason.contains("has 1"));
    }
}
