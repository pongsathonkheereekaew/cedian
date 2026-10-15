# ADR-0076: S1's live turns are capped at 3 in total, so the second take keeps its margin

- **Status:** Accepted (owner, 2026-10-15, AskUserQuestion. On how to spend the last turn: "ใช้ตอนนี้กับ cedian_debug (แนะนำ)". Told that the harness would not start the second take, because it reserves the turn plus a margin of one and only one of the 2 was left: "เพดานเป็น 3 (แนะนำ)")
- **Date:** 2026-10-15
- **Rule text:** the fork's `script/cedian-bench/run.sh` and `cap.py` (`admit` reserves the estimate plus a margin of one)
- **Refines:** ADR-0074 decision 2 (S1's cap was 2 turns; it is 3)

## Context

ADR-0074 gave S1 a pass of 2 turns, and said a second take runs only if its worst case fits what is left. The harness counts the worst case as the estimate plus one, so a one-turn take reserves 2.

The first take, in pass `s1live`, spent 1 turn. The model drove the breakpoint round trip on the real `lldb-dap`. Its recording was refused for the owner's user name, and the workflow gate had rejected `cedian_debug` evidence, so the model also read files outside the workspace. Fork PR #15 fixed both causes: the gate counts `cedian_debug` evidence, and the recording scrub redacts any call that names a path outside the workspace.

The second take then needed 2 reserved turns with 1 left, so the harness refused to start it, as ADR-0074 says it should.

## Decision

1. **S1's live turns are capped at 3 in total.** The second take ran as its own pass, `s1live-b`, with `MAX_TURNS=2`: it reserves 1 turn plus the margin of 1, so S1's worst case is 3.
2. **No third take.** If the margin is ever spent, it is booked and reported, and S1 gets no further turns without a new ADR.

## Consequences

- The second take ran on 2026-10-15 and spent 1 turn, so S1 has spent 2 of its 3. OMP picked `cedian_debug` for all 6 debugger calls and never OMP's `debug`. It stopped at `main.c:4`, read `x = 42`, ran to exit, and its evidence passed the workflow gate. The fixture passed the scrub (one `bash` call was redacted as outside the workspace), and it becomes S1's replay test.
- The margin turn was not used and stays unspent.
