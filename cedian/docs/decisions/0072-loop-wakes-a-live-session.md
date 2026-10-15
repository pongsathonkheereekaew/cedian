# ADR-0072: A loop wakes a live session, armed by the agent, with the session's own permissions

- **Status:** Accepted (owner, 2026-10-14, in chat: "ให้สิทธิ์เหมือน pstack ทำเลยได้ไหม ให้ agent รีวิวเองและแก้เองแบบที่ poteto ทำ", over the agent's recommendation to refuse loops under `policy = "omp"` and to let only the person's `/loop` arm one)
- **Date:** 2026-10-14
- **Rule text:** ROADMAP S7 (scope and exit); ARCHITECTURE §54 rules and the policy-source list; the user's `~/.omp/agent/pstack/omp-tools.md` `/loop` row, outside this repo
- **Supersedes in part:** ADR-0035 decision 5 ("never for unattended runs"), for loop ticks only. S7's scheduled automations and reviewers keep the cedian profile
- **Builds on:** ADR-0019 (local-only automations), ADR-0022 (host-tool-first), ADR-0013 (`ask` is a lease), ADR-0027 (no TS), ADR-0034 (OMP decides, cedian executes), ADR-0039 (independent review), ADR-0051 and ADR-0052 (pstack as the user's OMP config)

## Context

pstack 0.15.24 (`cursor/plugins` `1e5c90a`, installed 2026-10-14) uses Cursor's `/loop` in five places:

- **autonomous-run:** the agent picks its own wake ("an event to watch gets a watcher subagent that wakes you, with a long time-based heartbeat as fallback") and drives to a predicate it defines. It fixes what it finds on the way and surfaces only irreversible actions or real product calls.
- **autopilot-full and multi-phase-plan:** an audit tick is armed as `/loop 1h` "on the operator's go".
- **visual-parity:** "`/loop` per component until the diff is zero".
- **README:** "works extremely well with cursor's `/loop`".

In Cursor, every tick runs with the same permissions as the chat that armed it.

Facts on OMP 18.10.4, 2026-10-14:

- **No built-in loop.** OMP has no loop command or scheduler. The `task` tool's background jobs and `wait` keep running only inside one turn.
- **What can wake an idle session:**
  - the RPC client, with `prompt` (or `follow_up` while a turn runs);
  - an OMP extension, through `sendUserMessage` or `sendMessage({ triggerTurn: true })`. cedian cannot use this: an extension is TypeScript (ADR-0027), and a loop inside one would be invisible to cedian, with no history, no caps and no Stop.
- **Stop clears the queue:** Stop's `abort_and_restore_queue` withdraws OMP's queued follow-ups and puts their text back in the composer (ADR-0059).
- **Session switching:** the panel switches OMP sessions inside one process (S10 U1), and "Branch here" makes a child session (ADR-0071).

S7 as written schedules a cron expression that spawns a fresh turn under the cedian profile. That covers none of the following: the session that armed the loop, a stop condition only the agent can judge, an event wake, or the agent arming its own loop.

## Decision

1. **One scheduler, two targets.** S7's scheduler gets a second kind of target: a **loop**, which sends a prompt into a live OMP session, next to the scheduled automation that spawns a fresh turn. A loop is a cedian feature, not a pstack one.
2. **The agent arms its own loops, as in pstack.**
   - **The tool:** the host tool `cedian_loop` (`arm`, `stop`, `status`) arms a loop with no confirmation dialog. An arm names the prompt, the wake, the stop predicate in words, and a duration cap (default 24 h).
   - **The person:** can arm a loop from the composer's `/loop` (offered while OMP has no command of that name) or from the palette.
   - **Notice:** arming shows a notification and a badge with a Stop button.
3. **What wakes a loop.**
   - **Interval:** at least 1 minute.
   - **Event:** an event cedian already watches. That means S6's PR state (CI finished, PR merged, new review comment) and a ref moving in the workspace. Event wakes land with S6.
   - **Before S6:** pstack's watcher stays inside the turn (a background `task` running `watch-pr`), and the loop's interval is the heartbeat fallback.
4. **A tick is a turn in that session.**
   - **Delivery:** a tick is sent as `prompt` when the session is idle and as `follow_up` while a turn runs. At most one tick is queued at a time.
   - **Sleep:** ticks missed while the machine slept merge into one on wake. Nothing wakes the machine (S7's cut stands).
   - **Bound to one session:** a loop is bound to the OMP session id that armed it. It holds its ticks while another session is current in the panel. A branch never inherits it.
5. **Ticks run with the session's own permissions.** A tick gets exactly what an interactive turn in that session gets:
   - the cedian profile, or the user's OMP config under `policy = "omp"` (ADR-0035);
   - strict-wins at the cedian gate;
   - audit rows and the badge.

   Anything OMP or cedian would prompt for interactively it prompts for in a tick too. That prompt holds the turn under ADR-0013, and the loop queues no further tick until it is answered.
6. **The agent reviews and fixes its own work. cedian still judges done.**
   - **Inside each tick:** the agent works as pstack says: it verifies, reviews and fixes what it finds, including the findings of cedian's independent reviewer (ADR-0039), which arrive like any other finding.
   - **Completion:** completion is still `cedian_complete` against the gate floor.
   - **Stopping is not completing:** stopping a loop never completes work, and a loop never lowers a gate.
7. **Stopping and pausing.** A loop ends or pauses at the first of these:
   - the agent calls `cedian_loop stop` with a reason (pstack's "until X");
   - a stop predicate holds (S7's pure predicates over stored state);
   - its duration cap runs out;
   - S7's consecutive-failure limit (default 3) pauses it and notifies the person;
   - the person presses Stop (the panel's Stop, `abort_and_restore_queue`, also pauses every loop of that session, and a withdrawn tick prompt is dropped, not put back in the composer);
   - its session closes.

   A paused loop resumes from its badge, or when the agent arms it again.
8. **Visible.** Each tick is a row in S7's run history: trigger, turn, outcome, and the stop reason on the last one. Ticks notify nobody unless the person is needed. That matches pstack's tick, which reports only a change no earlier message reported.

## Consequences

- pstack's long playbooks run under cedian as Lauren ships them, visual-parity included, since `eval` follows the session's permissions. When `cedian_loop` lands, the `/loop` row in `omp-tools.md` points at it. Until then it names the in-turn watcher.
- **Ticks run unattended with the session's permissions.**
  - **Cedian profile:** an `eval` or exec prompt stalls the loop until the person answers.
  - **`policy = "omp"`:** the user's OMP config approves for them. So a project's yolo config now also covers turns nobody watches. The audit log and the badge are the record.
- **§88 "cedian agent scheduler" still holds.** The agent decides that a wake happens, when, and what it does. cedian runs the timer and delivers the prompt (ADR-0034). §54's rules say so.
- **S7 grows.** Its exit gains: "the agent arms a loop through `cedian_loop`, it ticks in that session while the machine is awake, it stops on `cedian_loop stop`, and every tick is in the run history." Event wakes need S6. S7's place in the order is unchanged: after DAP and v0.1 (owner rulings 2026-10-14).
- **Supersession:** if OMP ships its own loop or scheduler, ADR-0034 gives that feature the native surface, and this ADR is superseded.
- **Rejected alternatives:**
  - **Cron-only S7 with a fresh turn per run:** it loses the session's context.
  - **A loop inside an OMP extension:** it is TypeScript, and cedian cannot see it.
  - **Requiring the person's confirmation for every agent arm:** it breaks autonomous-run while the person is away.
