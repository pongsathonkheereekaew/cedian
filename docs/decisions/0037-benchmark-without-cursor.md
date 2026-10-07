# ADR-0037: The speed benchmark measures cedian on its own, against fixed budgets

- **Status:** Proposed (awaiting the owner)
- **Date:** 2026-10-07
- **Rule text:** [ROADMAP.md](../ROADMAP.md) S2 exit (Benchmark), S9 exit; `docs/plans/s2-workflow-core.md` (B1–B10)
- **Supersedes in part:** ADR-0026 decision 4, second bullet ("must be lower than Cursor's with the same model"), and decision 5 ("Benchmark against Cursor")

## Context

ADR-0026 says speed is measured, never claimed, and it measures speed by running ten real tasks in both cedian and Cursor with the same model. The owner approved the ten tasks (B1–B10) on 2026-10-07. Preparing the run surfaced three problems with the Cursor arm:

- **No shared model.** OMP routes to whatever the user's OMP config picks (at present `opencode-go/muse-spark-1.3-contributor`). Cursor offers a different model list. "Same model" needs a model both can reach with the owner's auth, and that choice constrains OMP's routing, which cedian does not own (ADR-0027).
- **cedian cannot run the Cursor arm.** Cursor is a separate app. The agent cannot drive it as part of a headless run, so half of every result would depend on manual runs.
- **The owner does not want the comparison.** Asked on 2026-10-07, the owner chose to stop comparing with Cursor.

The underlying rule still holds: a speed claim needs a number behind it.

## Decision

1. **The benchmark runs cedian only.** Each task runs through cedian (`cedian prompt`, or `cedian shell` when a task needs more than one turn) in a fresh worktree at the task's start commit. No Cursor arm.
2. **What is recorded per task:**
   - **time-to-usable-result:** wall time from the prompt until the task's done predicate passes (checked by the harness after the turn, with the task's hidden tests applied);
   - **false-done:** the agent claimed done (a final reply that says so, or an accepted `cedian_complete`) and the predicate fails;
   - **cedian overhead per turn:** turn wall time minus OMP's own time (from the first prompt frame sent to `prompt_result`), split into spawn, context assembly, edit apply and diff rebuild where the CLI can time them;
   - **owner review time:** optional. The owner records it when reviewing the results.
3. **Absolute budgets replace the Cursor comparison.**
   - false-done: 0 across the ten tasks;
   - cedian overhead: under 1 s per turn at the median, and under 3 s for any turn (warm `cedian shell` turns), excluding OMP's time;
   - time-to-usable-result: no budget on the first run. The first run sets the baseline; a later run (S9 exit) is a regression when a task's median is more than 25% slower on the same model.
4. **Method stays the same otherwise.** The same B1–B10 tasks and done predicates. The harness is committed and rerunnable. The model and OMP version are recorded with every result. `benchmark-checklist` is applied before any number is reported.
5. **A miss is an ADR, never a silent claim** (unchanged from ADR-0026).

## Consequences

- The S2 exit's benchmark line reads "10 real tasks, cedian only, against the budgets in ADR-0037" instead of "vs Cursor with the same model". ROADMAP and the S2 plan change to match once this is accepted.
- cedian can no longer claim "faster than Cursor". It can only claim what the budgets and baseline show.
- The overhead budget needs per-turn timing in the CLI (spawn, prompt, `prompt_result`, edit apply). That instrumentation lands with the harness.
- The budget numbers are a first proposal with no measurement behind them yet. If the first run shows they are wrong in either direction, the owner adjusts them in a follow-up ADR before they gate anything.
