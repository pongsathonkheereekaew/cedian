# ADR-0008: `computer` goes through the CUA `cua-driver` contract only

- **Status:** Accepted
- **Date:** 2026-10-05
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §9 (`computer` row), §38

## Context

Desktop automation is the most dangerous capability: raw OS input outside a typed contract can't be audited or sandboxed.

## Decision

- `computer` = CUA `cua-driver` (MIT, Rust) behind OMP tool semantics: default-deny, per-action `ask`, audit-logged.
- Driver-only first; Lume/Spaces VM sandbox deferred.
- Driver + Seatbelt profile + bypass-proof test land ATOMICALLY; until then `computer` is hard-disabled, including as a fallback.
- Pin `cua-driver` by git rev + cargo vendor on a fixed cadence alongside the OMP pin.

## Consequences

Resumed turns and automation runs need a fresh `Ask` for actuation (grants don't survive restart).

- **Amended by ADR-0020 (how):** `computer` is an `eval` prelude in OMP; it is disabled via `computer.enabled: false` in the cedian spawn overlay, not via a tool allow-list.
- **Superseded in part by ADR-0035:** "hard-disabled" is the default. A project can opt in to OMP's own `computer` prelude (`computer = "omp"`); the CUA driver stays the default target once it lands.
