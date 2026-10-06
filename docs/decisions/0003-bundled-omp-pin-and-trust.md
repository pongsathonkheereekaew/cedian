# ADR-0003: Bundled OMP is pinned and verified as a trust boundary

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §6

## Context

The bundled `omp` inherits the user's uid and sandbox profile at spawn — a silent privilege boundary. A version string alone can't prove which source produced the binary.

## Decision

- `script/build-omp` records `{commit, normalized source-tree hash, builder}` in `vendor/omp-revision.json`; the startup handshake verifies the triple.
- After an OMP revision change, first run shows `OMP runtime updated <old→new> [Review changes] [Continue]`.
- Protocol mismatch and revision change both fail closed — never a silent fallback.

## Consequences

- Auto-update (S9) ships cedian.app + omp as ONE unit and re-verifies the triple.
- Open item: the triple is still `TODO` in the pin file and the handshake does not check it yet.
