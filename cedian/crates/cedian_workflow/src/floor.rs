//! Gate floor (§46, ADR-0010): the gates cedian policy requires per task
//! kind × risk. OMP may ADD gates; it can never remove or weaken a floor
//! gate. An empty floor is the fast lane (ADR-0026): nothing is required
//! unless OMP or the user starts a workflow.
//!
//! Pure data. The policy source is `cedian.toml` once ROADMAP row E lands;
//! until then the floor is built in code (empty by default) and tests
//! inject one.

use crate::gate::Gate;
use crate::profile::{Risk, TaskKind, TaskProfile};
use serde::{Deserialize, Serialize};

/// Gates required for one task kind at or above a risk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloorRule {
    pub kind: TaskKind,
    pub min_risk: Risk,
    pub gates: Vec<Gate>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateFloor {
    pub rules: Vec<FloorRule>,
}

impl GateFloor {
    /// Floor gates for this task, all forced `required`.
    pub fn gates_for(&self, task: &TaskProfile) -> Vec<Gate> {
        self.rules
            .iter()
            .filter(|r| r.kind == task.kind && task.risk >= r.min_risk)
            .flat_map(|r| r.gates.iter().cloned())
            .map(|mut g| {
                g.required = true;
                g
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{GateKind, GatePredicate};

    fn floor() -> GateFloor {
        let gate = Gate::register(
            "build",
            GateKind::Build,
            false, // forced required by the floor
            GatePredicate {
                kinds: vec![],
                min_items: 1,
                require_ok: true,
                fresh: true,
            },
            false,
        )
        .unwrap();
        GateFloor {
            rules: vec![FloorRule {
                kind: TaskKind::Feature,
                min_risk: Risk::Medium,
                gates: vec![gate],
            }],
        }
    }

    #[test]
    fn floor_applies_by_kind_and_min_risk() {
        let mut task = TaskProfile::new("t", TaskKind::Feature);
        task.risk = Risk::Low;
        assert!(floor().gates_for(&task).is_empty());
        task.risk = Risk::High;
        let gates = floor().gates_for(&task);
        assert_eq!(gates.len(), 1);
        assert!(gates[0].required);
        task.kind = TaskKind::BugFix;
        assert!(floor().gates_for(&task).is_empty());
        assert!(GateFloor::default().gates_for(&task).is_empty());
    }
}
