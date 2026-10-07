# ADR-0026: Fast lane — proportional rigor, inline edit, revert turn, measured speed

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §15, §18, §62, §73; [ROADMAP.md](../ROADMAP.md) P6, S2 and S9 exits
- **Refines:** ADR-0023 (product scope)
- **Superseded in part by [ADR-0037](0037-benchmark-without-cursor.md):** decision 4's Cursor comparison for cedian overhead and decision 5 (benchmark against Cursor). The benchmark runs cedian alone against fixed budgets.

## Context

The owner's goal is a Cursor-like workflow (agentic + IDE) that is **better and faster**. The plan so far made cedian *better* through verification: gates, evidence, claims (ADR-0010, ADR-0024). Nothing stopped that rigor from becoming a tax on every turn, so cedian risked being better but slower for everyday small edits.

Two core Cursor-style interactions were also missing from the plan:
- **Inline edit:** select code, give an instruction, get the diff in place.
- **Turn-level revert:** undo everything one turn did in one action.

## Decision

1. **Proportional rigor.**
   - A normal turn is as light as Cursor or lighter. Edits land immediately as reviewable hunks, review can happen later, and there is no workflow and no gate.
   - Workflow and gates engage only when the gate floor in `cedian.toml` requires them for the task kind × risk, or when the user asks (`/verify`, a playbook).
   - Trivial and small tasks (§62) never pass through gates unless the user asks.
2. **Inline edit.**
   - Flow: select code → ⌘K → instruction → one OMP turn with the selection as its explicit target → the diff appears in the editor → accept or reject in place.
   - Uses OMP's fast model role (`smol`) by default; the user can override per edit.
   - Same provenance and review rules as any turn.
   - Headless form (before S9): `cedian shell` command `edit <path> <range> <instruction>`.
3. **Revert turn.**
   - One action rejects every hunk the turn produced, found through provenance (`AgentEdit` records grouped by turn).
   - Hunks the user changed afterwards (`STALE`) are skipped and listed. Revert never overwrites user work.
   - Revert is itself undoable.
4. **Speed is measured, not claimed.**
   - **UI:** typing, scrolling and diff rendering never block while an agent streams (§72–§73); measured as frame time in the S9 app.
   - **cedian overhead:** time cedian adds on top of model time (spawn, context assembly, edit apply, diff rebuild) is measured per turn and must be lower than Cursor's with the same model.
   - **Warm runtime:** the OMP session stays alive in `cedian shell` or the app; no spawn per prompt.
5. **Benchmark against Cursor.**
   - Ten real tasks, same model, run in both tools.
   - Record time-to-usable-result, false-done count (claimed done but not done), and the owner's review time.
   - Run at the S2 exit (verification lane) and the S9 exit (whole app).

## Consequences

- "Better" (verification) and "faster" (light default lane, native UI, low overhead) stop competing: rigor scales with risk instead of applying everywhere.
- New prerequisite P6 (revert turn + headless inline edit) in ROADMAP, built on the review crate and `cedian shell`.
- If the benchmark loses on a measured axis, that becomes an ADR with a fix or an explicit acceptance — never a silent claim.
