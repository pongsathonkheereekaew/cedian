# ADR-0067: u12b records the s3 lanes on `cursor/default`

- **Status:** Accepted (owner, 2026-10-14, in chat: "ใช้โมเดล auto ของ cursor แทนได้ไหม", then "ทำตามแผนเลยครับ")
- **Date:** 2026-10-14
- **Rule text:** the fork's `crates/cedian_cli/tests/replay_cli.rs` (`implementer_model`, `S3_IMPLEMENTER_MODEL`); `BENCH_MODEL=cursor/default` for the two `record` steps
- **Amends:** ADR-0061 (the pass pins its model), for the `s3review` and `s3same` fixtures of pass `u12b` only
- **Builds on:** ADR-0065, ADR-0066

## Context

After ADR-0066 the reviewer can record, but the pinned model would not follow the s3 lanes' steps. In `s3review` it called `read` 24 times and never edited or asked for a review. In `s3same` it never edited `add.py`. Six of `u12b`'s 20 turns went to these two lanes and none were kept.

A fixture is a recording of OMP's frames for a replay. It doesn't produce a benchmark number, so its model doesn't affect comparison with S2's baseline. OMP 18.8.6's catalog lists `cursor/default` (Cursor's Auto), and the owner's OMP is logged in to Cursor.

## Decision

1. **Pass `u12b` records `s3review` and `s3same` with `BENCH_MODEL=cursor/default`**, at most 4 turns. The pass's wrapper pins that model for the implementer, and the fixtures are checked against it. The independent reviewer keeps its own pinned model (ADR-0066). The same-model lane's reviewer runs on the implementer's model, as that lane requires.
2. **A replay expects the model its fixture ran on** (`S3_IMPLEMENTER_MODEL`). It changes only together with the fixtures that ran on it.
3. **B1–B10 stay on the pinned model** (ADR-0061). A `run` on another model is refused, as before.
4. **If neither lane records in its 4 turns**, the s3 fixtures stay hand-made and `u12b` ends there.

## Consequences

- The s3 fixtures, if they record, are the first fixtures not on the S2 model. Their replays assert their own model, so nothing compares them with a benchmark run.
- These turns use the owner's Cursor quota, not opencode's.
