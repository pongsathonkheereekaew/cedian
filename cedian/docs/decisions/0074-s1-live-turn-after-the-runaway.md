# ADR-0074: S1's live turn gets its own 2-turn cap, usable only behind a watchdog

- **Status:** Accepted (owner, 2026-10-14, AskUserQuestion "จะปิด S1 อย่างไรครับ (pass s10 ใช้ไป 25/20 และปิดแล้ว)?", answer "โควตาใหม่ 2 turn (แนะนำ)". On `stop`, the owner asked "แนะนำหน่อย cursor ทำแบบไหน"; the agent recommended asking, said the code already does, and the owner did not object. Decision 5 records that recommendation.)
- **Date:** 2026-10-14
- **Rule text:** ROADMAP S1; the fork's `script/cedian-bench/run.sh` (`record`, `MAX_TURNS`, `BENCH_PASS`); the fork's `cedian/OMP_PARITY.md` row `debug`
- **Builds on:** ADR-0060 (a pass's cap and the harness that enforces it), ADR-0070 (the `s10` pass on `cursor/default`)
- **Refines:** ADR-0073 decision 5 (adds `stop`) and decision 6 (the live turn comes from this cap, not ADR-0070's 20)

## Context

S1's last exit item is one live `cursor/default` turn in which the model drives a breakpoint round trip (ADR-0073 decision 6). On 2026-10-14 that turn ran in the `debug_live` record lane and did not finish. The lane's test binary is `harness = false`, so its own `main` runs. Zed's environment loader re-executed that binary as `<exe> --printenv` in a login shell, in a new session (`setsid`), and `main` ran the record scenario for any argument. Each copy started another OMP process and another model turn. The harness checks the cap before a step and books the spend after it, so nothing stopped the copies while they ran. The worker killed the tree by binary name after 25 turns, against the `s10` pass's 20.

All 25 taps show the model calling `cedian_debug` and never OMP's `debug`. Every copy got the same prompt, so that is one observation repeated, not 25. No fixture was written, because the process was killed before its `write_fixture`.

Fork PR #12 made `live_turn` and `debug_round_trip` exit on arguments they do not own. Its review found six more record binaries without that guard (F1) and no bound on a step while it runs (F2).

## Decision

1. **The `s10` pass stays closed at 25 of 20.** No unit spends from it again, so U2 to U12 get no live turns until the owner grants a new pass.
2. **S1 gets its own pass of 2 turns** on `BENCH_MODEL=cursor/default`, under its own `BENCH_PASS` name so its ledger starts at zero. Only the `debug_live` lane may spend from it.
3. **The pass opens only after both guards are merged into `cedian/s9`.** Every `harness = false` record binary exits on arguments it does not own (F1). `run.sh` watches the tap while a step runs, and once the count passes the step's reservation it kills by executable path and by the OMP pids named in the tap, books the real count and fails the step. A wall-clock alarm bounds the step as well (F2). A process-group kill is not enough, because the loader's child runs in its own session.
4. **One take, a second only with a known cause.** The first take spends 1 turn. A second runs only if the first failed for a cause that has been found and fixed, and only if its worst case fits what is left.
5. **`stop` asks on a session the person started.** It ends the program under debug and drops its state, which is more than `continue` does, and `continue` already asks. On a session the agent started, `stop` is allowed. This adds `stop` to ADR-0073 decision 5. It is the agent's recommendation under the owner's question and matches the code as built.
6. **Any overspend is reported to the owner at once** and ends the pass.

## Consequences

- S1 can close on a real OMP fixture and a replay test in CI, at a cost of 1 or 2 model calls through the owner's Cursor login.
- The guard and the watchdog protect every later record lane, not only S1's.
- S10's turn-only frames (todos, goal mode, `btw`, subagent transcripts, plan mode) have no budget now. Their fixtures are hand-made and named in ROADMAP Follow-ups until the owner grants a pass.
- OMP still writes lock files and logs into `~/.omp` during a take, even with a temporary `HOME`. The 25 empty lock files from the runaway are listed for the owner to delete; the agent does not delete in `~/.omp`.
