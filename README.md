# cedian

**cedian is a Cursor-style agentic IDE — the same agent + IDE workflow and product features — built for OMP: OMP runs as upstream ships it, every OMP feature gets a native surface, and Zed is forked only as far as OMP needs. OMP decides; cedian executes, renders, and verifies.**

Built in Rust + GPUI on a Zed fork. Identity and parity: [ADR-0034](docs/decisions/0034-identity-and-omp-parity.md).

> Docs: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) (rules) · [`docs/decisions/`](docs/decisions/) (why) · [`docs/ROADMAP.md`](docs/ROADMAP.md) (slices, exits, stand-ins, Cursor workflow coverage) · [`docs/OMP_PARITY.md`](docs/OMP_PARITY.md) (per-feature OMP coverage). This table is the only status source.
> Stack lock: Rust + GPUI only in the cedian process. No TypeScript/Electron/WebView/Tauri.

## Status (slices; execution order in ROADMAP)

✅ = exit holds through an OMP turn or hermetic replay; ◐ = partial, gap named (ROADMAP exit-criterion rule).

| Slice | State |
|---|---|
| S0 Foundation loop | ✅ 2026-10-07 by hermetic replay: prompt → streamed text in the thread → tool card → edit → review → accept/reject persisted, STALE over a user edit, OMP disk edits never overwritten; evidence in [the record](docs/plans/done/s0-foundation-loop.md#exit-evidence) |
| S1 Language services | ◐ LSP client + symbols + bridge (fake-green); live diagnostics need a warm server (`cedian shell`, P4), DAP-live needs Developer mode |
| S9a App spike | ✅ 2026-10-07: the fork builds as `cedian` and a live GPUI test drives an OMP turn into `CedianPanel`, with OMP's edit imported as one undoable transaction; layout choice open (ROADMAP Follow-ups); evidence in [the plan](docs/plans/done/s9a-app-spike.md#exit-evidence) |
| P6 Revert turn + inline edit | ✅ 2026-10-07 by hermetic replay of a recorded `cedian shell` session: inline edit changes only its range, revert turn keeps later user lines (STALE); evidence in [the plan](docs/plans/done/p6-revert-turn-inline-edit.md#exit-evidence) |
| P7 OMP parity ledger | ✅ 2026-10-07: `cargo test -p cedian_omp --test omp_parity` passes against the vendored `wire.rs` (all commands, notifications and UI requests have a row in `docs/OMP_PARITY.md`), and fails naming the feature when a row is renamed (checked: `steer` → `command \`steer\``); runs in pre-commit through `cargo test --workspace` |
| S2 Workflow engine | ✅ 2026-10-07 by hermetic replay of recorded OMP turns, and the [ADR-0037](docs/decisions/0037-benchmark-without-cursor.md) benchmark within budget (0 false-done of 10; cedian overhead median 28 ms per turn); evidence in [the plan](docs/plans/done/s2-workflow-core.md#exit-evidence) |
| P8 OMP policy opt-in | ✅ 2026-10-07 by live recording and hermetic replay (`replay_p8_omp_policy`, `replay_p8deny_omp_policy`): an opted-in project runs on OMP's own approvals, audited, and cedian denies still win; GPUI surfaces at S9; evidence in [the plan](docs/plans/done/p8-omp-policy.md#exit-evidence) |
| S3 Review agents | 🔜 gated on sandbox profile + audit tuples; gate 3 holds 2026-10-07: the user's `cedian.toml` is the only settings file (`[permissions]`, `reviewer_allow_list`, `[[workflow.floor]]`), workspace `cedian.json`/`cedian.toml` refused (`cargo test -p cedian_cli --test settings_cli`) |
| S4 Browser evidence | ◐ CDP screenshot/DOM as gate evidence; fresh Chrome per command, so frame seq / same-tab are not real yet (ROADMAP stand-in row D); closes inside S9 ([ADR-0038](docs/decisions/0038-s4-s5-close-inside-s9.md)) |
| S5 Parallel workers | ◐ worktree mechanism + safe merge-back/remove from CLI; P5: an OMP turn requests a worktree via `cedian_worktree_request` and cedian creates it (hermetic replay); subagent view + real steer missing (ROADMAP stand-in row C); closes inside S9 ([ADR-0038](docs/decisions/0038-s4-s5-close-inside-s9.md)) |
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
cargo run -p cedian_cli -- shell               # P4: long-lived session (prompt/steer/abort + any verb); holds .cedian/shell.lock
cargo test --workspace            # unit + fake-omp replay (fast, hermetic)
cargo test -p cedian_omp -- --ignored --nocapture          # live smoke vs real omp
cargo test -p cedian_omp --test live_spawn_profile -- --ignored --nocapture  # P1 precedence + auth
cargo test -p cedian_fake_omp -- --ignored record_       # re-record runtime fixture (live)
CEDIAN_P2_RECORD=cli cargo test -p cedian_cli --test replay_cli    # re-record CLI fixture (live; or =shell|channel)
cargo test -p cedian_agent_ui -- --ignored --nocapture     # live panel + tool cards
cargo clippy --workspace --all-targets && cargo fmt --all
```

Settings: the user's `cedian.toml` only — `$CEDIAN_CONFIG`, else
`$XDG_CONFIG_HOME/cedian/cedian.toml`, else `~/.config/cedian/cedian.toml`
(absent = defaults; `schema = 1` required; unknown keys fail). A
`cedian.toml` or `cedian.json` inside the workspace is refused, never read
(ADR-0018, ADR-0035). Tests set `CEDIAN_CONFIG` to their own file.

Live smoke needs ambient OMP auth (e.g. `openai-codex` subscription).
Never pin `--provider/--model` to a keyed provider in code — it fails with
`No API key found` outside environments that have the key.
