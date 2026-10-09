# ADR-0055: Workflow, evidence and review run in the app; findings annotate code as blocks

- **Status:** Accepted (owner, 2026-10-09, in chat: "รัน reviewer จากแอป", "แสดงรายชื่อ ไม่ทำ hunk", "เฉพาะที่ตั้งใน cedian.toml", "U9 ใช้ hash ไปก่อน"; the rest decided by the agent as the safer option)
- **Date:** 2026-10-09
- **Rule text:** `docs/plans/s9-real-app.md` U9; ROADMAP stand-in rows H and I, Follow-ups
- **Builds on:** ADR-0010, ADR-0024, ADR-0026, ADR-0032, ADR-0039, ADR-0041, ADR-0047, ADR-0049

## Context

The workflow store, gates, evidence, claims ledger, correction ledger, review findings and the reviewer runner were built headless in S2 and S3. They all live in `cedian_cli`. The app registers only `cedian_worktree_request`, so a turn in the app has no workflow, no gate and no reviewer. The panel emits hunk events, but nothing writes them to `corrections.jsonl`. A `bash` call's writes are not review hunks. An OMP write that lands before cedian has read the file's disk text imports as Unchanged.

## Decision

1. **One copy of the rigor code.** The workflow store, correction ledger, findings store and reviewer runner move from `cedian_cli` into `cedian_shell`. The app and the CLI both call that copy.
2. **The app runs the workflow.** It registers `cedian_workflow_update`, `cedian_complete` and `cedian_review_request`, and shows the phases, gates, evidence outcomes and the last claims ledger in the panel. A normal turn still has no workflow (ADR-0026). These tools write no workspace files, so they are registered under `policy = "omp"` too.
3. **The reviewer runs from the app** (owner). Its diffs come from the task's Zed buffers (`TaskReview`), not from disk. Findings show on their hunk in Review Changes. A blocker refuses completion, and a Dismiss takes a reason, which is audited.
4. **Findings annotate code as editor blocks**, not as Zed diagnostics. Diagnostics reach OMP's prompt through the U6 context, which would feed the reviewer's words back to the implementer.
5. **Corrections are written in the app.** Rejecting a hunk, editing over an agent hunk and reverting a turn each write their ADR-0032 row.
6. **Bash writes** (ADR-0047). An open buffer that a `bash` call changes becomes that call's hunk. A file that is not open has no text from before the call. It is listed in Review Changes as changed by that call, not reviewable as hunks (owner). A start-of-call snapshot is added only if it is measured within the ADR-0037 budget. When another mutating call overlaps on the same file, nothing is imported for it and the file is shown as such.
7. **The ToolStart race.** When cedian lost the race to read a file's disk text, it uses the `oldText` in OMP's edit and write results. When OMP pruned that text, the file is shown as not reviewable, never as Unchanged.
8. **Browser gate by floor only** (owner). It is required only where the user's `[[workflow.floor]]` in `cedian.toml` asks for it. A capture taken at the end of OMP's `browser` call is evidence attributed to that call.
9. **§54 escalation.** A dialog that times out during an active workflow blocks the current phase and shows the escalation with a Resume button. Outside a workflow, it shows a notice only.
10. **Evidence stays bound to content hashes in U9** (owner). Stand-in H stays, and binding evidence to `clock::Global` is its own step before S9 closes.

## Consequences

- The S2 and S3 exits can be checked through the app.
- `cedian_apply_edit` is not registered in the app, so replays recorded with it are re-recorded or handled in the test that uses them.
- Typing in a buffer without saving does not yet make evidence stale. ROADMAP stand-in H names this.
- `OMP_PARITY.md`: the `tool_execution_end` `details` (`oldText`/`newText`) and `bash` rows change in the changes that build them.
