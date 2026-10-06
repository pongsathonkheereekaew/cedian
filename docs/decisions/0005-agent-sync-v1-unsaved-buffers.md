# ADR-0005: Unsaved buffers: Agent Sync V1 (with an expiry)

- **Status:** Accepted
- **Date:** 2026-10-05
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §13

## Context

If a user has unsaved edits, OMP `read` sees stale disk. An overlay filesystem would solve it properly but is heavy before the review baseline is solid.

## Decision

V1 = Agent Sync ON: save dirty buffers → record versions → start the turn. Crash between save and prompt start loses nothing and never double-applies.

## Consequences

Contradicts §12's "correct unsaved state" goal — tracked tech debt with the workspace crate as owner; replaced by overlay semantics once the `clock::Global` baseline is solid. Open item: the headless `agent_sync` marks saved but does not write to disk yet.
