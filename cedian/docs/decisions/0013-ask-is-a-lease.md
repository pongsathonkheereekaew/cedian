# ADR-0013: `ask` is a lease, not a lock

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §63

## Context

A dialog waiting on a dead runtime is a ghost that blocks the user forever.

## Decision

Disconnect or 5-minute timeout resolves a pending `ask` as `Abstain` (logged, system-generated); the turn resumes as `blocked` on reconnect. Strict-wins applies to the answer, not to liveness.

## Consequences

The headless dialog model has `abstain`; the timeout and the answer path back to OMP are still missing.
