# ADR-0065: A second live pass, capped at 20 turns on the pinned model

- **Status:** Accepted (owner, 2026-10-14, in chat: the live turns the plan needs, at "20 เทิร์น โมเดลเดิม")
- **Date:** 2026-10-14
- **Rule text:** `docs/plans/done/s9-real-app.md` U12; the fork's `script/cedian-bench/run.sh` (`MAX_TURNS`, `BENCH_PASS`)
- **Builds on:** ADR-0058 (the benchmark on the app), ADR-0060 (the first pass's cap and the harness that enforces it), ADR-0061 (the pass pins its model)

## Context

The first live pass (`u12`, 2026-10-11) spent its 46 turns. What U12 left open needs model turns again. Hand-cut fixtures are still hand-cut (`workflow.jsonl`, `review.jsonl`, `inline_edit`, the CLI replays' `edit` frames). Three record lanes' takes don't satisfy their own assertions. And B1–B10 have one run per task (ROADMAP Follow-ups). The owner allowed a second pass and chose its size.

## Decision

1. **A new pass, `u12b`, capped at 20 model turns.** It runs with `BENCH_PASS=u12b MAX_TURNS=20`. The harness already refuses more than its cap and keeps one ledger per pass (ADR-0060), so `u12` and `u12b` never share turns. `cap.py`'s ceiling of 46 stays the most any one pass may ask for.
2. **The same pinned model as `u12`** (ADR-0061), so the new numbers compare with the old ones directly.
3. **Order.** First the hand-cut fixtures, re-recorded (mandate on). Then the lanes whose takes failed their assertions, after their predicates are fixed to the task and not to one take's text. Then repeat runs of B1–B10 with what is left. As in ADR-0060, a step starts only when its worst case fits, and the first step that doesn't ends the pass.
4. **It runs on settled code**, after the S9 code units of 2026-10-14 (Stop and send, the rename) have landed, so recordings don't need re-cutting.

## Consequences

- The fixtures the ROADMAP lists as hand-made can be replaced by recordings, and those rows deleted.
- 20 turns don't cover two full mandate arms again, so B1–B10 gain repeat runs only for the tasks that fit. Each result says which runs it rests on.
