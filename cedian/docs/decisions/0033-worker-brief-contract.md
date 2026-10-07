# ADR-0033: A worktree request carries a checked brief; worker liveness is side effects

- **Status:** Accepted (owner, 2026-10-07)
- **Date:** 2026-10-07
- **Rule text:** [ROADMAP.md](../ROADMAP.md) S5 exit; ARCHITECTURE §42, §43 when S5 lands
- **Builds on:** ADR-0009 (mechanism vs policy), ADR-0022 (host tools), ADR-0024 (evidence), ADR-0025 (verification profiles), ADR-0027 (edits imported from disk)

## Context

`cedian_worktree_request` takes `{id, title, kind?, base?}` today (`crates/cedian_worker/src/host_tool.rs`). Nothing says what the worker is for, where it may write, or how its result will be checked.

pstack's Orchestrate playbook, which runs fleets of subagents under one coordinator, rests on three rules:
- "The brief is the product. A vague brief fails quietly, because a worker cannot ask you a question."
- Every brief has GOAL, SCOPE, CONTEXT, ACCEPTANCE, VERIFY, TIMEBOX, FORBIDDEN and REPORT. "Missing fields are a refuse-to-spawn condition."
- Liveness is probed from side effects (commits, pushes, ledger rows). "Transcript mtime is not liveness."

In pstack these are instructions to the coordinating agent. cedian creates the worktree, imports every edit made in it (ADR-0027), and holds the evidence store. That puts cedian in a position to enforce the rules, and to do it without owning orchestration policy (ADR-0009).

## Decision

1. **The request carries a brief, or no worktree is created.**
   - Required: `goal` (one sentence), `scope.write` (one or more path globs), `acceptance` (one or more checkable lines), `verify` (one or more items, each a command or a feature-map id from the project verification profile, ADR-0025), `timebox_min`.
   - Optional: `scope.deny`, `context` (file and PR pointers), `forbidden`.
   - Any missing or empty required field returns an error naming every missing field. As today, a refused request leaves no tree and no registry row.
   - The report shape is fixed by cedian: status, head SHA, evidence ids, deviations. It is not a brief field.
2. **The brief is stored and immutable.** It is written next to the registry row. A change is a new revision appended beside it, never an edit. A steer message does not change the brief. The subagent view shows the current revision.
3. **Scope is checked on every imported edit.**
   - OMP writes files itself (ADR-0027), so cedian cannot prevent a write. It detects one on `tool_execution_end`.
   - A changed file outside `scope.write`, or inside `scope.deny`, gets a new hunk status, `OutOfScope`. Accept-all skips `OutOfScope` hunks, and the write is recorded in the audit log.
   - Merge-back is refused while any `OutOfScope` hunk is unresolved.
   - `cedian_apply_edit` refuses out-of-scope paths before writing.
4. **`verify` items become the worker's required gates.**
   - The worker's `cedian_complete` needs fresh, attributed `pass` evidence for each item (ADR-0024).
   - Merge-back needs a completed worker. `inconclusive` blocks merge-back, as it blocks every other gate.
5. **Liveness is computed from side effects.**
   - `last_progress` is the latest of: a commit on the worker branch, an imported edit, a stored evidence item, or a completion attempt. Streaming text and tool calls without effects do not count.
   - When `timebox_min` has passed with no progress, the worker shows as `stuck`. cedian never stops, retries or replaces a worker on its own, because that is OMP's policy (ADR-0009). It surfaces the state and offers the user a stop.
6. **Not adopted from pstack.** Standing-orders text, model routing, retry modes and wave sizing are orchestration policy. They stay in OMP and in the user's skills.

## Consequences

- S5 exit grows: a brief-less request is refused (fake-omp replay), an out-of-scope edit blocks merge-back, and a timeboxed worker with no side effects shows `stuck`.
- The `cedian worker spawn` CLI verb takes `--brief <file>` with the same schema, so the headless and agent paths share one validator.
- Changing the tool schema changes the recorded P5 fixtures, so they must be re-recorded. Pre-v0.1 there are no external consumers to keep compatible.
- `HunkStatus` gains `OutOfScope`. Review and revert-turn treat it like `Stale`: never auto-resolved.
- The swarm, arena and interrogate presets (S5, Phase 17) inherit this for free: each arm is a worker with a brief, so every arm's result is checkable the same way.
