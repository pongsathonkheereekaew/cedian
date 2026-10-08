# ADR-0047: Files an OMP `bash` call changes are agent edits

- **Status:** Accepted (owner, 2026-10-08, in chat: "นับเป็นของ agent")
- **Date:** 2026-10-08
- **Rule text:** `docs/plans/s9-real-app.md` U5 design; ROADMAP Follow-ups
- **Refines:** ADR-0027 decision 2

## Context

ADR-0027 decision 2 imports the files OMP's `edit`, `write` and `ast_edit` change as agent transactions. The S9 U5 review model follows it: those calls name their paths, and each write becomes one undoable buffer transaction credited to its tool call.

OMP also changes files through `bash`: formatters (`cargo fmt`, `prettier`), `sed -i`, code generators, scripts. Those writes reach Zed only through its file watcher. U5 counts that reload as a user edit, so the formatter's changes are not hunks, cannot be rejected or reverted with the turn, and turn any overlapping agent hunk STALE. Formatters run in most turns, so this is the common case, not an edge.

## Decision

1. **A file a `bash` call changes is an agent edit.** cedian imports it as one transaction credited to that call's `tool_call_id`, reviewable, rejectable and revertable with its turn, like an `edit` write.
2. **Attribution is by time window.** `bash` names no paths. A workspace file whose disk content changes between the call's `tool_execution_start` and `tool_execution_end` is that call's write. A buffer with unsaved user edits is never overwritten; it is STALE (ADR-0027 decision 3, §18).
3. **Build in U9.** U5 keeps the conservative behaviour (the reload counts as a user edit, no data lost) until U9 wires it.

## Consequences

- A change made outside OMP during a `bash` call (the user's own terminal, another tool) is credited to the agent. The user can still reject it, and native undo still applies.
- Two edit-capable calls running at once cannot be told apart by time window; their writes are credited to both, and revert-turn refuses a hunk credited to two turns.
- cedian must watch workspace files for the duration of each `bash` call, including files not open in a buffer.
