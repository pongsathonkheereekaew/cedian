# ADR-0068: S9 closes as a partial exit; Tab completion moves to S10

- **Status:** Accepted (owner, 2026-10-14, in chat: "ปิดแบบ partial exit" and "ย้ายไป S10")
- **Date:** 2026-10-14
- **Rule text:** README's S9 row; `docs/plans/done/s9-real-app.md` "Exit evidence"; ROADMAP "Cursor workflow coverage" and Follow-ups
- **Builds on:** ADR-0038 (S4 and S5 close inside S9), ADR-0057 (the parity bar and S10), ADR-0034 (the rename the owner approved on 2026-10-14)

## Context

Units U1–U13 of S9 are done, and so is the second live pass (`u12b`). Three things keep S9 from a clean ✅:

1. **S5's last exit item waits on OMP.** OMP's subagent events name no worktree, so the app can't show which subagent works in which tree (ADR-0050, ADR-0063's interim).
2. **Tab completion.** The coverage table promised it from Zed's edit prediction in S9. The rename's `disable_ai` default (owner, 2026-10-14) turns that off, and Zed's own predictor needs the Zed account sign-in the rename hides.
3. **Two benchmark findings wait for another benchmark pass.** The mandate arm's ~5 s of post-turn work, and b3's verdict coming from its own fake LSP. `u12b` spent its turns on fixtures (ADR-0065).

AGENTS.md's timebox rule says to commit what is green and name the rest.

## Decision

1. **S9 closes as a partial exit**, ◐ in README, with the gaps above named. The plan moves to `done/`, and work goes on in S10.
2. **Tab completion moves to S10.** `disable_ai` stays on. S10 picks a predictor not tied to a Zed account, such as an OMP-routed model, Copilot or a local model, and turns it on without bringing Zed's agent back.
3. **Plan mode and the usage and cost meter** were moved to S10 by ADR-0057's parity bar. The coverage table now says so.
4. **The gaps move.** S5's worktree attribution waits on OMP and is settled when OMP names the worktree. The benchmark findings, the record lanes' predicates and the parity core's remaining gaps go to S10.
5. **S4 closes** (✅), on S9 U7's and U9's evidence in the app. **S5 stays ◐** until OMP names the worktree.

## Consequences

- README's S9 row no longer lists open units. Its open items are the named gaps, and the next work is S10.
- The coverage table no longer promises S9 items that ADR-0057 or this decision moved.
