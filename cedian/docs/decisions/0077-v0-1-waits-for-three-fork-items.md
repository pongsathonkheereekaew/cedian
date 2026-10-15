# ADR-0077: v0.1 waits for the bundled fork OMP, S5's worktree attribution, and project-scoped reads

- **Status:** Accepted (owner, 2026-10-15, AskUserQuestion, each the recommended option: the IDE keeps OMP as the agent's brain ("IDE ที่ใช้ OMP เป็นสมอง"); v0.1 waits for three things ("รอ 3 อย่างก่อน"); S10's live turns are granted one unit at a time ("ให้ทีละ unit ตอนถึง"). On reads the owner asked what others do ("แนะนำแบบไหน เจ้าอื่นทำยังไง"); decision 4 is the agent's recommendation, stated to the owner, taken as the default.)
- **Date:** 2026-10-15
- **Rule text:** [ROADMAP.md](../ROADMAP.md) the identity line, the versions table, the "fully native" checklist, Follow-ups; the S10 plan
- **Builds on:** ADR-0075 (the narrow OMP fork)
- **Refines:** ADR-0057 (`upstream-blocked` for a capability the IDE needs), ADR-0068 (S5 waits on OMP), ADR-0070 and ADR-0074 (live turns)

## Context

v0.1 was to be declared once S1 closed, with S5 waiting for OMP to name the worktree in its subagent events (ADR-0068). ADR-0075 then made OMP a narrow fork, so a capability the IDE needs no longer has to wait for upstream. Three gaps stood out:

- S5: the app cannot show which subagent works in which worktree.
- The app still runs the person's installed upstream OMP, so a fork patch cannot reach it.
- OMP's own read, grep and find tools read anywhere on the machine. In S1's first live take on 2026-10-15 the model read the agent's notes, other repositories and private docs outside the workspace without anyone being asked.

## Decision

1. **cedian stays an IDE with OMP as the agent's brain.** The fork patches OMP only for what the IDE needs (ADR-0075 decision 1); prompts, agent behavior and tools stay OMP's.
2. **v0.1 is declared, in its own ADR, only when all of these hold:**
   - S0 to S5 are ✅ in README;
   - the app runs the fork's OMP, bundled with it (ADR-0075 decision 5), not the person's `omp`;
   - S5's worktree attribution works through a fork patch to OMP's subagent events;
   - reads are scoped to the project (decision 4);
   - a cancelled approval never ends audited as allowed (ROADMAP follow-up, found in S1's review).
3. **`upstream-blocked` no longer covers a capability the IDE needs.** Such a row gets a fork patch, or is cut as `unused by decision` with its ADR. `upstream-blocked` stays for things cedian chooses not to patch.
4. **Reads are scoped to the project, and anything outside asks.** OMP's read, grep, find and similar tools read inside the project without asking. A path outside it asks the person in the app's approval dialog, and the answer is audited. Folders the person allows ahead of time go in the user's `cedian.toml`, per project, never in a file the repository carries, so a repository cannot widen its own read scope (as ADR-0035 keeps `policy = "omp"` there). `bash` keeps its own approvals. Of the tools the agent compared from memory (unverified), Claude Code works the same way; Codex limits writes, not reads; Cursor scopes its file tools to the workspace and gates terminal commands.
5. **S10's live turns are granted one unit at a time.** When a unit's code has settled, the agent asks with the number of turns it needs (usually 1 to 3), and the turn runs through the watchdog of ADR-0074.

## Consequences

- v0.1 comes days later than "right after S1", and with S0 to S5 ✅ it has no known hole in S5, in which OMP it runs, or in what the agent can read.
- The app's build gains the OMP binary (bun and Rust), and its spawn profile runs the bundled path.
- The read scope is new work, likely in the fork (OMP's tools see the path first) with the approval in cedian's gate. Its design comes with the unit.
- Rows that said "waits on OMP" (S5's worktree, inline edit's model role, one driver per session) become fork-patch candidates, ordered by the plan.
