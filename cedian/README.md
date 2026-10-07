# cedian

**cedian is a Cursor-style agentic IDE — the same agent + IDE workflow and product features — built for OMP: OMP runs as upstream ships it, every OMP feature gets a native surface, and Zed is forked only as far as OMP needs. OMP decides; cedian executes, renders, and verifies.**

Built in Rust + GPUI on a Zed fork. Identity and parity: [ADR-0034](docs/decisions/0034-identity-and-omp-parity.md).

> Docs: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) (rules) · [`docs/decisions/`](docs/decisions/) (why) · [`docs/ROADMAP.md`](docs/ROADMAP.md) (slices, exits, stand-ins, Cursor workflow coverage) · [`docs/OMP_PARITY.md`](docs/OMP_PARITY.md) (per-feature OMP coverage). This table is the only status source.
> Stack lock: Rust + GPUI only in the cedian process. No TypeScript/Electron/WebView/Tauri.

## Status (slices; execution order in ROADMAP)

✅ = exit holds through an OMP turn or hermetic replay; ◐ = partial, gap named (ROADMAP exit-criterion rule).

| Slice | State |
|---|---|
| S0 Foundation loop | ◐ prompt → cards → edit → review → accept/reject in CLI; streaming text, task attribution + STALE, persisted resolutions fixed 2026-10-06; P1 spawn profile live-verified 2026-10-07 (project yolo/computer-on loses to the overlay); P2 hermetic replay 2026-10-07: `cedian prompt` → card → host-tool edit → `review` → `reject` from a recorded OMP turn ; OMP-native disk edits replay too since P6 (fake-omp records each tool's file writes as `fs` frames); `cedian shell` re-reads buffers from disk before every turn (stale buffers had skewed pre-turn snapshots); P4 `cedian shell` 2026-10-07: one runtime serves two turns + `.cedian/shell.lock` blocks mutating one-shots (hermetic replay), steer/abort mid-turn live-verified; headless CLI/shell answer OMP approval dialogs fail-closed (Deny at once, refusal printed) instead of stalling until the prompt timeout |
| S1 Language services | ◐ LSP client + symbols + bridge (fake-green); live diagnostics need a warm server (`cedian shell`, P4), DAP-live needs Developer mode |
| S9a App spike | ✅ 2026-10-07 — fork `~/src/zed` branch `cedian/s9a-spike` builds as `cedian` (runtime shaders, no Xcode); live gpui test drives the real `CedianPanel`: OMP turn through the spawn profile streams into `Thread`, OMP's own `edit` is imported as ONE transaction keyed by `tool_call_id`, one buffer undo reverts it (`cargo test -p cedian_panel -- --ignored live_` in the fork); findings → ADR-0029/0030 (layout choice pending) |
| P6 Revert turn + inline edit | ✅ 2026-10-07 (hermetic replay of a recorded `cedian shell` session): `edit notes.txt 2-2 …` changes only its target line; `revert-turn 2` puts a prompt turn back but keeps the line the user rewrote since (STALE, listed); reverting the revert redoes it; `turns` lists them — finding: OMP has no RPC model-role switch, so inline edit uses the session model unless `--model provider/id` (ADR-0026 asked for `smol`) |
| P7 OMP parity ledger | ✅ 2026-10-07: `cargo test -p cedian_omp --test omp_parity` passes against the vendored `wire.rs` (all commands, notifications and UI requests have a row in `docs/OMP_PARITY.md`), and fails naming the feature when a row is renamed (checked: `steer` → `command \`steer\``); runs in pre-commit through `cargo test --workspace` |
| S2 Workflow engine | ◐ 2026-10-07 — exit (d) ✅ by hermetic replay of a recorded OMP turn (`replay_s2_bugfix_skill_blocked_claim`): OMP runs the bug-fix playbook skill from the workspace's own `.omp/skills/` copy, reproduces with an attributed `read`, fixes the file, cannot verify (headless denies `bash`/`eval`), claims done with a `measured` claim → `cedian_complete` refuses (claim flagged), the agent fails its own verify phase, and after the turn the workflow is `failed` with `verify` listed (the P5 replay shows the `blocked` case); fast lane ✅ (recorded trivial edit leaves no workflow); ADR-0024 in code (code state + stale, pass/fail/inconclusive, claims ledger, measurements, kind follows the bound tool, `workflow.json` v2), gate floor + add-only `op=gate`, CLI-typed evidence unattributed — gaps: verification profile (feature gates, draft, Doctor health) is unit-tested only, not yet through an OMP turn; gate-floor config waits for row E; benchmark not run (B1–B10 await final approval, `docs/plans/s2-workflow-core.md`) |
| S3 Review agents | 🔜 gated on sandbox profile + audit tuples |
| S4 Browser evidence | ◐ CDP screenshot/DOM as gate evidence; fresh Chrome per command, so frame seq / same-tab are not real yet (ROADMAP stand-in row D) |
| S5 Parallel workers | ◐ worktree mechanism + safe merge-back/remove from CLI; P5: an OMP turn requests a worktree via `cedian_worktree_request` and cedian creates it (hermetic replay); subagent view + real steer missing (ROADMAP stand-in row C) |
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

Live smoke needs ambient OMP auth (e.g. `openai-codex` subscription).
Never pin `--provider/--model` to a keyed provider in code — it fails with
`No API key found` outside environments that have the key.
