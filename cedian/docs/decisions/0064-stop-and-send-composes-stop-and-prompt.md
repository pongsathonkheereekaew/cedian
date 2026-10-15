# ADR-0064: Stop and send composes Stop and a prompt; `abort_and_prompt` is unused

- **Status:** Accepted (agent, 2026-10-14). The owner said to continue the S9 plan. This is the last `deferred: S9` parity row (`omp_parity.rs` `DEFERRED_IN_S9`), and an architect pass with three independent design runners converged on it.
- **Date:** 2026-10-14
- **Rule text:** `crates/cedian_panel/src/panel.rs` (`stop_and_send`, `AfterStop`, `send_next_if_due`), `crates/cedian_panel/tests/stop_and_send.rs`, the `abort_and_prompt` row of `cedian/OMP_PARITY.md`
- **Builds on:** ADR-0059 decision 2 (Stop is `abort_and_restore_queue`), ADR-0006 (review baselines per task, one review turn per prompt), ADR-0056 (an inline edit keeps its own review turn), ADR-0057 (parity statuses), ADR-0034 (cutting a feature needs an ADR)

## Context

OMP's `abort_and_prompt` aborts the running turn and starts a new prompt in one command. The composer needs that capability: a person sees the turn going wrong and wants to say "do this instead" without two steps. The OMP command does not fit cedian's turn machinery in three ways:

1. OMP's abort keeps its queue. A queued steer or follow-up would run after the new prompt, the race ADR-0059 decision 2 removed from Stop.
2. It answers with an acknowledgement only, and the new run completes with a `prompt_result` keyed by the command's id. The link's worker runs one prompt at a time through `prompt_and_wait` and would see the new run as one OMP started on its own. The stopped prompt's own return would then reset the link's state while the new run streams.
3. OMP starts the new run before cedian sees the stopped one end, so the stopped turn's review turn (ADR-0006) would still be open when the new run's edits arrive.

## Decision

1. **Stop and send is cedian's composition, not OMP's command.** It runs Stop's `abort_and_restore_queue` unchanged. The queued messages go back to the composer as Stop puts them back, and are never resent. Then the composer's text, taken whole when the person pressed, goes out as a plain `prompt`.
2. **When it goes out.** The next prompt waits until the stopped prompt's call has returned *and* the stopped run is over (OMP's `session_settled`, or the call's error path ending the turn). A stopped prompt usually returns as a success (`prompt_result` with status `aborted`), so neither signal alone is enough. Waiting for both means the review turn of the stopped run ends before the new one begins, and the link's worker is free when the prompt reaches it. A settle that arrives while the new prompt is still queued belongs to the stopped run and is ignored.
3. **A failed stop sends nothing.** If the abort or the audit fails, the turn ends as failed, and the held text goes back to the composer with the reason. The same happens on Restart and when OMP stops.
4. **The surface.** A "Stop and send" button beside Stop while a turn runs, and `cmd-shift-enter` (`ctrl-shift-enter` on Linux and Windows) in the composer, the chord upstream Zed binds to `agent::SendImmediately`. While the text waits, the panel shows it as "next: …". Plain Enter while streaming stays a follow-up, and Enter while stopping is still refused.
5. **Parity.** The `abort_and_prompt` row becomes `unused by decision (ADR-0064)`, and S9 has no `deferred: S9` row left.

## Consequences

- The capability is native without an OMP change, and without a second abort path racing Stop's epoch.
- The new prompt starts a little later than OMP's command would start it, after the stopped run settles. Two ordered events, not a timer, decide when.
- If OMP grows an `abort_and_prompt` that withdraws its queue and reports the stopped run's end before the new run starts, this decision can be revisited. The surface would stay the same.
- A plain Send pressed in the gap between the stopped run's settle and its call returning still meets the link's eager phase reset (`OmpLink::send`). Stop and send avoids that gap by waiting for the return. Plain Send's gap is a ROADMAP follow-up.
