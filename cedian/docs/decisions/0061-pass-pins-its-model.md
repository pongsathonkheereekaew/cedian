# ADR-0061: The live pass pins its model in the spawn wrapper

- **Status:** Accepted (owner, 2026-10-11, in chat: "ให้ pass pin โมเดลเองใน wrapper")
- **Date:** 2026-10-11
- **Rule text:** the fork's `script/cedian-bench/run.sh` (`wrapper`, `model_check`); `docs/plans/s9-real-app.md` U12a
- **Builds on:** ADR-0058 decision 1 (the model is S2's), ADR-0058 decision 5, ADR-0059 decision 3

## Context

ADR-0058 decision 1 needs the pass to run the model S2's baseline ran, `opencode-go/muse-spark-1.3-contributor`, or the 25 % comparison means nothing. `model_check` enforced that by reading the person's OMP `modelRoles.default` and refusing any other value.

On 2026-10-11 that default became `commandcode/deepseek/deepseek-v4.1-flash`, and every step of the pass refused. The default is also the person's own setting for their other work, and the pass may not write OMP's config (ADR-0058 decision 5).

## Decision

The pass pins its model where it spawns OMP. `wrapper` adds `--model $BENCH_MODEL` to every `--mode rpc` spawn in both mandate arms, so the model the pass measures is S2's whatever the person's default is. `model_check` no longer compares the default: it requires `$BENCH_MODEL` in OMP's own catalog, and prints both the pinned model and the person's current default, so the difference is visible rather than hidden. The gates that already read what OMP reported stay: a recording must still report the pinned model (`crates/cedian_fake_omp/src/recording.rs`), and `cap.py models` still counts what a step's OMP actually used.

## Consequences

- The person's `modelRoles.default` is untouched by the pass, and a step can no longer be refused or silently measured on another model because of it.
- The pinned model is in the wrapper's argv, so a tap file or a log shows what a step ran on.
- S2's own run used whatever default it had, so the comparison rests on the pinned name agreeing with the baseline's, not on the mechanism that selected it.
