# ADR-0017: Plan by vertical slices, with an exit-criterion rule and registered stand-ins

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) ROADMAP.md (method, stand-ins)

## Context

Phase numbering produced layers nobody could try. A review on 2026-10-06 also found ✅ claims weaker than their exits, and headless stand-ins that silently contradicted the target architecture.

## Decision

- Schedule by slices S0–S9 (ROADMAP); phases survive only as scope blocks.
- Exit criteria name who acts and what is observed; ✅ requires an OMP turn or hermetic replay, else ◐ with the gap named; status lives only in README.
- Every pre-S9 stand-in needs a row (owner, deviation, fate) in ROADMAP; unregistered stand-ins violate §88.

## Consequences

The plan was split (2026-10-06) into ARCHITECTURE / decisions / ROADMAP / README so that each fact has exactly one home.

## Original research notes (verbatim from the 2026-10-06 plan)

> **Senior-dev correction (2026-10-06).** Phases below stay as the work-breakdown reference; SCHEDULING follows slices. Each slice is a thin vertical cut ending triable in `cedian` CLI (or the app once the fork lands). Rule: never start a slice whose exit criterion can't be exercised in this repo — integration risk first, never accumulate untested layers.
