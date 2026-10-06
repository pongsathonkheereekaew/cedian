# ADR-0022: Host-tool-first — OMP reports to cedian through host tools

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §10, §43, §46, §55; [ROADMAP.md](../ROADMAP.md) "OMP-side work"

## Context

Several slices need OMP to tell cedian something:

- S2: the task profile, playbook and phase transitions, and a "done" claim to check (A1-narrow, ADR-0010)
- S5: a worktree request (ADR-0009)
- S3: review findings

The plan routed these through OMP-side TypeScript additions (§8): workflow events, integrations. Nobody owns that work and no slice schedules it. That is the same "planned but not doable here" trap that hit earlier slices.

The spike proved that the model discovers cedian host tools (mounted under `xd://`) and calls them, and that each call carries the agent's `tool_call_id` (`HostToolContext::tool_call_id`).

## Decision

1. **Default channel.** Anything OMP must tell cedian goes through a cedian host tool the model calls. Initial set:
   - `cedian_workflow_update`: profile kind/risk, playbook, phase transitions. Evidence claims reference earlier `tool_call_id`s.
   - `cedian_complete`: runs `can_complete`. If a required gate is unmet it returns an error listing the missing gates, so the agent keeps working. Bounded by `max_continue` (ADR-0010).
   - `cedian_worktree_request`: cedian creates the worktree and returns its `WorktreeId` (ADR-0009).
   - `cedian_review_finding` (S3): one structured finding per call (§59).
2. **Evidence is checked against the router log.** Evidence submitted through a host tool counts as attributed only when every referenced `tool_call_id` exists in the router log with a successful `ToolEnd`. Anything else is stored `unattributed`, so required gates reject it. The agent cannot self-certify.
3. **Completion is cedian state, not prose.** A turn that ends with "Done" but never passed `cedian_complete` leaves the workflow not complete, and the UI shows the missing gates. Model compliance is not assumed.
4. **OMP-side TypeScript only when a host tool cannot do the job.** Each such item is listed with an owner and a slice in ROADMAP "OMP-side work". Known today:
   - routing OMP's native `edit`/`write` through `cedian_apply_edit` (stand-in row G)
   - Plan mode (ADR-0014)
5. **No name clashes.** Host tool names never shadow OMP tools (§10, ADR-0004). They are checks and requests, not replacements: `todo`, `task` and `goal` stay OMP's.

## Consequences

- S2 and S5 exits no longer depend on forking OMP.
- Host tool descriptions plus the ambient context must tell the model when to call them (e.g. "a cedian workflow is active: call `cedian_complete` before declaring the task done"). Behavior is verified by the fake-omp replay harness (hermetic) and the live lane, never assumed.
- `set_host_tools` replaces the whole set (ADR-0004), so the runtime always registers the complete cedian tool list in one call.
