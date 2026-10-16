# ADR-0063: A running worker's brief is the scope for imported edits (interim)

- **Status:** Accepted (agent, 2026-10-13, interim). The owner was asked how to attribute a write to a brief and asked for a recommendation instead ("แนะนำอันไหน"); this is that recommendation, taken as the interim so ADR-0033 decision 3 can land. Supersede it when the owner picks another, or when OMP names the worktree in its subagent events.
- **Date:** 2026-10-13
- **Superseded by ADR-0085** (2026-10-16): the fork names the directory a subagent works in, so a write is attributed to the worker whose tree holds it; the union below is gone.
- **Rule text:** `crates/cedian_panel/src/review.rs` (`FileReview::rebuild`), `crates/cedian_worker/src/brief.rs` (`Scope`), `crates/cedian_review/src/diff.rs` (`HunkStatus`); ADR-0033 decision 3, ROADMAP S5 exit
- **Builds on:** ADR-0033 (the brief contract), ADR-0050 (a worker is an OMP subagent), ADR-0006 (review provenance and precedence), ADR-0022 (audit rows)

## Context

ADR-0033 decision 3 says a changed file outside the brief's `scope.write`, or inside `scope.deny`, is `OutOfScope`, and that merge-back is refused while an `OutOfScope` hunk is unresolved. It assumes a write can be tied to the brief it violates. It cannot, today: ADR-0050 made a worker an OMP subagent that runs inside the OMP process with the app's task isolation pinned off, so its writes land in the workspace itself, and OMP's subagent events name no worktree (ROADMAP Follow-ups). The three ways to close that:

| | Rule | Cost |
|---|---|---|
| (i) | While a worker is `running`, the union of the running workers' `scope.write` is the turn's scope, and the union of their `scope.deny` is the turn's deny. A write outside all of them is `OutOfScope`. | Coarse with several workers at once, and it cannot tell which worker a hunk belongs to. Available now. |
| (ii) | Scope applies only to files under `.worktrees/<id>` | Correct in spirit, but nothing today writes there (isolation is pinned off), so it would classify nothing. |
| (iii) | Wait for OMP to name the worktree in its subagent events | Correct, and the only one that attributes a hunk to a worker. S5's exit stays open until OMP grows it. |

## Decision

1. **Interim: (i).** At import time the panel reads the registry and the briefs of the workers whose status is `running`, builds one scope from them, and classifies each imported hunk against it. No running worker means no scope and no new status, so a turn without workers behaves exactly as before.
2. **The rule lives with the brief.** `cedian_worker` compiles the globs (`globset`, already a workspace dependency) into a scope that answers "is this worktree-relative path writable"; the panel does not re-implement matching. Several briefs union: a path is in scope when some brief's `scope.write` covers it and no brief's `scope.deny` does.
3. **`HunkStatus::OutOfScope`** joins `cedian_review`'s vocabulary. It is never auto-resolved: accept-all skips it (the reply names how many it skipped), a single accept or reject treats it like `Stale`, and revert-turn keeps it. It blocks merge-back until a person resolves it.
4. **The write is audited** (ADR-0033 decision 3): one row per out-of-scope file, naming the tool call, the path and the worker whose brief it left. It goes through the link, which owns the audit log's handle.
5. **What this does not claim.** A hunk is attributed to a scope, not to a worker; with two workers running, a file one brief allows is not out of scope even if it left the other. That is the interim's price, and (iii) is what removes it.

## Consequences

- S5's exit can be met through the app: an out-of-scope edit shows as `OutOfScope`, blocks merge-back, and needs a person's decision, all of it visible in Review Changes.
- A worker whose brief is narrower than the task's real work now gets in the way: the fix is a wider `scope.write` in the brief, which is the point of the boundary.
- When OMP names the worktree in a subagent event, (iii) replaces this with a rule that attributes each hunk to its own worker, and this ADR is superseded.
- The scope is read per import, so a brief edited mid-turn (which ADR-0033 decision 2 forbids: a new revision is a new brief) cannot silently widen a running worker's boundary.
