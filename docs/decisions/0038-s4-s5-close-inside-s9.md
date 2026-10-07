# ADR-0038: After S3 comes S9; S4 and S5 close inside S9

- **Status:** Accepted (owner, 2026-10-07, in chat)
- **Date:** 2026-10-07
- **Rule text:** [ROADMAP.md](../ROADMAP.md) "Execution order", stand-in rows B, C, D
- **Refines:** ADR-0023 (execution order)

## Context

The execution order ran S2 → `cedian.toml` → P8 → S3 → S4 → S5 → S9. S4 and S5 were built early and are both partial. What they still lack needs the app:

- **S4.** A monotonic frame seq across captures needs one long-lived Chrome shared with OMP. Stand-in row D says that Chrome arrives at S9.
- **S5.** Workers must be visible from subagent events and steerable through their own OMP session. Row C names the subagent view and a live steer, which are app surfaces.

Closing either headless first means building more stand-in code that S9 deletes. The ROADMAP's own rule says never accumulate untested layers. Building a layer only to delete it is the same cost.

`cedian_dap` (row B) has no dependent crate and no binary reaches it. Row B already deletes it at S9.

## Decision

1. **Order after S2:** S3 review agents → S9 real app (= v0.1). S6, S7 and S8 keep their places after v0.1.
2. **S4 and S5 close inside S9.** Their exits are unchanged. They are checked through the app, alongside the S9 exit. Until then they stay partial in README with their gaps named.
3. **No new headless work on rows C and D**, except a fix for a correctness or safety bug.
4. **`cedian_dap` is frozen until S9.** No new work in it. It is deleted at S9 under row B.

## Consequences

- v0.1 still binds S0–S5. S4 and S5 reach ✅ only when S9 does.
- S3 is the last headless slice before the app.
- If S9 slips, S4 and S5 slip with it. That is accepted. Their headless gaps would otherwise be closed with code S9 throws away.
