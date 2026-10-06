# ADR-0010: Workflow split A1-narrow: OMP drives, cedian checks

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §2 (exception note), §46–§56, §61, §62

## Context

The plan said "the workflow layer lives in OMP" while S2 built TaskProfile/Playbook/Gate/Evidence in Rust inside cedian — an apparent "second workflow engine". Options reviewed 2026-10-06:
- **A1** — gate engine stays in cedian as the pure checker.
- **A2** — the whole layer moves to OMP (`packages/coding-agent/src/workflow/`); cedian only renders.

A2 makes the agent its own verifier: the evidence it would check (router log, `AgentEdit` provenance) lives in cedian (ADR-0006), so OMP would either round-trip for it or trust its own claims. It also adds permanent TS in the OMP fork and blocks S3 on OMP-side work.

## Decision

**A1-narrow.**
- OMP: task classification (TaskProfile), playbook choice, phase progression, deciding how to obtain evidence, workflow events.
- cedian: evidence store (provenance-linked), gate evaluation as a pure function with a fixed budget, the completion block (`can_complete`), `max_continue` (default 3) with escalation, and a **gate floor** in `cedian.toml` — OMP may add gates, never remove or weaken floor gates.
- Required gates reject `unattributed` evidence.

## Consequences

- Same trust logic as permissions (ADR-0012): the checked party is not the checker.
- §88 "no second workflow engine" stays true because cedian never schedules, spawns, or gathers.
- `cedian_workflow`'s profile/playbook/phase code is a stand-in (row A) until OMP emits workflow events; no new features there.
