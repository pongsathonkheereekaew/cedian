# ADR-0042: All cedian code moves into the fork; this repo keeps the docs

- **Status:** Accepted (owner, 2026-10-07, in chat)
- **Date:** 2026-10-07
- **Rule text:** `AGENTS.md` "Where things live"; `docs/plans/s9-real-app.md` U1
- **Refines:** ADR-0030 (layout option B)

## Context

ADR-0030 chose option B: the cedian crates move into the Zed fork at S9 start, and this repo keeps "docs, OMP vendoring and the headless CLI until the CLI dies at S9". Doing the move shows that split cannot work:

- `cedian_omp` depends on `vendor/omp-rpc`, so the vendored client has to sit next to it.
- `cedian_cli` and `cedian_fake_omp` depend on most cedian crates, and every replay test runs through them. Left here, they would path-depend into the fork. That is the same two-repo coupling ADR-0030 set out to remove, only reversed.

## Decision

1. Every code artifact moves into the fork (`pongsathonkheereekaew/zed`):
   - `crates/cedian_*` → the fork's `crates/`;
   - `vendor/omp-rpc` and `vendor/omp-revision.json` → the fork's `vendor/`;
   - `script/bench` → the fork's `script/cedian-bench`;
   - `docs/OMP_PARITY.md` → the fork's `cedian/OMP_PARITY.md`. It is checked by a test against the vendored `wire.rs`, and a pin bump must update both in one change (ADR-0034), so it lives beside the pin.
   One workspace, one lockfile, one place to run every test.
2. This repo keeps the docs only: `README.md` (status), `AGENTS.md`, `docs/` (ROADMAP, ARCHITECTURE, ADRs, plans). Its git history stays the record of the code before the move. The move commit in the fork names the source commit.
3. `spike/rpc-spike` does not move. It answered its question in S0; it stays in this repo's history.
4. The headless CLI still dies at S9, as ADR-0030 and ROADMAP say. It lives in the fork until then.

## Consequences

- Building cedian crates now uses the fork's workspace dependencies and lints (for example `serde_json` with `preserve_order`, `toml` 0.9). The move fixes whatever that breaks, test by test.
- The pre-commit check that ran `cargo test --workspace` here becomes a fork-side script that checks the cedian crates.
- "One home per fact" holds: the parity ledger's one home becomes the fork. `AGENTS.md` here points at it.
