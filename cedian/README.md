# cedian

Zed-fork (GPUI/Rust) environment + OMP sidecar as sole intelligence runtime.
Rule: `cedian = environment, OMP = intelligence`.

> Spec: [`CEDIAN_AGENTIC_IDE_PLAN.md`](CEDIAN_AGENTIC_IDE_PLAN.md) (~3640 lines, §§1–91).
> Stack lock: Rust + GPUI only in the cedian process. No TypeScript/Electron/WebView/Tauri.

## Status

| Phase | State |
|---|---|
| 0.5 RPC Spike | ✅ `spike/` — 14/14 vs omp 18.6.1, see `spike/CAPABILITY_TABLE.md` |
| 1 OMP Runtime | ✅ `crates/cedian_omp/` — spawn, v2, prompt/abort/restore/images |
| 2 Agent Panel (headless) | ✅ `crates/cedian_agent/` + `crates/cedian_agent_ui/` |
| 3 Tool Cards | ✅ args preview + result summary, no raw JSON |
| 4 Edit Surface | ▶ next — host tools + `cedian://` + buffer transactions |

GPUI views bind when the Zed fork lands. Until then every crate is headless
(state machines + tests + live smoke against the real `omp --mode rpc-ui`).

## Layout

```text
crates/cedian_omp/       process + protocol boundary (runtime, event_router, session)
crates/cedian_agent/     task / thread / state (one task = one OMP session)
crates/cedian_agent_ui/  panel / composer / message / tool_card / ask (headless models)
vendor/omp-rpc/          vendored upstream Rust RPC client (pin: vendor/omp-revision.json)
spike/                   Phase 0.5 throwaway probe (do not grow)
script/build-omp         build pinned OMP into cedian.app (records commit/tree-hash/builder)
```

## Build / test

```sh
cargo build --workspace
cargo test --workspace            # unit (fast, hermetic)
cargo test -p cedian_omp -- --ignored --nocapture          # live smoke vs real omp
cargo test -p cedian_agent_ui -- --ignored --nocapture     # live panel + tool cards
cargo clippy --workspace --all-targets && cargo fmt --all
```

Live smoke needs ambient OMP auth (e.g. `openai-codex` subscription).
Never pin `--provider/--model` to a keyed provider in code — it fails with
`No API key found` outside environments that have the key.
