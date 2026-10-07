# ADR-0030: Zed fork layout and build profile

- **Status:** Accepted (owner chose option B, 2026-10-07)
- **Date:** 2026-10-07
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §7
- **Evidence:** S9a spike, fork `~/src/zed` branch `cedian/s9a-spike`

## Context

S9a had to connect two repositories: the Zed fork (`pongsathonkheereekaew/zed`) and this repo, which holds the headless crates. For the spike, the fork's `crates/cedian_panel` path-depends on `../../../../cedian/crates/*`, so the build works only with a sibling checkout at `~/cedian`. Cargo resolves those crates' `workspace = true` keys against *this* repo's workspace, which works today but couples the two `Cargo.lock` files loosely.

Build facts on the dev machine (8 cores, 8 GB RAM, Command Line Tools only, no Metal compiler):

- `--features gpui_platform/runtime_shaders` builds GPUI without Xcode. Shaders compile at runtime.
- A cold `-j4` debug build with `CARGO_PROFILE_DEV_DEBUG=line-tables-only` finished. `target/` reached about 39 GB after one cold build plus the `paths` rebuild, and the binary is 1.1 GB.
- Zed's fork guard requires `paths::APP_NAME` to match the binary name. Setting `APP_NAME = "cedian"` also gives cedian its own config/data dirs, separate from Zed.

## Decision (build profile, adopted)

1. The binary is `cedian`, with `APP_NAME = "cedian"` and `default-run = "cedian"`.
2. Dev builds on this machine use `-j4`, `line-tables-only` debug info and `runtime_shaders`. A release `.app` bundle with precompiled shaders needs full Xcode; that is an S9 packaging task, not a dev requirement.
3. Zed core is touched only at registration points (`initialize_panels`, `main.rs` init, the manifests). cedian logic lives in `cedian_*` crates (§7).

## Layout decision: option B

The cedian crates move into the fork at S9 start: one workspace, one lockfile, one CI. Until then, option A (sibling path deps) stays, so the headless slices (S2–S5) keep their fast `cargo test` here. At the move, this repo keeps docs, OMP vendoring and the headless CLI until the CLI dies at S9.

### Options considered

| Option | How | For | Against |
|---|---|---|---|
| A. Sibling path deps (spike today) | fork crates depend on `../cedian/crates/*` | zero setup, both repos stay as they are | builds only with a fixed checkout layout; CI needs both repos; two lockfiles |
| **B. cedian crates move into the fork** (chosen) | `crates/cedian_*` live in the fork; this repo keeps docs, OMP vendoring and the headless CLI until it dies at S9 | one workspace, one lockfile, one CI; matches §7 "cedian-owned crates" inside the fork | the move is a one-time migration; upstream rebases see more files (they are all new crates, so no conflicts) |
| C. Fork as a git submodule of this repo | this repo pins the fork commit | one entry point | submodule friction; still two workspaces |
- **Refined by ADR-0042:** all code moves into the fork, the CLI, the OMP vendoring and `OMP_PARITY.md` included; this repo keeps the docs.
