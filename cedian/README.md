# cedian

Zed-fork (GPUI/Rust) environment + OMP sidecar as sole intelligence runtime.
Rule: `cedian = environment, OMP = intelligence`.

> Spec: [`CEDIAN_AGENTIC_IDE_PLAN.md`](CEDIAN_AGENTIC_IDE_PLAN.md) (~3640 lines, §§1–91).
> Stack lock: Rust + GPUI only in the cedian process. No TypeScript/Electron/WebView/Tauri.

## Status (slices — value order; phases stay as work-breakdown in the plan §84)

| Slice | State |
|---|---|
| S0 Foundation loop | ✅ prompt → cards → edit → review → accept/reject in CLI |
| S1 Language services | ✅ LSP client + symbols + bridge (fake-green); diagnostics-live = follow-up (needs warm server), DAP-live needs Developer mode |
| S2 Workflow engine | 🔜 gates + playbooks as pure functions |
| S3 Review agents | 🔜 gated on sandbox profile + audit tuples |
| S4 Browser evidence | 🔜 CDP screenshot/DOM as gate evidence |
| S5 Parallel workers | 🔜 worktrees + swarm/arena presets |
| S6 PR workspace | 🔜 `gh`, PR baselines, merge Deny-by-default |
| S7 Local automations | 🔜 cron + history, no cloud ever |
| S8 iOS track | 🔜 extension track, after v0.3 |
| S9 Real app | 🔜 fork + GPUI binding (the only slice that yields `cedian.app`) |

## Layout

```text
crates/cedian_omp/       process + protocol boundary (runtime, event_router, session)
crates/cedian_agent/     task / thread / state (one task = one OMP session)
crates/cedian_agent_ui/  panel / composer / message / tool_card / ask (headless models)
crates/cedian_workspace/ host tools + cedian:// + buffer txns + ambient context
crates/cedian_review/    baseline + provenance + hunk accept/reject
crates/cedian_shell/     settings + palette + session manager (headless)
crates/cedian_cli/       throwaway harness — full loop without GPUI (dies at S9)
vendor/omp-rpc/          vendored upstream Rust RPC client (pin: vendor/omp-revision.json)
spike/                   Phase 0.5 throwaway probe (do not grow)
script/build-omp         build pinned OMP into cedian.app (records commit/tree-hash/builder)
```

## Build / test

```sh
cargo build --workspace
cargo run -p cedian_cli -- state            # OMP session snapshot
cargo run -p cedian_cli -- prompt "fix it"      # full turn (needs CEDIAN_WORKDIR)
cargo run -p cedian_cli -- review              # pending hunks
cargo test --workspace            # unit (fast, hermetic)
cargo test -p cedian_omp -- --ignored --nocapture          # live smoke vs real omp
cargo test -p cedian_agent_ui -- --ignored --nocapture     # live panel + tool cards
cargo clippy --workspace --all-targets && cargo fmt --all
```

Live smoke needs ambient OMP auth (e.g. `openai-codex` subscription).
Never pin `--provider/--model` to a keyed provider in code — it fails with
`No API key found` outside environments that have the key.
