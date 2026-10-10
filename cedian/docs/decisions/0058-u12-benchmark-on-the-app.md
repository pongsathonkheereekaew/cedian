# ADR-0058: The S9 benchmark: the S2 model, both mandate arms once, debug frame time against 16.7 ms p99

- **Status:** Accepted (owner, 2026-10-10, in chat: "ตัวเดิมกับ S2", "รอบเดียว ทั้งมีและไม่มี pstack", "debug ติดป้าย inconclusive", "p99 ≤ 16.7 ms (60 fps)", "เริ่มได้เลย หยุดที่ 40 เทิร์น", "เปิดไว้ เหมือนใช้จริง", "เก็บทำมือไว้พร้อมหมายเหตุ", "ปิด 2 แถวนั้น ใช้ต่อ"; the rest decided by the agent as the safer option)
- **Date:** 2026-10-10
- **Rule text:** `docs/plans/s9-real-app.md` U12; ROADMAP S9 exit (the ADR-0026 benchmark re-run on the app)
- **Builds on:** ADR-0026, ADR-0037, ADR-0051 decision 6, ADR-0057 decision 8

## Context

The S2 baseline ran the release CLI on OMP 18.6.1, using `opencode-go/muse-spark-1.3-contributor`, one run per task. It passed 9 of 10 tasks with 0 false-done, and cedian's median overhead was 28 ms per turn. ADR-0037 sets these budgets:

- false-done 0;
- cedian overhead: median under 1 s, and no turn over 3 s;
- a task more than 25 % slower on the same model counts as a regression.

There is no frame-time budget. GPUI measures frame durations, but nothing in cedian reads them. The app has no headless prompt mode. A release app build needs about 30 GB of free disk, and the machine has about 12 GB.

## Decision

1. **The model is S2's**, `opencode-go/muse-spark-1.3-contributor` on the pinned OMP 18.6.1, so the 25 % rule applies (owner).
2. **One run per task, in two arms** (owner). B1–B10 run once with the owner's pstack mandate (`~/.omp/agent/APPEND_SYSTEM.md`) and once without it, through `--append-system-prompt ""`. The arm without the mandate is the one compared with S2. A single run is reported as such, and the benchmark checklist names it as a limit.
3. **B1–B10 run on the headless CLI**, as S2 did. The app is measured separately, over warm turns: per-turn cedian overhead, using the CLI's timing rows, and frame time while a reply streams.
4. **The frame-time budget is p99 at or under 16.7 ms while streaming** (owner). U12 measures it on a debug build and labels it `inconclusive` (owner). The release measurement comes with U13's release build.
5. **The live pass** (owner: start now). It stops after 40 model turns and is reported as a partial exit if the cap is hit. It also re-records the hand-cut fixtures, with the mandate on, as in real use (owner). cedian never writes the owner's `~/.omp` config. It records hashes of that config and of `APPEND_SYSTEM.md` before and after the pass, and lists new files in `~/.omp/run` and `~/.omp/logs` without deleting any. Any secret in a recording is scrubbed in code before it is committed, as in U4.
6. **The login frames in `parity_core` stay hand-made**, and a note in the test says so (owner).
7. **b4 and b10 stay.** They run on `~/cedian`'s old history, where every file they need still resolves. The ROADMAP rows that called them broken are closed (owner).

## Consequences

- The S9 benchmark is valid against S2 for the without-mandate arm. Whether the mandate helps is answered once, with a single run.
- Frame time is not settled until U13. S9's benchmark item is partial until then.
- The fixtures carry what the mandate leads OMP to do. A change to the mandate can change future recordings.
