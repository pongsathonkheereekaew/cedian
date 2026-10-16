//! Side-effect progress for worker liveness (ADR-0033 decision 5).
//!
//! An imported edit or stored evidence item advances `last_progress` on the
//! running workers it belongs to. Commits are already read by
//! [`WorkerHead::is_stuck`]; spawn and completion touch the field directly.

use crate::brief::Brief;
use crate::registry::{Registry, WorkerError, WorkerStatus};
use std::path::Path;

/// Note progress on `worker`, the running worker an imported edit was
/// attributed to (`WorkerScope::attribute`). Whether a row was touched.
pub fn note_import(state: &Path, worker: &str) -> Result<bool, WorkerError> {
    let _lock = Registry::lock(state)?;
    let (mut registry, _) = Registry::open(state)?;
    let running = registry
        .get(worker)
        .is_some_and(|head| head.status == WorkerStatus::Running);
    if running {
        registry.note_progress(worker)?;
        registry.save(state)?;
    }
    Ok(running)
}

/// Note progress on every running worker whose brief lists `gate` in
/// `verify`. Returns how many rows were touched.
pub fn note_evidence(state: &Path, gate: &str) -> Result<usize, WorkerError> {
    let _lock = Registry::lock(state)?;
    let (mut registry, _) = Registry::open(state)?;
    let mut n = 0;
    let ids: Vec<(String, Brief)> = registry
        .all()
        .filter(|h| h.status == WorkerStatus::Running)
        .filter_map(|h| {
            Brief::latest(state, &h.id)
                .ok()
                .map(|brief| (h.id.clone(), brief))
        })
        .collect();
    for (id, brief) in ids {
        if brief.verify.iter().any(|v| v == gate) {
            registry.note_progress(&id)?;
            n += 1;
        }
    }
    if n > 0 {
        registry.save(state)?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WorkerHead;
    use crate::worktree::tests::fixture;

    fn scene(state: &Path, rows: &[(&str, &str, &str)]) {
        let mut registry = Registry::default();
        for (id, write, verify) in rows {
            let brief = Brief {
                goal: "g".into(),
                scope: crate::brief::Scope {
                    write: vec![(*write).into()],
                    deny: vec![],
                },
                acceptance: vec!["a".into()],
                verify: vec![(*verify).into()],
                timebox_min: 15.0,
                context: vec![],
                forbidden: vec![],
            };
            brief.store(state, id, 1).unwrap();
            registry
                .insert(WorkerHead {
                    id: (*id).into(),
                    branch: format!("cedian-worker/{id}"),
                    worktree: format!(".worktrees/{id}"),
                    task_title: "t".into(),
                    kind: "task".into(),
                    status: WorkerStatus::Running,
                    note: String::new(),
                    timebox_min: 15.0,
                    last_progress: Some(1),
                })
                .unwrap();
        }
        registry.save(state).unwrap();
    }

    #[test]
    fn import_advances_only_the_attributed_worker() {
        let repo = fixture();
        let state = repo.join(".cedian-state");
        std::fs::create_dir_all(&state).unwrap();
        scene(
            &state,
            &[("w1", "notes.txt", "v"), ("w2", "notes.txt", "v")],
        );
        assert!(note_import(&state, "w1").unwrap());
        let (reg, _) = Registry::open(&state).unwrap();
        assert!(reg.get("w1").unwrap().last_progress.unwrap() > 1);
        assert_eq!(reg.get("w2").unwrap().last_progress, Some(1));
        assert!(!note_import(&state, "w9").unwrap(), "no such worker");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn evidence_advances_workers_listing_that_verify_item() {
        let repo = fixture();
        let state = repo.join(".cedian-state");
        std::fs::create_dir_all(&state).unwrap();
        scene(
            &state,
            &[
                ("w1", "notes.txt", "cat notes.txt"),
                ("w2", "notes.txt", "cargo test"),
            ],
        );
        assert_eq!(note_evidence(&state, "cat notes.txt").unwrap(), 1);
        let (reg, _) = Registry::open(&state).unwrap();
        assert!(reg.get("w1").unwrap().last_progress.unwrap() > 1);
        assert_eq!(reg.get("w2").unwrap().last_progress, Some(1));
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn done_workers_are_not_touched() {
        let repo = fixture();
        let state = repo.join(".cedian-state");
        std::fs::create_dir_all(&state).unwrap();
        scene(&state, &[("w1", "notes.txt", "v")]);
        {
            let _lock = Registry::lock(&state).unwrap();
            let (mut reg, _) = Registry::open(&state).unwrap();
            reg.set_status("w1", WorkerStatus::Done, String::new())
                .unwrap();
            reg.save(&state).unwrap();
        }
        assert!(!note_import(&state, "w1").unwrap());
        assert_eq!(note_evidence(&state, "v").unwrap(), 0);
        let (reg, _) = Registry::open(&state).unwrap();
        assert_eq!(reg.get("w1").unwrap().last_progress, Some(1));
        let _ = std::fs::remove_dir_all(&repo);
    }
}
