# ADR-0050: S5's workers are OMP subagents; worktrees stay cedian's

- **Status:** Accepted (owner, 2026-10-08, in chat: "subagent ของ OMP", "Enter = ต่อคิว, ปุ่ม Steer แยก", "ลบ steer กับ list ก่อน"; the rest decided by the agent as the safer option)
- **Date:** 2026-10-08
- **Rule text:** ROADMAP S5 exit and stand-in row C; `docs/plans/s9-real-app.md` U8; ARCHITECTURE §42, §43
- **Refines:** ADR-0038 for S5; ADR-0009 holds unchanged

## Context

The S5 exit says workers on worktrees are "visible (from subagent events) and steerable (a steer reaches the worker's OMP session)". On OMP 18.6.1 (the pin) those are two different things:

- An **OMP subagent** is a session inside the OMP process, started by OMP's `task` tool. It has an id and a session file and runs in the parent's checkout unless `task.isolation.enabled` is on, in which case OMP makes its own isolated copy (a git worktree among others) and merges it back itself. OMP sends `subagent_lifecycle` and `subagent_progress` only after `set_subagent_subscription`, and takes `steer_subagent {subagentId, message}` and `cancel_subagent {subagentId}` mid-turn.
- A **cedian worker** is a git worktree plus a row in `cedian_worker`'s registry. It has no OMP session. `cedian worker steer` changes the row's status and appends a note; nothing reaches an agent.

So "the worker's OMP session" does not exist. The only real steer is `steer_subagent`.

## Decision

1. **A worker in the S5 exit is an OMP subagent** (owner). The app subscribes at level `progress`, shows each subagent under the `task` tool card that started it (`parentToolCallId`) with its status, and gives each running one Steer and Cancel through `steer_subagent` and `cancel_subagent`. Level `events` (every subagent token over the pipe) is not used; a transcript is read on demand with `get_subagent_messages`.
2. **Worktrees stay cedian's** (ADR-0009). The app registers `cedian_worktree_request` with OMP, and the request takes the ADR-0033 brief; one without it is refused with no tree created. The spawn overlay pins `task.isolation.enabled: false` so OMP never makes trees of its own. Under `policy = "omp"` the user's OMP config decides, as with `computer` (ADR-0035), and the badge says so.
3. **Send during a turn queues; Steer interrupts** (owner). Enter while a turn runs sends `follow_up`, shown as a queued chip and run after the turn. A separate Steer button sends `steer` into the running turn.
4. **Cancelling a subagent is the person's act and is audited.** A Cancel writes an audit row. OMP answering `cancelled: false` (the subagent had already ended) shows "already ended", not an error. A failed steer ("Subagent not running") shows on the row.
5. **CLI stand-ins** (owner). `cedian worker steer` and `cedian worker list` are deleted: the app's steer and subagent view replace them. `spawn`, `preview`, `merge-back` and `remove` stay until the app has merge-back and remove for worktrees.

## Consequences

- The S5 exit is read with decision 1: "a steer reaches the worker" means a `steer_subagent` the fake-omp replay checks field by field.
- No field in OMP's subagent events names a cedian worktree, so the app cannot yet show which subagent works in which tree. Tying them, `OutOfScope` edits, `stuck`, cleanup classification and merge-back or remove in the app are named in ROADMAP Follow-ups if U8 does not reach them.
- `OMP_PARITY.md`: `set_subagent_subscription`, `get_subagent_messages`, `steer_subagent`, `cancel_subagent`, `subagent_lifecycle`, `subagent_progress`, `steer`, `follow_up` and `task.isolation.enabled` move to native or pinned in the change that builds them; `subagent_event` stays unused by decision 1.
