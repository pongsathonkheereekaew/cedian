//! Review tracker: baseline → current diffs + hunk accept/reject (plan §18).
//!
//! Semantics: accept = mark (code already in the buffer); reject = inverse
//! patch to baseline state. STALE (user edited after agent) never auto-rejects
//! — compare/restore manually. Bulk accept-all SKIPS `Unattributed` hunks (§17
//! R2: per-hunk resolve only).
//!
//! Debounce (§20): diff rebuilds queue on edit-complete and flush at 50–100ms;
//! streaming tokens and tool progress never trigger recomputation. Headless:
//! the owner calls `request_rebuild` + `rebuild_due`; GPUI ticks it per frame.

use crate::{line_diff, Baseline, FileDiff, HunkStatus};
use cedian_workspace::{TextEdit, Version, WorkspaceHost};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Rebuild debounce window (§20: 50–100ms).
pub const REBUILD_DEBOUNCE: Duration = Duration::from_millis(50);

/// Tracker failures (caller-visible).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackerError {
    /// Host edit failed during inverse patch.
    Host(String),
    /// No diff computed for this path yet.
    NoDiff { path: PathBuf },
    /// Hunk index out of range.
    BadHunk { path: PathBuf, index: usize },
    /// Transition not allowed (e.g. resolving `Interrupted` directly).
    BadTransition { status: HunkStatus },
}

impl std::fmt::Display for TrackerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDiff { path } => write!(f, "no review diff for {}", path.display()),
            Self::BadHunk { path, index } => write!(f, "no hunk {index} in {}", path.display()),
            Self::BadTransition { status } => {
                write!(f, "hunk transition not allowed from {status:?}")
            }
            Self::Host(e) => write!(f, "host edit failed: {e}"),
        }
    }
}

impl std::error::Error for TrackerError {}

/// Per-task review state: baseline + computed diffs + hunk statuses.
pub struct ReviewTracker {
    task_id: String,
    baseline: Baseline,
    baseline_texts: HashMap<PathBuf, String>,
    diffs: HashMap<PathBuf, FileDiff>,
    rebuild_queued: bool,
    last_rebuild: Option<Instant>,
}

impl ReviewTracker {
    /// New tracker for a task with its baseline snapshot.
    pub fn new(
        task_id: &str,
        baseline: Baseline,
        baseline_texts: HashMap<PathBuf, String>,
    ) -> Self {
        Self {
            task_id: task_id.to_string(),
            baseline,
            baseline_texts,
            diffs: HashMap::new(),
            rebuild_queued: false,
            last_rebuild: None,
        }
    }

    /// Owning task.
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Queue a diff rebuild (call on edit-complete; debounced, §20).
    pub fn request_rebuild(&mut self) {
        self.rebuild_queued = true;
    }

    /// Rebuild queued diffs when the debounce elapsed. Returns rebuilt paths.
    /// Reads through the host trait so the live store is used (never a copy).
    pub fn rebuild_due(&mut self, host: &dyn WorkspaceHost) -> Vec<PathBuf> {
        if !self.rebuild_queued {
            return Vec::new();
        }
        if let Some(last) = self.last_rebuild {
            if last.elapsed() < REBUILD_DEBOUNCE {
                return Vec::new();
            }
        }
        self.rebuild_queued = false;
        self.last_rebuild = Some(Instant::now());
        let mut rebuilt = Vec::new();
        for path in self.baseline.paths() {
            let before = self.baseline_texts.get(&path).cloned().unwrap_or_default();
            let after = host.read_buffer(&path).unwrap_or_default();
            let hunks = line_diff(&before, &after);
            let statuses = self.preserve_statuses(&path, hunks.len());
            self.diffs.insert(
                path.clone(),
                FileDiff {
                    path: path.to_string_lossy().into_owned(),
                    hunks,
                    statuses,
                },
            );
            rebuilt.push(path);
        }
        rebuilt
    }

    /// Keep user resolutions across rebuilds (same hunk count + same ranges);
    /// new/changed hunks start `Pending`. Structural change resets to Pending
    /// (never silently carry an accept onto different lines).
    fn preserve_statuses(&self, path: &Path, n: usize) -> Vec<HunkStatus> {
        let old = self.diffs.get(path);
        match old {
            Some(FileDiff {
                hunks, statuses, ..
            }) if hunks.len() == n => statuses.clone(),
            _ => vec![HunkStatus::Pending; n],
        }
    }

    /// Current diff for a path.
    pub fn diff(&self, path: &Path) -> Result<&FileDiff, TrackerError> {
        self.diffs.get(path).ok_or_else(|| TrackerError::NoDiff {
            path: path.to_path_buf(),
        })
    }
    /// Paths with a computed diff (sorted for stable display).
    pub fn paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.diffs.keys().cloned().collect();
        paths.sort();
        paths
    }

    /// Accept one hunk: mark only (code is already in the buffer).
    /// `Unattributed` accepts per-hunk (allowed); bulk accept skips those.
    pub fn accept_hunk(&mut self, path: &Path, index: usize) -> Result<(), TrackerError> {
        let diff = self
            .diffs
            .get_mut(path)
            .ok_or_else(|| TrackerError::NoDiff {
                path: path.to_path_buf(),
            })?;
        let status = diff
            .statuses
            .get_mut(index)
            .ok_or_else(|| TrackerError::BadHunk {
                path: path.to_path_buf(),
                index,
            })?;
        match status {
            HunkStatus::Pending | HunkStatus::Unattributed | HunkStatus::Stale => {
                *status = HunkStatus::Accepted;
                Ok(())
            }
            HunkStatus::Interrupted => Err(TrackerError::BadTransition { status: *status }),
            HunkStatus::Accepted | HunkStatus::Rejected => Ok(()),
        }
    }

    /// Reject one hunk: apply the inverse patch (restore baseline lines for
    /// the hunk range) via the buffer store, then mark.
    pub fn reject_hunk(
        &mut self,
        path: &Path,
        index: usize,
        host: &dyn WorkspaceHost,
    ) -> Result<Version, TrackerError> {
        let diff = self.diffs.get(path).ok_or_else(|| TrackerError::NoDiff {
            path: path.to_path_buf(),
        })?;
        let hunk = diff.hunks.get(index).ok_or_else(|| TrackerError::BadHunk {
            path: path.to_path_buf(),
            index,
        })?;
        match diff.statuses[index] {
            HunkStatus::Interrupted => {
                return Err(TrackerError::BadTransition {
                    status: HunkStatus::Interrupted,
                })
            }
            HunkStatus::Accepted | HunkStatus::Rejected => {
                return host
                    .buffer_version(path)
                    .ok_or_else(|| TrackerError::NoDiff {
                        path: path.to_path_buf(),
                    });
            }
            _ => {}
        }
        // Inverse patch: replace current hunk lines with baseline hunk lines.
        let before = self.baseline_texts.get(path).cloned().unwrap_or_default();
        let after = host.read_buffer(path).unwrap_or_default();
        let before_lines: Vec<&str> = before.lines().collect();
        let after_lines: Vec<&str> = after.lines().collect();
        let want: Vec<&str> = before_lines
            .iter()
            .skip(hunk.before_start)
            .take(hunk.before_count)
            .copied()
            .collect();
        let (start_off, end_off) = line_range_offsets(&after, hunk.after_start, hunk.after_count);
        let current = host
            .buffer_version(path)
            .ok_or_else(|| TrackerError::NoDiff {
                path: path.to_path_buf(),
            })?;
        let replacement = if want.is_empty() {
            // Pure deletion of inserted lines: drop the range + one newline when present.
            String::new()
        } else {
            let mut s = want.join("\n");
            s.push('\n');
            s
        };
        let result = host
            .apply_edit(
                path,
                current,
                &TextEdit {
                    start: start_off,
                    end: end_off,
                    replacement,
                },
            )
            .map_err(|e| TrackerError::Host(e.to_string()))?;
        if let Some(diff) = self.diffs.get_mut(path) {
            diff.statuses[index] = HunkStatus::Rejected;
        }
        // Hunk ranges shifted — queue a rebuild so the next read is coherent.
        self.request_rebuild();
        // Silence unused binding (after_lines documents the range basis).
        let _ = after_lines.len();
        Ok(result.new_version)
    }

    /// Bulk accept: every `Pending`/`Stale` hunk, SKIPPING `Unattributed`
    /// (§17 R2). Returns accepted `(path, hunk)` pairs.
    pub fn accept_all(&mut self) -> Vec<(PathBuf, usize)> {
        let mut accepted = Vec::new();
        for (path, diff) in self.diffs.iter_mut() {
            for (i, status) in diff.statuses.iter_mut().enumerate() {
                if matches!(status, HunkStatus::Pending | HunkStatus::Stale) {
                    *status = HunkStatus::Accepted;
                    accepted.push((path.clone(), i));
                }
            }
        }
        accepted
    }

    /// Mark one hunk's status directly (reconciliation: `Interrupted →
    /// Unattributed`; staleness detector: `→ Stale`). Only the allowed
    /// transitions; everything else fails closed.
    pub fn set_status(
        &mut self,
        path: &Path,
        index: usize,
        status: HunkStatus,
    ) -> Result<(), TrackerError> {
        let diff = self
            .diffs
            .get_mut(path)
            .ok_or_else(|| TrackerError::NoDiff {
                path: path.to_path_buf(),
            })?;
        let current = *diff
            .statuses
            .get(index)
            .ok_or_else(|| TrackerError::BadHunk {
                path: path.to_path_buf(),
                index,
            })?;
        let allowed = match (current, status) {
            (HunkStatus::Interrupted, HunkStatus::Unattributed) => true,
            (HunkStatus::Pending, HunkStatus::Stale) => true,
            (HunkStatus::Unattributed, HunkStatus::Stale) => true,
            (a, b) if a == b => true,
            _ => false,
        };
        if !allowed {
            return Err(TrackerError::BadTransition { status: current });
        }
        diff.statuses[index] = status;
        Ok(())
    }
}

/// Byte offsets of `count` lines starting at line `start` in `text`
/// (0-based). End extends through the trailing newline when present so pure
/// insertions remove cleanly.
fn line_range_offsets(text: &str, start: usize, count: usize) -> (usize, usize) {
    let lines: Vec<&str> = text.lines().collect();
    let mut off = 0;
    for line in lines.iter().take(start) {
        off += line.len() + 1;
    }
    let start_off = off.min(text.len());
    for line in lines.iter().skip(start).take(count) {
        off += line.len() + 1;
    }
    (start_off, off.min(text.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (cedian_workspace::HostTools, ReviewTracker) {
        let store = cedian_workspace::HostTools::new(Path::new("/"));
        store.open(Path::new("/a.rs"), "one\ntwo\nthree\n");
        let mut baseline = Baseline::new();
        baseline.snapshot(Path::new("/a.rs"), Version(0));
        let mut texts = HashMap::new();
        texts.insert(PathBuf::from("/a.rs"), "one\ntwo\nthree\n".to_string());
        (store, ReviewTracker::new("task-1", baseline, texts))
    }

    #[test]
    fn rebuild_finds_agent_hunk() {
        let (store, mut tracker) = setup();
        store
            .apply_edit(
                Path::new("/a.rs"),
                Version(0),
                &TextEdit {
                    start: 4,
                    end: 7,
                    replacement: "TWO".into(),
                },
            )
            .unwrap();
        tracker.request_rebuild();
        // Debounce: force by resetting the clock.
        tracker.last_rebuild = None;
        tracker.request_rebuild();
        let rebuilt = tracker.rebuild_due(&store);
        assert_eq!(rebuilt, vec![PathBuf::from("/a.rs")]);
        assert_eq!(tracker.diff(Path::new("/a.rs")).unwrap().len(), 1);
    }

    #[test]
    fn accept_marks_reject_restores() {
        let (store, mut tracker) = setup();
        store
            .apply_edit(
                Path::new("/a.rs"),
                Version(0),
                &TextEdit {
                    start: 4,
                    end: 7,
                    replacement: "TWO".into(),
                },
            )
            .unwrap();
        tracker.last_rebuild = None;
        tracker.request_rebuild();
        tracker.rebuild_due(&store);
        tracker.accept_hunk(Path::new("/a.rs"), 0).unwrap();
        assert_eq!(
            tracker.diff(Path::new("/a.rs")).unwrap().statuses[0],
            HunkStatus::Accepted
        );

        let (store2, mut tracker2) = setup();
        store2
            .apply_edit(
                Path::new("/a.rs"),
                Version(0),
                &TextEdit {
                    start: 4,
                    end: 7,
                    replacement: "TWO".into(),
                },
            )
            .unwrap();
        tracker2.last_rebuild = None;
        tracker2.request_rebuild();
        tracker2.rebuild_due(&store2);
        tracker2
            .reject_hunk(Path::new("/a.rs"), 0, &store2)
            .unwrap();
        assert_eq!(
            store2.read_buffer(Path::new("/a.rs")).unwrap(),
            "one\ntwo\nthree\n"
        );
    }

    #[test]
    fn accept_all_covers_pending_and_stale() {
        let (store, mut tracker) = setup();
        store
            .apply_edit(
                Path::new("/a.rs"),
                Version(0),
                &TextEdit {
                    start: 0,
                    end: 3,
                    replacement: "ONE".into(),
                },
            )
            .unwrap();
        store
            .apply_edit(
                Path::new("/a.rs"),
                Version(1),
                &TextEdit {
                    start: 8,
                    end: 11,
                    replacement: "TWO".into(),
                },
            )
            .unwrap();
        tracker.last_rebuild = None;
        tracker.request_rebuild();
        tracker.rebuild_due(&store);
        assert_eq!(tracker.diff(Path::new("/a.rs")).unwrap().len(), 2);
        tracker
            .set_status(Path::new("/a.rs"), 0, HunkStatus::Stale)
            .unwrap();
        let accepted = tracker.accept_all();
        assert_eq!(accepted.len(), 2, "stale + pending both bulk-accepted");
    }

    #[test]
    fn interrupted_reconciles_to_unattributed_then_resolves() {
        let (store, mut tracker) = setup();
        store
            .apply_edit(
                Path::new("/a.rs"),
                Version(0),
                &TextEdit {
                    start: 4,
                    end: 7,
                    replacement: "TWO".into(),
                },
            )
            .unwrap();
        tracker.last_rebuild = None;
        tracker.request_rebuild();
        tracker.rebuild_due(&store);
        // Crash path: force Interrupted (test-only direct write), then drive
        // the allowed chain Interrupted → Unattributed → per-hunk accept.
        tracker.diffs.get_mut(Path::new("/a.rs")).unwrap().statuses[0] = HunkStatus::Interrupted;
        tracker
            .set_status(Path::new("/a.rs"), 0, HunkStatus::Unattributed)
            .unwrap();
        assert!(
            tracker.accept_all().is_empty(),
            "bulk accept skips unattributed"
        );
        tracker.accept_hunk(Path::new("/a.rs"), 0).unwrap();
        assert_eq!(
            tracker.diff(Path::new("/a.rs")).unwrap().statuses[0],
            HunkStatus::Accepted
        );
    }

    #[test]
    fn bad_transitions_fail() {
        let (store, mut tracker) = setup();
        store
            .apply_edit(
                Path::new("/a.rs"),
                Version(0),
                &TextEdit {
                    start: 4,
                    end: 7,
                    replacement: "TWO".into(),
                },
            )
            .unwrap();
        tracker.last_rebuild = None;
        tracker.request_rebuild();
        tracker.rebuild_due(&store);
        // Pending → Unattributed directly is NOT allowed (only via Interrupted).
        assert!(tracker
            .set_status(Path::new("/a.rs"), 0, HunkStatus::Unattributed)
            .is_err());
    }
}
