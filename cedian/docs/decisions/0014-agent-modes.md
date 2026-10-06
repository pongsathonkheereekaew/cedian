# ADR-0014: Agent modes: Normal + Goal; Plan only via OMP addition; no Deep

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §65

## Context

The UI design assumed `normal | plan | deep | goal` switching over RPC.

## Decision

Bind only to what exists upstream: Normal; Goal when `goal.continuationModes ∋ rpc`; Plan only after an OMP-side addition with owner + slice. `Deep` is deleted. A mid-turn switch aborts and starts a new turn.

## Consequences

Recorded as the single MISSING row of the spike capability table (`spike/CAPABILITY_TABLE.md`).
- **Amended by ADR-0027:** Plan needs no OMP addition; if wanted it runs as a separate runtime with OMP's launch-time plan options.

## Original research notes (verbatim from the 2026-10-06 plan)

> **Scrutinize R4 fix (mode parity is measured, not assumed).** `set_mode` is a Phase 0.5 capability-table row (`normal | plan | deep | goal` — add it). If RPC lacks a mode OMP's normal UI has: (a) Phase 2 ships with only the modes RPC supports, (b) the missing mode is an explicit OMP-side addition (§8) with owner + phase, NOT a silent fallback to `normal`. A mode switch mid-turn aborts the turn (same path as `abort`) and starts a new turn under the new mode — never mid-stream reinterpretation.
>
> **Upstream verdict (oh-my-pi, verified 2026-10-06): `set_mode(normal|plan|deep|goal)` DOES NOT EXIST in `RpcCommand` — and `deep`/`normal` are not OMP modes at all.** `--mode` is `text|json|rpc|acp|rpc-ui` (launch transports, `docs/cli-reference.md`); plan+rpc are MUTUALLY EXCLUSIVE at launch (`issues/5380` open, maintainer-gated). `goal` IS RPC-controllable (`get/create/resume/pause/drop`, continuation needs `goal.continuationModes ∋ rpc`) but is not a drop-in for plan/deep. CONSEQUENCE: delete `Deep` from the cedian mode switch NOW (no upstream to bind to); `Plan` ships only via an OMP-side addition (§8) with owner + phase, or Phase 2 cuts to `Normal + Goal`. The mid-turn-abort contract above stands for whichever modes land.
