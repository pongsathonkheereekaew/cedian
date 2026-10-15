# The S9 pass on the app, OMP 18.8.6 (2026-10-11)

The ADR-0026 benchmark re-run for S9, per [ADR-0058](../../decisions/0058-u12-benchmark-on-the-app.md), [ADR-0059](../../decisions/0059-omp-pin-18-8-6.md), [ADR-0060](../../decisions/0060-live-pass-cap-46.md) and [ADR-0061](../../decisions/0061-pass-pins-its-model.md). The S2 baseline is `../2026-10-07/`.

## What ran

- The owner's machine (8 cores, 8 GB), the same box as the baseline.
- The release CLI `run.sh run` builds, one run per task per arm, `policy = "omp"` per task as S2 ran.
- Model `opencode-go/muse-spark-1.3-contributor`, pinned into every spawn by the harness wrapper; the person's own `modelRoles.default` was another model that day (ADR-0061).
- OMP 18.8.6, the pin. S2 ran 18.6.1, so a difference cannot be split cleanly between cedian and OMP (ADR-0059 decision 5) and the 25 % rule is a warning, not a verdict.
- One run per task per arm (ADR-0058 decision 2), so every row below is one sample.

## The compared arm: no mandate (`compare-off.txt`)

8 of 10 predicates pass. Against S2, four tasks are faster (b1 0.41x, b4 0.54x, b7 0.81x, b9 0.44x) and three are slower (b2 1.58x, b5 2.21x, b8 1.06x); the first two cross the 25 % warning line. In both of those, the extra wall time is OMP's own long inference (`omp_ms` grows by 180 s and 83 s) while cedian's part of the turn stays at 31 ms and 30 ms, so a cedian regression does not explain it. b10 has no S2 verdict to compare with (`hidden_tests_broken` there) and passes now.

cedian's own overhead per turn: **median 31 ms, max 38 ms over 9 turns**, against ADR-0037's budgets (median under 1 s, no turn over 3 s). S2 measured 28 ms.

b6 fails at the fixed 600 s one-shot timeout in both arms, and S2's b6 failed the same way (ROADMAP's row).

b3's `fail` and its `false_done` are measurement artifacts, not a gate failure: the predicate's own test (`crates/cedian_workspace/tests/fake_lsp_bridge.rs`) timed out waiting 120 s for its fake LSP's `initialize`, and the harness's `claimed_done` is a regex over the reply's first block, where the word "Done." appears. The model's patch is the change the task asks for. Reported as **inconclusive**.

## The mandate arm (`on`): `compare-on.txt`

Informational: ADR-0058 decision 2 compares the arm without the mandate with S2. Every predicate matches the other arm's.

cedian's overhead per turn is **5026 ms median, 5066 ms max over 9 turns**, over ADR-0037's 3 s ceiling, and the number is **inconclusive**: all of it is `post_ms`, the work after OMP's prompt call returns, and the mandate arm's turns do far more (its own model time is up to 2x the other arm's), so the post work grows with what the turn changed. Which part of the post work dominates is not measured; profiling it needs a run this pass does not have.

## The app's own runs (U12e)

The app driving the pinned OMP itself, not the CLI: one approval turn and four warm turns in one session, mandate off, debug build. Rows: `app/app-timing.jsonl` and `app/app-frames.jsonl`.

- cedian's overhead per turn: **median 147 ms, max 300 ms over 5 turns** (the approval turn included; the four warm ones are 300, 147, 107 and 185 ms). Inside ADR-0037's budgets. The CLI arm's 31 ms is the same code without a panel: the difference is the panel's context snapshot and its post-turn work over the reply.
- Frame time while a reply streams: one row per turn, p50 2.9 to 4.9 ms and p99 7.8 to 10.6 ms, so inside ADR-0058 decision 4's 16.7 ms p99 on this **debug** build, which the decision labels **inconclusive**.

### The release build (`app-release/`)

The same lane on the release build (`cargo test --release -p cedian_panel --test live_approval`), five turns, OMP's own approval mode so no dialog is needed: GPUI's debug bounds are a debug-build feature, so no test click reaches a dialog there.

- cedian's overhead per turn: **median 167 ms, max 227 ms over 5 turns**, next to the debug run's 147 ms.
- Frame time while streaming: p50 0.45 to 0.64 ms and **p99 1.0 to 1.9 ms, max 1.9 ms**. ADR-0058 decision 4's budget of 16.7 ms p99 **is met on the release build**: the debug run's inconclusive number was 4 to 8 times it.

## What the numbers do not say

- A single run per task cannot separate the model's own variance from a real change; the two arms and S2 are three samples of the same task, not a distribution.
- The app's rows are five trivial prompts; they measure the panel's own cost per turn, not a task's.
- b3 and b6 are not clean passes, so the false-done count for this pass is 1 in each arm by the harness's regex, 0 by inspection.
