# S9 — The real app plan

Exit (ROADMAP S9 is the contract): `cargo run -p cedian -- …` → onboard → prompt → answer with zero terminal; every non-extension item of the "fully native" checklist holds; ⌘K inline edit and revert turn work in the editor; the ADR-0026 benchmark is re-run on the app (frame time while streaming, per-turn overhead against ADR-0037, the 10 tasks against the S2 baseline). S9 also closes S4 and S5 (ADR-0038) and deletes stand-ins B, D, G, H and I.

**Where the work happens.** From U1 on, all code lives in the fork (`~/src/zed`, `pongsathonkheereekaew/zed`, branch `cedian/s9`), per [ADR-0042](../decisions/0042-all-code-moves-into-the-fork.md). This repo keeps the docs; status still lives only in README here.

**Machine (2026-10-07).** 8 cores, 8 GB RAM, Command Line Tools only (no Metal compiler, so `runtime_shaders`), 31 GB free disk after clearing the benchmark worktrees; the fork's `target/` is 20 GB warm. A cold app build needed about 39 GB in S9a, so builds stay warm and `-j4` (ADR-0030).

**Timebox.** 20 working sessions in total. Each group below is its own partial exit: commit what is green, README names the rest. A unit that needs the owner (signing identity, full Xcode, CI secrets) stops at that point and is listed, never worked around.

## Units (dependency order)

| # | Unit | Check (done when) | Box |
|---|---|---|---|
| **A. Foundation** | | | |
| U1 | Move all code into the fork (ADR-0042): crates, `vendor/`, bench, `OMP_PARITY.md`; workspace members; dependency and lint fixes; fork-side check script | in the fork, every cedian crate's tests pass (`script/cedian-check`), the replays included; this repo builds nothing and AGENTS.md points at the fork | 1 |
| U2 | Fork hygiene: rebase `cedian/s9` onto current upstream `main`; record cadence and conflict owner; GPL audit note for a redistributed binary | the app builds after the rebase; docs name the cadence and the owner | 1 |
| U3 | App shell: `cargo run -p cedian` starts OMP through the spawn profile with no terminal; settings from the user's `cedian.toml`; crash isolation (OMP dies → panel shows it, IDE lives) | gpui test with fake OMP: launch → ready; kill OMP → panel error, app alive; restart restores the session | 1.5 |
| U3a | OMP settings page ([ADR-0040](../decisions/0040-omp-settings-mirror-omp-config.md)): read and write OMP's own settings through OMP, live on `config_update`, the layer of each value shown, the spawn overlay shown as "set by cedian"; a "Model roles" section ([ADR-0039](../decisions/0039-model-roles-and-independent-review.md) decision 2); sessions shared with the OMP CLI through `open_session` | gpui test with fake OMP: a `config_update` re-reads the page; a shadowed write says by what; `omp config set` accepts one `modelRoles` entry, or the section is read-only (finding recorded) | 1.5 |
| **B. Agent loop native** | | | |
| U4 | Prompt, stream, cancel, image input, `ask`, permission dialogs answered in the UI (replacing headless refusals), audit rows from the dialog record | gpui tests per surface; one live recorded turn replayed in the app harness | 2 |
| U5 | Edits are buffer transactions with native undo (S9a import); Review Changes per task, hunk accept/reject, stale detection on Zed buffers; delete stand-ins G and I | gpui tests port the S0 replays' checks; `Version` becomes `clock::Global` | 2 |
| U6 | Context: active selection to OMP; diagnostics and LSP/DAP from Zed's project; delete stand-in B (`cedian_lsp`, `cedian_dap`) | gpui test: selection and a diagnostic reach the prompt context; no cedian LSP/DAP process | 1.5 |
| **C. Closes S4 and S5** | | | |
| U7 | Browser: one long-lived Chromium shared with OMP, same tab for agent and user, inline screenshot, console and network; delete stand-in D | S4 exit through the app: frame seq across captures, `stale-frame` real | 2 |
| U8 | Subagents and worktrees visible, cancel and steer through the live session | S5 exit through the app: subagent events render, a steer reaches the worker | 1.5 |
| **D. Rigor visible** | | | |
| U9 | Workflow, verification evidence and review findings visible; findings annotate code; reviewers shown from cedian's records (ADR-0041) | gpui tests over the S2/S3 replays | 1.5 |
| U10 | ⌘K inline edit and revert turn in the editor | gpui test ports the P6 replay | 1 |
| **E. Close** | | | |
| U11 | Parity: every `OMP_PARITY.md` row native, gated with a working opt-in, or upstream-blocked; parity test green on the pin | parity test + row review | 1 |
| U12 | Benchmark re-run on the app (frame time while streaming, per-turn overhead, B1–B10 against the S2 baseline) | `benchmark-checklist` applied; results recorded here | 1.5 |
| U13 | Packaging: branding, `.app` bundle, signing and notarization for cedian and the bundled OMP | needs full Xcode and a signing identity: owner step | 1 |
| U14 | README S0–S5 and S9 rows, ROADMAP stand-ins removed, plan → `done/` | docs only | — |

## U1 design

- **Paths in the fork.** `crates/cedian_*`, `vendor/omp-rpc`, `vendor/omp-revision.json`, `script/cedian-bench/`, `cedian/OMP_PARITY.md`. The fork's `members` list is explicit, so each cedian crate is added by name.
- **Dependencies.** The fork's `serde` has no `derive` feature: each cedian crate asks for it. `serde_json` gains `preserve_order` through the fork's workspace, which can change key order in JSON the tests compare. `toml` stays at the version `cedian_shell` pins unless 0.9 is a drop-in. Lints become the fork's; findings are fixed, not silenced.
- **History.** Files are copied in one commit that names the source commit here (`git filter-repo` is not installed). This repo's history keeps everything before the move.
- **Checks.** `script/cedian-check` in the fork runs `cargo fmt --check`, `clippy` and `test` for the cedian crates only, so a check does not build all of Zed. This repo's pre-commit hook stops running cargo.

## Progress

- **U1 done (2026-10-07).** Fork commits `00ab4c66d7` (the move), `3f446e30b6` (CI, guardrails, build-omp), `26814bc04c` (ADR renumbering), `a7e8478977` (S3 conformance to ADR-0039, done while the code was moving). `script/cedian-check` passes 258 tests and 14 replays with no real OMP on PATH; `cedian_panel` checks. This repo is docs only (`fa03403`).

