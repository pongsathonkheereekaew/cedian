# ADR-0016: Crash, persistence and resume

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §74, §75, §76

## Context

Crashes mid-stream can leave half tool calls; quits leave running turns; persisted state drifts across versions. Studied herdr (Rust agent runtime) and Synara (quit dialog + guarded resume). `pstack` (Cursor plugin) was checked and contributed nothing implementable.

## Decision

- Reconnect discards partial text buffers, marks orphaned tool calls `INTERRUPTED` (their edits `UNATTRIBUTED`), and resets the interrupted turn's `max_continue`.
- Tri-state task status, validated respawn argv, `snapshot_version` on every store (herdr).
- Quit dialog lists running tasks with a persisted "resume on next launch"; resume is always BLOCKED behind one click; `computer` grants never survive restart (Synara pattern).
- Composer drafts persist per task (debounced, versioned) and die with the task.

## Consequences

Daemon-style survival across reboot is deferred (ADR-0019).

## Original research notes (verbatim from the 2026-10-06 plan)

> **Scrutinize R3 fix (crash-during-stream).** `open_session` restores the transcript, NOT the in-flight stream. On reconnect cedian MUST: (1) discard the partial `TextDeltaBuffer` (never render half a tool call as complete), (2) reconcile: any `tool.started` without matching `tool.completed` in the EventRouter log is marked `INTERRUPTED` and its partial edits become `UNATTRIBUTED` (§17) until re-driven, (3) reset `max_continue` counters tied to the interrupted turn (§54) — a crash must not consume the agent's retries. Covered by Agent Lifecycle Tests (`runtime crash`, `runtime reconnect`).
>
> **Non-finding (cursor/plugins `pstack`, checked 2026-10-06).** `pstack` is a prompt-only Cursor plugin (poteto-mode playbooks, no daemon/supervisor/sandbox code) — contributes NO implementable mechanism for §§74–76 and is deliberately NOT cited as evidence anywhere in this plan. Its only transferable idea (checkpoint-on-`wip`-commit + off-context resume note) is already covered by `open_session` directory-adopt + `fork`/`branch` above.

> **Lessons from `herdrdev/herdr` (Rust agent runtime, 42k⭐, studied 2026-10-06).** Adopt three mechanisms, adapted to cedian's GPUI (not terminal) surface:
>
> 1. **Pane status as first-class signal.** herdr marks every pane `working | blocked | idle` and pushes attention when an agent stops needing an answer (`agent_view_eval.rs`: `status()`, `attention()` seq per entry). cedian's subagent tree (§42) + tool cards (§66) adopt the tri-state, derived from EventRouter state (running turn → `working`, pending ask/abstain → `blocked`, else `idle`); a `blocked` task surfaces in the panel WITHOUT opening it, never via polling.
> **Ponytail cut (2026-10-06): no attention counter in v1.** herdr's monotonic attention-seq is a cross-process ordering mechanism — unnecessary single-process. A bool `needs_attention` reset on task-open suffices; upgrade when multi-window needs cross-window ordering.
> 2. **Agent resume as validated argv, not free text.** herdr's `agent_resume.rs` constrains resume commands (bare command name, ≤64 args, ≤8KiB, no control chars/apostrophes, cwd part of the dedupe identity). cedian's §74 respawn adopts the same discipline: the respawned `omp --mode rpc-ui` command line is constructed from a validated struct, never string-concatenated; `{binary_path, session_dir, cwd}` is the dedupe key so two workspaces never share a runtime by accident.
> 3. **Versioned snapshots.** herdr's `persist/snapshot.rs` carries `SNAPSHOT_VERSION` and rejects incompatible restores. cedian's `workspace ↔ session ↔ task` mapping store (§75) + review baseline (§16) carry a `snapshot_version: u32` from day one — old state fails closed with "state too old, re-baseline" instead of silently misreading.

> **Market pattern (Synara quit-dialog + guarded resume, verified 2026-10-06).** Synara lists running chats on quit (Cancel / Quit + persistent "Resume chats automatically" checkbox) and resumes ELIGIBLY on relaunch — skipping completed/archived/newer-work/dead-project chats, with a bounded quit-wait that falls back to plain interruption if the intent can't be recorded. cedian adopts the same shape, adapted to one harness:
