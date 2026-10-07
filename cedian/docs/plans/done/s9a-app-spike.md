# S9a — App spike plan

Exit (ROADMAP S9a, verbatim intent): the Zed fork builds as `cedian`; one GPUI panel spawns OMP through the P1 spawn profile and renders a streamed reply from the existing `Thread` model; `Version` ↔ `clock::Global` for one buffer; one OMP-native `edit` write is imported as ONE agent-attributed Zed transaction keyed by `tool_call_id`, and native undo reverts it. No OMP change.

**Timebox:** 2 working sessions. **Partial exit:** commit what is green on the fork branch, mark the rest follow-up in README; never block S2 on polish.

## Where the code lives (spike decision)

- Fork: `~/src/zed`, branch `cedian/s9a-spike` (origin = owner's GitHub fork).
- The fork path-depends on this repo's crates (`../../cedian/crates/cedian_omp`, `cedian_agent`). Cheapest reversible wiring; the final layout (submodule, single repo, or published crates) is an ADR finding of this spike.
- New fork crate `crates/cedian_panel/` (one file per concept: `panel.rs`, `import.rs`). Zed core is touched at exactly two points: `zed/src/zed.rs` `initialize_panels` (add the panel) and `crates/zed/Cargo.toml` (bin name `cedian`, deps).

## Machine constraints (2026-10-07)

8 cores, 8 GB RAM, ~68 GB free disk, Command Line Tools only (no Metal compiler). Build with `gpui_platform/runtime_shaders`, `-j4`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`. If that build fails, stop and report; do not install toolchains unasked.

## Tasks

| # | Task | Done when |
|---|---|---|
| T0 | Cold-build upstream `zed` with runtime shaders | `target/debug/zed` exists and launches a window |
| T1 | Rename bin to `cedian`; add path deps; empty `CedianPanel` registered in `initialize_panels` | the app shows a "cedian" dock panel |
| T2 | Panel spawns `OmpRuntime` (spawn profile, background thread); router events → `cedian_agent::Thread` → rendered rows; one prompt input | a typed prompt streams a reply into the panel (live, manual) |
| T3 | `Version` ↔ `clock::Global` for the active buffer | unit/gpui test: cedian version advances exactly when `buffer.version()` changes |
| T4 | Import: on `tool_execution_end` for `edit`/`write`, resolve the path, `buffer.reload()` → keep `tool_call_id ↔ TransactionId` | gpui test (`TestAppContext`): disk write + recorded `ToolEnd` → one transaction; `undo` restores pre-edit text; a dirty buffer is not overwritten (conflict → `STALE`) |
| T5 | Live end-to-end in the app | manual: OMP `edit` in the panel → editor shows it → ⌘Z reverts it |

## Facts already established (Zed source, fork HEAD a1b71072e5)

- `Buffer::reload` diffs disk vs buffer and applies it as ONE finalized transaction, returned through its receiver; it refuses when the buffer changed since the diff base (`has_conflict = true`). T4 calls it directly on `ToolEnd` instead of waiting for the file watcher, whose later reload is then a no-op.
- `ReplicaId::AGENT` is used for agent *selections* only (`set_agent_selections`), not edits. Edit attribution stays a cedian map (`tool_call_id → TransactionId`) unless the spike finds a better hook (`action_log`).
- Panels register via `Panel::load` + `workspace.add_panel` in `crates/zed/src/zed.rs::initialize_panels`.

## Findings that must become ADRs before S2

Repo layout for the fork ↔ cedian crates; edit attribution mechanism (map vs `ReplicaId::AGENT` vs `action_log`); `Version` → `clock::Global` replacement plan for `cedian_workspace`; build constraints if `runtime_shaders` or memory forced changes.
