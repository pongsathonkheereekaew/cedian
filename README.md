# cedian

**cedian is a Cursor-style agentic IDE — the same agent + IDE workflow and product features — built for OMP: OMP runs as upstream ships it, every OMP feature gets a native surface, and Zed is forked only as far as OMP needs. OMP decides; cedian executes, renders, and verifies.**

Built in Rust + GPUI on a Zed fork. Identity and parity: [ADR-0034](docs/decisions/0034-identity-and-omp-parity.md).

> Docs: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) (rules) · [`docs/decisions/`](docs/decisions/) (why) · [`docs/ROADMAP.md`](docs/ROADMAP.md) (slices, exits, stand-ins, Cursor workflow coverage) · [`cedian/OMP_PARITY.md`](https://github.com/pongsathonkheereekaew/zed/blob/cedian/s9/cedian/OMP_PARITY.md) in the fork (per-feature OMP coverage). This table is the only status source.
> Stack lock: Rust + GPUI only in the cedian process. No TypeScript/Electron/WebView/Tauri.

## Status (slices; execution order in ROADMAP)

✅ = exit holds through an OMP turn or hermetic replay; ◐ = partial, gap named (ROADMAP exit-criterion rule).

| Slice | State |
|---|---|
| S0 Foundation loop | ✅ 2026-10-07 by hermetic replay: prompt → streamed text in the thread → tool card → edit → review → accept/reject persisted, STALE over a user edit, OMP disk edits never overwritten; evidence in [the record](docs/plans/done/s0-foundation-loop.md#exit-evidence) |
| S1 Language services | ◐ LSP client + symbols + bridge (fake-green); live diagnostics need a warm server (`cedian shell`, P4), DAP-live needs Developer mode |
| S9a App spike | ✅ 2026-10-07: the fork builds as `cedian` and a live GPUI test drives an OMP turn into `CedianPanel`, with OMP's edit imported as one undoable transaction; layout choice open (ROADMAP Follow-ups); evidence in [the plan](docs/plans/done/s9a-app-spike.md#exit-evidence) |
| P6 Revert turn + inline edit | ✅ 2026-10-07 by hermetic replay of a recorded `cedian shell` session: inline edit changes only its range, revert turn keeps later user lines (STALE); evidence in [the plan](docs/plans/done/p6-revert-turn-inline-edit.md#exit-evidence) |
| P7 OMP parity ledger | ✅ 2026-10-07: `cargo test -p cedian_omp --test omp_parity` passes against the vendored `wire.rs` (all commands, notifications and UI requests have a row in the fork's `cedian/OMP_PARITY.md`), and fails naming the feature when a row is renamed (checked: `steer` → `command \`steer\``); runs in pre-commit through `cargo test --workspace` |
| S2 Workflow engine | ✅ 2026-10-07 by hermetic replay of recorded OMP turns, and the [ADR-0037](docs/decisions/0037-benchmark-without-cursor.md) benchmark within budget (0 false-done of 10; cedian overhead median 28 ms per turn); evidence in [the plan](docs/plans/done/s2-workflow-core.md#exit-evidence) |
| P8 OMP policy opt-in | ✅ 2026-10-07 by live recording and hermetic replay (`replay_p8_omp_policy`, `replay_p8deny_omp_policy`): an opted-in project runs on OMP's own approvals, audited, and cedian denies still win; GPUI surfaces at S9; evidence in [the plan](docs/plans/done/p8-omp-policy.md#exit-evidence) |
| S3 Review agents | ✅ 2026-10-07 by recorded OMP turns replayed hermetically: an OMP turn asks for a review, cedian runs the reviewer as its own sandboxed OMP process on the `review` model role, a same-model review is `inconclusive` (ADR-0039), a blocker refuses completion until dismissed with an audited reason; gate items 1–4 and the correction ledger hold; evidence in [the plan](docs/plans/done/s3-review-agents.md#exit-evidence) |
| S4 Browser evidence | ◐ CDP screenshot/DOM as gate evidence; fresh Chrome per command, so frame seq / same-tab are not real yet (ROADMAP stand-in row D); closes inside S9 ([ADR-0038](docs/decisions/0038-s4-s5-close-inside-s9.md)) |
| S5 Parallel workers | ◐ 2026-10-09: in the app, OMP's subagents (the workers, [ADR-0050](docs/decisions/0050-workers-are-omp-subagents.md)) show under their task card and a steer or cancel reaches them; OMP asks for a worktree with the ADR-0033 brief and cedian creates it; `OutOfScope`, `stuck` and cleanup classification missing (ROADMAP Follow-ups); closes inside S9 ([ADR-0038](docs/decisions/0038-s4-s5-close-inside-s9.md)); [evidence](docs/plans/s9-real-app.md#progress) |
| S6 PR workspace | 🔜 `gh`, PR baselines, merge Deny-by-default |
| S7 Local automations | 🔜 cron + history, no cloud ever |
| S8 iOS track | 🔜 extension track, after v0.1 |
| S10 OMP parity | 🔜 every OMP feature the S9 core deferred ([ADR-0057](docs/decisions/0057-parity-bar-and-s10.md)) |
| S9 Real app | ◐ 2026-10-09: U1–U10 done: the app starts and supervises OMP, has a live OMP settings page, runs turns natively (dialogs answered and audited, Stop, images, follow-up and steer, one driver per session), reviews OMP's edits as Zed buffer transactions (bash writes included), gives OMP the person's context and code intelligence from Zed, shares one owned Chromium with the agent, shows OMP's subagents with steer and cancel, and runs the workflow, its gates and evidence, and the independent reviewer in the app with findings on the code, and has inline edit (ctrl-enter) and revert turn in the editor (stand-ins B, D, G deleted); U11 onward is open; [progress](docs/plans/s9-real-app.md#progress) |

## Code, build and test

All code lives in the Zed fork, [`pongsathonkheereekaew/zed`](https://github.com/pongsathonkheereekaew/zed) branch `cedian/s9` ([ADR-0042](docs/decisions/0042-all-code-moves-into-the-fork.md)); this repo holds the docs. In the fork:

```text
crates/cedian_*        cedian's crates (headless models, OMP boundary, review, workflow, CLI harness until S9 ends)
crates/cedian_panel    the GPUI binding (S9)
vendor/omp-rpc         vendored upstream OMP RPC client (pin: vendor/omp-revision.json)
cedian/OMP_PARITY.md   the OMP parity ledger, tested against the vendored wire.rs
script/cedian-check    fmt + clippy + hermetic tests for the cedian crates only
script/cedian-bench    the S2 benchmark harness (tasks from this repo's history)
```

```sh
script/cedian-install-guardrails                          # pre-commit cedian-check, pre-push force guard
script/cedian-check                                       # fmt, clippy, unit + fake-omp replays (hermetic)
cargo run -p cedian_cli -- prompt "fix it"                # headless turn (needs CEDIAN_WORKDIR)
cargo run -p cedian_cli -- shell                          # long-lived session
cargo test -p cedian_omp --test live_spawn_profile -- --ignored   # live lanes need real omp
CEDIAN_P2_RECORD=cli cargo test -p cedian_cli --test replay_cli    # re-record a fixture (live)
```

In this repo, `script/install-guardrails` installs a pre-push force guard and a pre-commit `script/check-adrs` (duplicate ADR number, `docs/decisions/README.md` out of step with the files).

Settings: the user's `cedian.toml` only — `$CEDIAN_CONFIG`, else
`$XDG_CONFIG_HOME/cedian/cedian.toml`, else `~/.config/cedian/cedian.toml`
(absent = defaults; `schema = 1` required; unknown keys fail). A
`cedian.toml` or `cedian.json` inside the workspace is refused, never read
(ADR-0018, ADR-0035). Tests set `CEDIAN_CONFIG` to their own file.

Live smoke needs ambient OMP auth (e.g. `openai-codex` subscription).
Never pin `--provider/--model` to a keyed provider in code — it fails with
`No API key found` outside environments that have the key.
