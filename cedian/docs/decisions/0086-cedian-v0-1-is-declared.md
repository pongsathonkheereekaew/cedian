# ADR-0086: cedian v0.1 is declared

- **Status:** Proposed (agent, 2026-10-17; the owner accepts by merging the PR, or rejects it there)
- **Date:** 2026-10-17
- **Rule text:** none in code; README's status table stays the only status source
- **Implements:** ADR-0077 decision 2 (v0.1 is declared in its own ADR once every gate holds)
- **Builds on:** ADR-0075 (the narrow fork), ADR-0078 (one product in two repos), ADR-0084 (project-scoped reads), ADR-0085 (worktree attribution)

## Context

ADR-0077 decision 2 holds v0.1 back until five things are true: S0 to S5 are ✅ in README, the app runs the fork's OMP bundled with it, S5's worktree attribution works through a fork patch, reads are scoped to the project, and a cancelled approval is never audited as allowed. The last of these, S5's attribution, merged on 2026-10-16 (cedian #30, main `8efe58dcd5`). This ADR checks each gate against the repository and states what v0.1 is, what it lacks and what follows it.

## Decision

1. **v0.1 is a private, Cursor-style agentic IDE for one person, the owner.** It is a Zed fork with the cedian agent (the OMP fork) bundled inside `cedian.app`. Its tagline is "An editor that shows its work". The person prompts the agent in the panel, sees each tool call and edit, reviews and accepts or rejects hunks, runs subagents in their own worktrees, and approves anything outside the project. Nothing is sent to an issue tracker or an update server.
2. **Every ADR-0077 gate holds.**

   | Gate | Evidence |
   |---|---|
   | S0 to S5 ✅ in README | At `8efe58dcd5` the rows S0, S1, S2, S3, S4, S5 are ✅, as are S9a and P8. S9 and S10 are ◐; they are not gates. S6, S7 and S8 are 🔜. |
   | The app runs the bundled fork OMP | ADR-0075 decision 5, ADR-0078, ADR-0083. Fork #19: the app runs only the bundled build, checked by commit and sha256, never the person's `omp`; a release build ignores `CEDIAN_OMP_BINARY` (cedian #2). `script/cedian-omp-build` builds it and `script/cedian-bundle` puts it in `Contents/Resources/cedian-agent`. |
   | S5 worktree attribution through a fork patch | ADR-0085; cedian-agent #5 and cedian #30. Tests: `test/rpc-subagents.test.ts`, `test/task/task-cwd.test.ts`, `cedian_panel/tests/subagents.rs` step 6b, `tests/worker_trees.rs` step 2b, `a_task_calls_disk_writes_are_attributed_to_its_subagents_tree`. One review and one fix round, approved. |
   | Reads scoped to the project | ADR-0084; cedian-agent #4 and cedian #22, after five security review rounds. Last round: fork suites 2792 pass, 0 fail; `script/cedian-check` 844 passed, 0 failed. |
   | A cancelled approval is not audited as allowed | Fork #18: a cancelled approval closes its dialog and is audited `abstain`, for `cedian_debug` and worktree asks ([S10 plan](../plans/s10-omp-parity.md#progress)). |

   The app around these gates also landed before v0.1: Settings (SET-1 to SET-4 in cedian #8, #12, #18, #21, then polish #23 and the sidebar fix #28), the cedian Dark and Light themes (#9), and the agent's issue reports and update check pinned off (ADR-0080, #8).
3. **v0.1 ships with these known gaps.** Each stays open in its issue:
   - #31, S5 follow-ups: a race where the first `bash` or `task` call can start before the scan of a worker's tree, `protectedPaths` missing from the spawn bound, a canonical path for tail entries.
   - #29, Settings follow-ups from the review of #28.
   - #16, Settings > Account shows stale state and fails quietly after SET-2. #25, Models and Account show a Start button while the agent starts, not Loading.
   - #24, the `stop_and_send` test is flaky against fake-omp timing.
   - #26, tests leak temporary directories (about 5000 `cedian-*` directories, 2.6 GB).
   - #14, the panel's thread does not follow the tail while streaming. #15, the thread list's virtualization tests and bench lint.
4. **v0.1 does not include:**
   - S10 beyond U1: 1 of 12 units is in the app. U9 (Settings listings, accounts, and pstack's panel agents, which ship but are not loaded), the usage and cost meter, and the other parity units wait for S10.
   - S6 (PR workspace), S7 (local automations) and S8 (iOS).
   - A signed or notarized build. `script/cedian-bundle` builds unsigned unless `CODESIGN_IDENTITY` is set (ADR-0062: the owner's own build). The ROADMAP marks the bundled agent's self-attesting stamp as needing signing before cedian runs on another machine.
   - Auto-update. Zed's `auto_update` is `false` in the default settings, and the agent's update check is pinned off (ADR-0080). The ROADMAP's update path (app and agent as one unit) is not built.
   - Multi-select in the panels. The repository records no work on it.
5. **v0.1 was declared without spending a live model turn.** Ruling 43 allowed 3 live turns across S5 and the panel-agent check; S5 used 0 of 3. S1's own live pass used 2 of its 3 (ADR-0076).
6. **The release step is a tag, made by the owner.** The repository defines no release process for cedian. `APP_VERSION` in `crates/paths/src/paths.rs` is already `0.1.0`, `script/cedian-bundle` reads it, and Settings > About shows `pre-release · {channel} · {commit}`. No `v0.1.0` tag exists. The minimal step: after merging this ADR, the owner tags the merge commit `v0.1.0` and builds with `script/cedian-bundle`. The About line keeps saying "pre-release"; changing it is a separate code change, not part of this ADR.

## Consequences

- After v0.1 the owner uses cedian on real work first. The roadmap then continues in the order that use shows: S10, S6, S7 (ruling 32).
- Bugs found while dogfooding go to issues and are ranked against the gaps in decision 3.
- The "upstream-blocked" rule of ADR-0077 decision 3 keeps applying after v0.1.
- README keeps no v0.1 marker; the status table is unchanged. A version row, if the owner wants one, is a separate change.
