# ADR-0009: Worktrees: cedian owns the mechanism, OMP owns the policy

- **Status:** Accepted
- **Date:** 2026-10-05
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §42, §43, §90 note

## Context

Both Zed (Project/worktree) and OMP (task isolation) could plausibly create worktrees; two owners means orphaned trees and races.

## Decision

- Zed Project/cedian is the single owner of `git worktree add/remove` and `WorktreeId`. OMP requests via a host tool and keeps orchestration/merge policy.
- Mechanism and visualization land together (slice S5) — neither ships alone.
- User opening a file a worker edits → STALE, never auto-merge; merge-back requires explicit accept.

## Consequences

- Headless `cedian_worker` (stand-in row C) refuses to remove unmerged workers and merges only into the checked-out base.
- Missing for S5: the host-tool request path and subagent-event visualization.
