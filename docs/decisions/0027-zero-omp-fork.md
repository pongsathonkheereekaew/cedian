# ADR-0027: Zero OMP fork — cedian adapts to OMP, only Zed is forked

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §8, §9, §10, §12, §13, §32, §65; [ROADMAP.md](../ROADMAP.md) "OMP changes", row G, S9a
- **Supersedes in part:** ADR-0023 (row "OMP fork: minimal"), ADR-0022 decision 4, ADR-0025 consequence on OMP-side work, ADR-0004 and ADR-0014 on OMP-side additions

## Context

After ADR-0022 (host tools) and ADR-0025 (playbooks as skills), only two items still assumed TypeScript in an OMP fork:
- routing OMP's native `edit`/`write` into editor buffers (stand-in row G)
- Plan mode

The owner wants cedian to be OMP-native: use OMP exactly as upstream ships it and fork only Zed. A second fork means a second rebase burden for one person, and OMP updates would stop being "bump the pin".

Every remaining item has a no-fork route:
- OMP's edit tools write the filesystem and emit `tool_execution_start/end` with the `tool_call_id`.
- Zed already reloads files changed on disk; that reload can be wrapped as an undoable transaction with agent identity (`clock::ReplicaId::AGENT`, §12).
- Plan is a launch-time option in OMP.
- Host tools accept any name that does not clash with an OMP tool, so `ios` can be a cedian host tool.

## Decision

1. **No OMP fork, ever.** cedian runs the pinned upstream OMP commit (ADR-0003). When a real need for an OMP change appears, open an upstream PR and wait. Until it ships, cedian works around it or does without.
2. **Agent edits: disk write → agent transaction import.**
   - OMP `edit`/`write`/`ast_edit` write files as usual.
   - On `tool_execution_end`, cedian imports each changed file into its Zed buffer as one transaction carrying agent replica identity and the `tool_call_id`.
   - That gives native undo, per-task review and provenance with no OMP change. This is the permanent design, not a stand-in.
3. **Unsaved buffers.** Agent Sync (§13: save dirty buffers before the turn) is permanent, because the overlay-filesystem alternative needs OMP to read cedian buffers. If the user types in a file while the agent writes it, the result is `STALE` (§18), never a silent overwrite.
4. **`ios` is a cedian host tool** registered with `set_host_tools`, not a new OMP tool.
5. **Plan mode, if wanted,** runs as a separate runtime spawned with OMP's launch-time plan options through the spawn profile (ADR-0020). Otherwise it is omitted.
6. Everything else stays as already decided:
   - host tools and `cedian://` (ADR-0022)
   - OMP skills (ADR-0025)
   - the spawn profile (ADR-0020)

## Consequences

- One fork (Zed) to maintain. OMP updates are a pin bump plus `wire.rs` regeneration.
- Agent edits reach disk before review. File watchers and dev servers see them immediately, as in other agentic IDEs. "Stage in buffer, write on accept" is not possible, and that is accepted.
- `cedian_apply_edit` stays as a host tool for cases where the model chooses it, but it is not the main edit path.
- The ROADMAP "OMP-side work" list becomes "OMP changes: none".
