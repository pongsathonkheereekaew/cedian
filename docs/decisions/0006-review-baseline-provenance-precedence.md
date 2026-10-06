# ADR-0006: Review: task baseline, cedian-owned provenance, strict hunk precedence

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §12 (Zed types), §16, §17, §18

## Context

Reviewing against git HEAD mixes the user's manual edits into the agent's changes. OMP compaction drops verbatim pre-compaction turns, so attribution can't be reconstructed from the transcript later. Zed already has the right primitives (`clock::Global`, `History`, `buffer_diff`, `acp_thread::diff`).

## Decision

- Baseline = per-task `clock::Global` version vectors, never git HEAD; review is a thin projection over Zed's diff/multibuffer, not a parallel engine.
- Provenance (`AgentEdit`, keyed task + tool call) lives in cedian, snapshotted at `tool_execution_end`, and survives compaction.
- Uncorrelatable edits are `UNATTRIBUTED`: per-hunk resolve allowed, accept-all skips them, no re-attribute button.
- Precedence `INTERRUPTED > UNATTRIBUTED > STALE`; one badge per hunk; no silent demotion. STALE is never auto-rejected.

## Consequences

- cedian review reads OMP's EXPORT view, never the model view.
- Headless implementation (2026-10-06): resolutions keyed by hunk identity and persisted in `.cedian/review.json` with `snapshot_version`.
