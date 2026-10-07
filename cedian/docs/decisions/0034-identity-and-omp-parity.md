# ADR-0034: Identity — a Cursor-style IDE for OMP, with full OMP feature parity

- **Status:** Accepted (owner, 2026-10-07)
- **Date:** 2026-10-07
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) header, §7, §9; [ROADMAP.md](../ROADMAP.md) header, "Cursor workflow coverage", P7, S9 checklist; [OMP_PARITY.md](../OMP_PARITY.md)
- **Refines:** ADR-0023 (identity row); keeps ADR-0001, ADR-0027

## Context

The owner restated what cedian is (2026-10-07):
- **Cursor-like** means Cursor's *workflow and product features*: agent and IDE in one app.
- **cedian must support every OMP feature.** OMP is the harness the owner already uses; nothing it can do should be lost by using it through cedian.
- **Only Zed is forked, and only to fit OMP.** OMP stays upstream (ADR-0027).

The plan already had the pieces (ADR-0001, ADR-0023, ADR-0027) but not the parity commitment. Coverage was tracked only by the Phase 0.5 spike table (`spike/CAPABILITY_TABLE.md`, OMP 18.6.1), which is not maintained. The pinned OMP exposes 65 RPC commands, 31 agent events and 12 UI requests (`vendor/omp-rpc/src/wire.rs`). The headless crates use a handful of them, and nothing says where the rest will surface or whether they will at all.

## Decision

1. **One-line identity**, used verbatim at the top of README, ARCHITECTURE, ROADMAP and AGENTS.md:
   > cedian is a Cursor-style agentic IDE — the same agent + IDE workflow and product features — built for OMP: OMP runs as upstream ships it, every OMP feature gets a native surface, and Zed is forked only as far as OMP needs. OMP decides; cedian executes, renders, and verifies.
2. **Cursor-style is a workflow map, not a clone.** ROADMAP "Cursor workflow coverage" lists each Cursor workflow and the cedian slice that delivers it. A Cursor feature enters the map only when it is a workflow the owner uses. Cursor's own models, cloud runners and extension marketplace are not targets: OMP owns models, ADR-0019 cuts cloud, and Zed's extensions stay as upstream ships them. The ADR-0023 rule stands: "better than Cursor" is claimed only on verification, review rigor, control and safety, and speed is benchmarked (ADR-0026).
3. **OMP feature parity is a commitment with a ledger.**
   - Every OMP capability reachable through RPC, launch options or config has a row in [`docs/OMP_PARITY.md`](../OMP_PARITY.md).
   - Each row has exactly one status: `native` (GPUI surface), `headless` (wired in cedian crates; GPUI surface at S9), `planned Sx`, `gated ADR-xxxx` (off by default for safety, with a named opt-in), or `upstream-blocked` (an upstream PR link).
   - No permanent cuts. A `gated` row must name its opt-in. An OMP feature can be cut only by a new ADR that says why.
   - Transports (`--mode text|json|acp`) are not features. cedian speaks `rpc-ui` only (§88 "ACP for OMP" stands).
4. **The ledger is enforced, not remembered.** A test (ROADMAP P7) fails when a command, agent event or UI request in the vendored `wire.rs` has no row. Every OMP pin bump regenerates `wire.rs`, so new OMP features cannot arrive unnoticed. Tool and config rows are reviewed by hand in the same change as the pin bump.
5. **Zed is forked only as far as OMP needs.** A Zed core patch exists only to give an OMP capability a native surface or to bind a cedian crate (registration points, ADR-0030). Every other Zed feature behaves as upstream ships it. The fork rebases on upstream Zed on a fixed cadence (S9 fork hygiene).

## Consequences

- v0.1 ("fully native", S9) adds two checklist items: every parity row is `native`, `gated` with a working opt-in, or `upstream-blocked`; and the parity test is green on the pinned OMP.
- The parity ledger is the one home for per-feature OMP coverage. Slice status stays in README only.
- `spike/CAPABILITY_TABLE.md` is frozen history. Its rows are carried into the ledger.
- Safety defaults no longer delete OMP features: the approval mode and `computer` become `gated` with per-project opt-ins (ADR-0035).
- Some rows are large (sessions tree, branch/fork, `live`, `btw`). They land in S9 or get their own slice. A row is never left without one.
