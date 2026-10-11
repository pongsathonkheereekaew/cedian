# ADR-0060: The live pass's cap is 46 turns

- **Status:** Accepted (owner, 2026-10-11, in chat: "ยกเพดานเป็น 46")
- **Date:** 2026-10-11
- **Rule text:** `docs/plans/s9-real-app.md` U12; the fork's `script/cedian-bench/run.sh` and `script/cedian-bench/cap.py`
- **Supersedes:** ADR-0059 decision 4 (the 45-turn cap). Everything else in ADR-0059 stands.
- **Builds on:** ADR-0058 decision 5, ADR-0059

## Context

ADR-0059 set the cap at 45 because the pass looked like 38 turns (the 15 record lanes at 18 turns, then B1–B10 in both mandate arms at 20) plus a 4-turn smoke. Two things on 2026-10-11 made the smoke cost more:

- The smoke's five checks are 6 turns, not 4: resume and the `eval` approval each need a model turn of their own, alongside the recorded `bash` approval, Stop with a queued message, and an edit's before-text.
- The smoke found two record lanes that could not produce a fixture their replay accepts (a refused steer that cannot happen under cedian's `--no-extensions` spawn, and a recording that carries frames but not OMP's file writes). Repairing them cost four turns: two re-recordings, one attempt in which the model read the file and answered without calling the tool, and the CLI lane's redo.

Both lanes have fixtures now. The rest of the pass is 16 lanes (16 turns) and both arms (20 turns), which is 36 turns against 35 left of 45.

## Decision

The live pass is capped at **46** model turns (owner). The harness enforces the number rather than trusting it:

- `script/cedian-bench/run.sh` defaults `MAX_TURNS` to 46 and refuses more;
- `script/cedian-bench/cap.py` has `CEILING = 46`;
- a pass's ledger keeps the `max` in its header, so a pass opened at another cap keeps that one, and two passes never share a ledger (`BENCH_PASS`).

The 36 remaining turns fit exactly: 46 minus the 10 the smoke and the two re-recordings booked. There is no spare turn, so a step that fails (a model answering without calling the tool costs its lane its turn) ends the pass one lane early: the harness cuts from the end of its own order, `s2profile` first, and reports a partial exit (ADR-0058 decision 5).

## Consequences

- The cap is a ceiling, not a target: a step starts only when its worst case fits, and the first step that does not ends the pass.
- A re-run of the lanes or the arms needs its own `BENCH_PASS` and a cap that fits what remains of it.
- The pass's arithmetic lives in `docs/plans/s9-real-app.md#progress`, where each step's booked turns are recorded.
