//! `cedian_worker`: parallel workers on isolated git worktrees (S5).
//!
//! Plan refs: §43 R1 (cedian owns worktrees, OMP requests; conflict → STALE,
//! never auto-merge; merge-back needs explicit accept).
//!
//! Phase 16 (mechanism + visualization land together — CLI `list` is the
//! headless visualization). §18 R3 (hunk precedence surfaces STALE through
//! the existing tracker; the registry adds no new hunk states).
//!
//! S9 handoff: this crate is the headless mechanism (registry record +
//! blocking `git` subprocess calls); the GUI binding visualizes `Registry`
//! and stays out of orchestration (§88: OMP is the only orchestrator).
//!
//! One-shot-per-invocation like `browser`: each CLI command shells out to
//! `git worktree`, persists the updated [`Registry`], and exits — no daemon
//! is ever held.

pub mod registry;
pub mod worktree;

pub use registry::{Registry, WorkerError, WorkerHead, WorkerStatus};
pub use worktree::{merge_back, merge_preview, remove, spawn, MergePlan};
