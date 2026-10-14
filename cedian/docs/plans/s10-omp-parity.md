# S10 — OMP parity plan

Exit (ROADMAP S10 is the contract): no row of the fork's `cedian/OMP_PARITY.md` says `deferred: S10`, and the parity test is green on the pin. Each deferred row ends `native`, `gated`, `upstream-blocked` or `unused by decision`; the last needs its ADR ([ADR-0057](../decisions/0057-parity-bar-and-s10.md)). S10 also delivers the two Cursor workflows ROADMAP gives it: Tab completion ([ADR-0068](../decisions/0068-s9-closes-as-a-partial-exit.md)) and the usage meter.

**Where the work happens.** Code lives in the fork (`~/src/zed`, `pongsathonkheereekaew/zed`). Each unit is a `cedian/s10-<unit>` branch off `cedian/s9` and its own PR into `cedian/s9`, merged when CI is green (owner 2026-10-14). This repo keeps the docs.

**Pin.** OMP 18.8.6 (`vendor/omp-revision.json`). A pin bump is its own change with an ADR; it is not part of a unit.

**Machine (2026-10-14).** 19 GB free with a 27 GB warm `target/`. One writer in the fork at a time and no extra worktrees (cargo rebuilds per path), so units run one after another, never in parallel.

**Recordings.** Frames that need no model turn (session lists, the tree, stats, commands, settings) are recorded from the pinned OMP over a real session file. Frames that need a model turn are recorded in one pass, `s10`, of at most 20 turns on `cursor/default` ([ADR-0070](../decisions/0070-s10-live-pass-cursor-default-20.md)); a unit the pass cannot cover keeps hand-made frames, named in ROADMAP Follow-ups.

**v0.1 first (owner, 2026-10-14).** After U1, S1's missing exit item (an agent breakpoint round trip through a debug adapter) is built, and v0.1 is then declared with S5 waiting on OMP to name the worktree, in its own ADR. U2 follows.

**Merges (owner, 2026-10-14).** The owner merges every PR, docs and fork. The agent opens it after review and green CI, and names the merge order.

**Timebox.** 16 working sessions. Each unit is its own partial exit: commit what is green, README names the rest. ROADMAP follow-ups that name S10 are taken by the unit that touches the same code; the exit does not wait on them.

## Units (order)

The order puts the Cursor workflows the owner uses first (threads and checkpoints, plan mode, the usage meter), then the composer and the side channels, then settings, and the two units with an open product question last.

| # | Unit | Ledger rows | Check (done when) | Box |
|---|---|---|---|---|
| **A. Threads** | | | | |
| U1 | Session manager and history | `switch_session`, `set_session_name`, `session_info_update`, `get_entries`, `get_messages`, `get_messages_page`, `get_last_assistant_text`, `handoff`, `export_html` | gpui tests with fake OMP over recorded frames: list the workspace's sessions, switch (the review and the one-driver check follow the switch), rename (and OMP's own rename shows), an opened session renders its history page by page, handoff compacts the session to OMP's summary (on the pin it opens no new session), export writes the file OMP made | 2 |
| U2 | Thread tree: branch, fork, checkpoints | `branch`, `fork`, `get_branch_messages`, `get_tree` | gpui test: the tree renders from `get_tree`; branching from an earlier message restores that branch's thread; fork opens a new session; OMP's `branch` opens a child session, so "Branch here" leaves files alone and "Restore checkpoint and branch" first restores every later agent change ([ADR-0071](../decisions/0071-restore-checkpoint-restores-everything.md)) | 1.5 |
| **B. Modes and meters** | | | | |
| U3 | Plan mode and goal mode ([ADR-0014](../decisions/0014-agent-modes.md)) | Plan mode (launch-time), `goal`, `goal_updated` | gpui test: the plan runtime starts through the spawn profile and its plan reaches the main session; a goal set, paused and done in the panel matches `goal_updated` | 1.5 |
| U4 | Usage meter and status line | `get_session_stats`, `advisor_cost_changed`, `advisor_yielded`, `cache_warming_start`, `cache_warming_end`, `set_cache_warming`, `set_fast_mode`, `set_slow_mode`, `setStatus`, `setWidget`, `setTitle`, `set_event_filter` | gpui test: the meter shows tokens and cost from `get_session_stats` and updates on the advisor events; an extension's `setStatus` and `setWidget` render, `setTitle` names the window; the event filter is set from what the panel renders | 1.5 |
| U5 | Todos | `set_todos`, `todo` (tool), `todo_reminder`, `todo_auto_clear` | gpui test: OMP's todo list renders in the workflow view and follows the tool, the reminder and the clear | 1 |
| **C. Composer and side channels** | | | | |
| U6 | Slash commands and composer text | `get_available_commands`, `available_commands_update`, `set_editor_text` | gpui test: typing `/` lists OMP's commands, an update refreshes them, an extension's `set_editor_text` fills the composer | 1 |
| U7 | `btw` and the person's own `bash` | `btw`, `btw_cancel`, `get_btw_history`, `btw_delta`, `btw_record`, `bash`, `abort_bash`, `command_output`, the `bash` tool row's deferred part | gpui test: a side question streams beside a running turn without touching it and stays in history; a person's `bash` streams its output and can be aborted; its writes are attributed as the person's (ADR-0047) | 1.5 |
| U8 | Subagent transcripts | `get_subagents`, `get_subagent_messages`, `irc_message`, `hub`/`wait`, `vibe_spawn`/`vibe_send` | gpui test: a subagent row opens its transcript; an IRC message shows in the subagent view; hub and vibe calls render as subagents. Takes the U8 follow-up row | 1.5 |
| **D. Settings** | | | | |
| U9 | Settings listings and accounts | `--profile`, skills/rules/agents listing, MCP/extensions listing, model roles page, the 18.8.6 config keys, `get_logout_accounts`, `logout` | gpui test: the settings page lists what OMP loaded, read-only; a profile is chosen per workspace; model roles are shown and written through OMP ([ADR-0040](../decisions/0040-omp-settings-mirror-omp-config.md)); one stored credential is removed | 1.5 |
| **E. Open product questions** | | | | |
| U10 | Composer word prediction and Tab completion | `predict_word`, `predict_word_feedback` | composer: a gpui test shows OMP's prediction as ghost text, accepted with Tab and reported back. Editor Tab completion: OMP's `predict_word` is a local prose-word engine (n-gram, SmolLM2 and macOS's own, blended in a daemon), one word at the cursor, not a multi-line code edit. Which editor predictor (Copilot, a local model, or OMP's word engine in the editor) is an owner call made at the unit's start, after a prototype compares them | 2 |
| U11 | Live (voice) session | `live_start`, `live_stop`, `live_mute`, `live_phase`, `live_levels`, `live_transcript`, `live_end` | gpui test over recorded frames: start, mute, the level meter, the transcript, end; microphone permission handled by macOS's prompt | 1.5 |
| **F. Close** | | | | |
| U12 | Parity test refuses `deferred: S10`; README row, ROADMAP, plan → `done/` | — | the parity test fails on a `deferred: S10` row; docs only otherwise | — |

## Progress

- **Planned (2026-10-14).** 39 ledger rows say `deferred: S10` on fork `f8e9d69c61`. U1 starts.
- **U1 merged (2026-10-14, fork #9).** Sessions list, switch, rename, history paging, handoff and HTML export in the app. The review (fable, APPROVE) left three follow-ups that U2 takes, since U2 changes the same switch path: a per-request session id on off-thread reads and rename, `session_file` in `LinkEvent::Taken`, and an in-flight guard on Load more.
- **S1-dap partial (2026-10-14, fork #10, #11, #12).** `cedian_debug` drives Zed's own debug session ([ADR-0073](../decisions/0073-cedian-debug-interim-exception.md)). The hermetic round trip on Zed's fake adapter runs in CI, and the real Command Line Tools `lldb-dap` stopped at `main.c:4` with `x = 42` and exited. The live `cursor/default` turn ran away: the `harness = false` record binary was re-executed by Zed's `--printenv` loader and spent 25 turns of the pass's 20. Every tap shows the model calling `cedian_debug`, none OMP's `debug`; no fixture was written. [ADR-0074](../decisions/0074-s1-live-turn-after-the-runaway.md) closes the `s10` pass and grants S1 2 turns behind an argument guard and a watchdog. Fork #13 makes `cedian_debug` ask once, at cedian's gate.
- **OMP upstream (2026-10-14).** [can1357/oh-my-pi#15727](https://github.com/can1357/oh-my-pi/pull/15727) lets OMP's `debug` use a host's session (fixes #15703). When a pin carries it, `cedian_debug` goes and ADR-0073 is superseded.
