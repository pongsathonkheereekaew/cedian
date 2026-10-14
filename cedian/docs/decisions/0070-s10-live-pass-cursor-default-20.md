# ADR-0070: S10 records its live fixtures on `cursor/default`, at most 20 turns

- **Status:** Accepted (owner, 2026-10-14, in chat: "ใช้ cursor auto 20 รอบได้เลยครับ"; Cursor's "Auto" is `cursor/default` in OMP 18.8.6)
- **Date:** 2026-10-14
- **Rule text:** `docs/plans/s10-omp-parity.md` "Recordings"; the fork's `script/cedian-bench/run.sh` (`MAX_TURNS`, `BENCH_PASS`, `BENCH_MODEL`)
- **Builds on:** ADR-0060 (a pass's cap and the harness that enforces it), ADR-0066 (the reviewer records on its own model), ADR-0067 (the s3 lanes on `cursor/default`)
- **Refines:** ADR-0061 for S10's pass only: its model is `cursor/default`, not the pinned model

## Context

Several S10 units need frames that only a model turn produces: OMP's todo tool, goal mode, `btw`, subagent transcripts and IRC messages, plan mode. Without a turn their fixtures are hand-made and prove cedian's pipeline, not OMP. In u12b the pinned model would not follow multi-step lanes, and `cursor/default` did (ADR-0067).

## Decision

1. **One S10 pass, `s10`, capped at 20 model turns** (`BENCH_PASS=s10 MAX_TURNS=20`), on `BENCH_MODEL=cursor/default`. The harness keeps its own ledger per pass, so no turn is shared with `u12` or `u12b`.
2. **It records fixtures only.** Benchmark numbers stay on the pinned model (ADR-0061, ADR-0067), so no B1–B10 run is spent here.
3. **Each unit spends from the same 20**, after its code has settled. A take starts only when its worst case fits what is left; the first one that does not ends the pass for that unit, and its frames stay hand-made and named in ROADMAP Follow-ups.
4. **The reviewer** keeps ADR-0066's rule: the pinned binary and its own model.

## Consequences

- S10's turn-only features can be proved against OMP itself, not against cedian's guess of OMP's frames.
- A fixture recorded on `cursor/default` names that model; a replay checks it as ADR-0067's s3 lanes do.
- The pass makes model calls through the owner's Cursor login and writes lock files and logs in `~/.omp`.
