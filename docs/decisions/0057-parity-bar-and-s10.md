# ADR-0057: S9 closes on a named parity core; the rest of OMP parity moves to a new slice S10

- **Status:** Accepted (owner, 2026-10-09, in chat: "เพิ่มสถานะ เลื่อนไป slice ที่ระบุ", must be native in S9: "Login/onboarding, เลือกโมเดลและ thinking, แจ้งเตือน (toast) ของ OMP, คำสั่งคิว + compaction/retry", "slice ใหม่ S10 OMP parity", "รวมกับ U12 รอบเดียว", "คงเดิม + บอกชื่อ hunk ที่ข้าม", "หา omp ตัวที่ตรง pin ก่อน ไม่เจอก็เตือน", "ลบ cedian_apply_edit และ buffer.rs"; the rest decided by the agent as the safer option)
- **Date:** 2026-10-09
- **Rule text:** ROADMAP S9 exit and S10; `docs/plans/s9-real-app.md` U11; the fork's `cedian/OMP_PARITY.md` status vocabulary
- **Refines:** ADR-0034 (parity rule), ADR-0038 (S9 = v0.1)

## Context

U11's bar was that every row of `OMP_PARITY.md` is native, gated with a working opt-in, or blocked upstream. On 2026-10-09 the ledger had 98 rows and about 74 missed that bar, mostly features not built yet: the session tree, branching, plan mode, voice, `btw`, word prediction, goals and todos. Building them all would take roughly 15 to 25 sessions, against U11's box of one. The parity test checks only that every OMP name appears in the ledger, not its status. cedian also spawns the first `omp` on PATH without checking its version. On this machine that is Homebrew's 18.7.0, not the pinned 18.6.1.

## Decision

1. **A row may be deferred to a named slice** (owner). The allowed statuses are:
   - `native`;
   - `gated` (a working opt-in);
   - `upstream-blocked` (with a link to the upstream issue or request);
   - `pinned`;
   - `unused by decision` (with its ADR);
   - `internal` (runtime plumbing with no surface of its own);
   - `reviewer` (used only by the reviewer process);
   - `deferred: S10` or another named slice.

   The parity test fails on any other status, and on a `deferred` row that names no slice.
2. **New slice S10, OMP parity** (owner). It comes after v0.1 and takes every deferred row. ROADMAP lists it, and the owner sets its order among S6–S8.
3. **Native before S9 closes** (owner):
   - login and onboarding in the app, since the S9 exit is "onboard → prompt → answer with zero terminal";
   - a model and thinking-level picker;
   - one toast surface for OMP's notices, extension errors and config warnings;
   - the remaining queue commands (`abort_and_prompt`, `promote_queued_message`, the queue modes);
   - compaction and auto-retry shown in the app.
4. **The OMP binary** (owner). A `CEDIAN_OMP_BINARY` that is set is an explicit choice and is used as given, so tests and the person's own override always run what they name. Otherwise cedian checks the `omp` on PATH, then `~/.local/bin/omp`, with `--version`, and runs the first one that matches the pin. In both cases, a binary that does not match the pin still runs, and the app shows a warning naming both versions. The headless CLI follows the same rule.
5. **Undo then redo** (owner). An agent hunk that undo and redo turned STALE stays STALE, so it is never reverted over the person's text. Revert turn names each hunk it skipped and why.
6. **Stand-in I is deleted** (owner). `cedian_apply_edit` and `cedian_workspace`'s `buffer.rs` go. The headless CLI lets OMP edit with OMP's own tools. Replays that recorded `cedian_apply_edit` drop those steps, because what they proved is covered by the app's tests from U5 and U9.
7. **Stand-in H is deleted in U11**, as the S9 plan and ADR-0055 decision 10 require. Evidence in the app binds to the Zed buffers' `clock::Global`, so an unsaved edit makes evidence stale. The headless CLI keeps content hashes, because it has no buffers.
8. **Live re-recording joins U12** (owner). The hand-cut fixtures from U8, U9 and U10 are re-recorded in the one live pass that U12's benchmark makes. The owner is told before it starts, because it writes lock files and logs in `~/.omp` and makes model calls.

## Consequences

- The S9 exit reads "every non-extension item of the checklist holds, or is deferred to S10 in the parity ledger".
- S10 holds most of OMP's surface. v0.1 is usable with zero terminal, without OMP's whole feature set.
- A pin bump re-runs the parity test, and a new OMP feature lands as `deferred: S10` or better.
