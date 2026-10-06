# ADR-0023: Product scope — what cedian is

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) header; [ROADMAP.md](../ROADMAP.md) "Execution order"

## Context

A review on 2026-10-06 asked the owner to pin down what cedian is before implementation continues. Without this, every slice re-argues scope: editor depth vs agent features, which languages matter, whether completion exists, how autonomous the agent is, how much OMP gets forked, and when the app itself gets built.

## Decision

**cedian is an agentic IDE built in Rust + GPUI (a Zed fork), with OMP as its only harness: OMP decides, cedian executes, renders, and verifies.**

| Question | Decision |
|---|---|
| Identity | Daily IDE **and** agent cockpit, equally. The editor must be good enough to replace the current IDE; verify/review/workers must be first-class. |
| Work it must serve | Rust/systems, Web (TS/JS), iOS/Swift, backend/scripts — all four. |
| Inline completion | Keep Zed's existing edit prediction at S9. cedian builds no completion engine and does not route completion through OMP (completion is not an agent, so "one harness" is untouched). |
| Default autonomy | `write` approval mode: read and edit inside the workspace freely; `bash`/`eval` outside the allow-list and every dangerous action ask (ADR-0012, ADR-0020). |
| OMP fork | Minimal. Host tools first (ADR-0022); playbooks and verification profiles are OMP skills (ADR-0025); fork only what cannot be done otherwise (routing native edits into buffers), and upstream it when possible. |
| iOS | After v0.1, extension track. Until then OMP `bash` + `xcodebuild` already work. |
| Order | Prerequisites P1–P3 → **S9a app spike** → P4/P5 → S2 → S3 → S4 → S5 → S9 (v0.1) → S6/S7 (v0.2) → S8. |
| First priority after the spike | Verification and review (S2 → S3): the main difference from other agentic IDEs, and it serves all four kinds of work. Browser evidence (S4) follows directly because web gates need it. |

## Consequences

- S9 risk is retired early by the S9a spike instead of last.
- Feature requests are checked against this table first. A request outside it needs a new ADR, not a quiet addition.
- "Better than Cursor" is claimed only on verification, review rigor, control and safety — not on breadth, extensions or completion quality.
