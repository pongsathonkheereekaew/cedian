# cedian

**cedian is an agentic IDE built in Rust + GPUI (a Zed fork), with OMP as its only harness: OMP decides, cedian executes, renders, and verifies.**

> Docs: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) (rules) · [`docs/decisions/`](docs/decisions/) (why) · [`docs/ROADMAP.md`](docs/ROADMAP.md) (slices, exits, stand-ins). This table is the only status source.
> Stack lock: Rust + GPUI only in the cedian process. No TypeScript/Electron/WebView/Tauri.

## Status (slices; execution order in ROADMAP)

✅ = exit holds through an OMP turn or hermetic replay; ◐ = partial, gap named (ROADMAP exit-criterion rule).

| Slice | State |
|---|---|
| S0 Foundation loop | ◐ prompt → cards → edit → review → accept/reject in CLI; streaming text, task attribution + STALE, persisted resolutions fixed 2026-10-06; P1 spawn profile live-verified 2026-10-07 (project yolo/computer-on loses to the overlay); P2 hermetic replay 2026-10-07: `cedian prompt` → card → host-tool edit → `review` → `reject` from a recorded OMP turn — gap: OMP-native disk edits are not replayed (fake-omp writes no files) |
| S1 Language services | ◐ LSP client + symbols + bridge (fake-green); live diagnostics need a warm server (`cedian shell`, P4), DAP-live needs Developer mode |
| S9a App spike | ◐ fork `~/src/zed` branch `cedian/s9a-spike` builds as `cedian` (runtime shaders, no Xcode); `cedian_panel` registered; OMP-write → ONE undoable transaction keyed by `tool_call_id` + `Version`↔`clock::Global` proven by gpui tests (watcher race, dirty buffer = STALE) — gap: live panel stream + ⌘Z on a real OMP `edit` not yet run (T2/T5, manual); findings → ADRs before S2 |
| S2 Workflow engine | ◐ gates block completion when driven from CLI; not yet enforced on an OMP turn; A1-narrow decided (ADR-0010): OMP drives, cedian checks |
| S3 Review agents | 🔜 gated on sandbox profile + audit tuples |
| S4 Browser evidence | ◐ CDP screenshot/DOM as gate evidence; fresh Chrome per command, so frame seq / same-tab are not real yet (ROADMAP stand-in row D) |
| S5 Parallel workers | ◐ worktree mechanism + safe merge-back/remove from CLI; OMP-requested worktrees + subagent view + real steer missing (ROADMAP stand-in row C) |
| S6 PR workspace | 🔜 `gh`, PR baselines, merge Deny-by-default |
| S7 Local automations | 🔜 cron + history, no cloud ever |
| S8 iOS track | 🔜 extension track, after v0.1 |
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
crates/cedian_fake_omp/  P2 test harness: fake `omp --mode rpc-ui` (record proxy + fixture replay)
vendor/omp-rpc/          vendored upstream Rust RPC client (pin: vendor/omp-revision.json)
spike/                   Phase 0.5 throwaway probe (do not grow)
script/build-omp         build pinned OMP into cedian.app (records commit/tree-hash/builder)
```

## Build / test

```sh
cargo build --workspace
cargo run -p cedian_cli -- state            # OMP session snapshot
cargo run -p cedian_cli -- prompt "fix it"      # full turn (needs CEDIAN_WORKDIR)
cargo run -p cedian_cli -- review              # pending hunks (task state: .cedian/review.json)
cargo run -p cedian_cli -- review reset        # start a new review task
cargo test --workspace            # unit + fake-omp replay (fast, hermetic)
cargo test -p cedian_omp -- --ignored --nocapture          # live smoke vs real omp
cargo test -p cedian_omp --test live_spawn_profile -- --ignored --nocapture  # P1 precedence + auth
cargo test -p cedian_fake_omp -- --ignored record_       # re-record runtime fixture (live)
CEDIAN_P2_RECORD=1 cargo test -p cedian_cli --test replay_cli  # re-record CLI fixture (live)
cargo test -p cedian_agent_ui -- --ignored --nocapture     # live panel + tool cards
cargo clippy --workspace --all-targets && cargo fmt --all
```

Live smoke needs ambient OMP auth (e.g. `openai-codex` subscription).
Never pin `--provider/--model` to a keyed provider in code — it fails with
`No API key found` outside environments that have the key.
