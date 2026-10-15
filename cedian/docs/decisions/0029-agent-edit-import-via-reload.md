# ADR-0029: Agent-edit import via `Buffer::reload`; attribution is a cedian map, not `ReplicaId::AGENT`

- **Status:** Accepted
- **Date:** 2026-10-07
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §12, §16, §17, §18
- **Amends (how, not what):** ADR-0027 (agent identity via `clock::ReplicaId::AGENT`), ADR-0006 ("provenance should key off `ReplicaId::AGENT` where possible")
- **Evidence:** S9a spike, fork branch `cedian/s9a-spike`, crate `cedian_panel` (`import.rs`, `version.rs`), 5 gpui tests green

## Context

ADR-0027 says OMP's disk writes become agent transactions "with agent identity (`clock::ReplicaId::AGENT`)". The S9a spike read the Zed source at fork HEAD `a1b71072e5` and built the import. It found:

1. `Buffer::reload` diffs the disk against the buffer and applies the change as **one finalized transaction**, returned through its receiver. When the buffer changed after the diff base, it refuses and sets `has_conflict`.
2. `Buffer::reload` does **not** check whether the buffer is dirty. Called on a buffer with unsaved user typing, it replaces that typing with the disk content.
3. With an empty diff, `reload` still returns the transaction on top of the undo stack (the previous one). Trusting the return value alone misattributes an earlier edit.
4. Zed's file watcher, through `Project` on `BufferEvent::ReloadNeeded`, may reload the buffer between OMP's write and the `tool_execution_end` frame. That transaction is undoable but carries no attribution.
5. `ReplicaId::AGENT` is used only for agent **selections** (`set_agent_selections`), not edits. Local edits always carry the local replica id. Pretending to be another replica would break Zed's CRDT assumptions for a local buffer.

## Decision

1. **Import = reload, driven by cedian at tool end.** When an edit-class OMP tool (`edit`, `write`, `ast_edit`) starts, cedian *marks* every open local buffer: it calls `finalize_last_transaction`, so the agent edit never groups with user typing, and records the version and the top of the undo stack. At `tool_execution_end` cedian calls `reload` itself and does not wait for the watcher.
2. **Exactly one transaction per buffer per tool call.** If the watcher's reload raced ahead, cedian merges its own transaction into the watcher's (`merge_transactions`). Only a version change since the mark counts as an import (fact 3).
3. **Dirty buffers are never reloaded.** They are reported `STALE` (§18), and disk stays authoritative for the agent's write (fact 2).
4. **Attribution is a cedian map** `tool_call_id → (buffer, TransactionId)`, owned by the cedian layer and later feeding provenance (§17). `ReplicaId::AGENT` stays what Zed uses it for: showing the agent's cursor/selection.
5. **Scope:** only buffers open when the tool starts are imported. Files the agent changes that are not open have no buffer and no undo stack. They review from disk diffs, exactly like headless row G, and become buffers normally when opened.
6. **`Version` ↔ `clock::Global`.** Until `cedian_workspace` moves onto Zed types, a per-buffer `VersionMap` issues a cedian `Version(u64)` that advances exactly when the buffer's `clock::Global` changes, and resolves it back for `expected_version` checks. At S9, `cedian_workspace::Version` is replaced by `clock::Global` (ARCHITECTURE §12 already shows that signature) and the map is deleted.

## Consequences

- No OMP change (ADR-0027 holds). Native ⌘Z reverts one OMP edit in one step.
- A user typing in a file during a turn never loses that typing. The agent's write to that file shows as `STALE` in review instead.
- Follow-up: hook the map into `cedian_review` provenance at S9 (`tool_call_id` precision instead of headless turn-level attribution, row G).
- The live check (one real OMP `edit` in the app, then ⌘Z) is the remaining S9a exit item.
