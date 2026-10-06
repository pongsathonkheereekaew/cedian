# ADR-0001: cedian is the environment, OMP is the intelligence

- **Status:** Accepted
- **Date:** 2026-10-05
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §1, §2, §79, §88, §90, §91, chapter 3 (stack lock)

## Context

cedian is a personal IDE built on a minimal Zed fork. The risk of an agentic IDE is ending up with two harnesses (an IDE-side agent loop plus the real one), two context engines, or a generic "any agent" abstraction nobody needs.

## Decision

- OMP is the ONLY harness: agent loop, models, tools, subagents, sessions, memory, workflow decisions.
- cedian owns every surface: editor, buffers, review UI, browser/iOS/terminal/debugger presentation, dialogs, GPUI rendering.
- No generic harness abstraction (`AgentRuntime` trait, ACP runtime, …) unless a concrete OMP-internal need appears.
- Stack lock: Rust + GPUI only in the cedian process; TS only in the OMP repo; `gpui-kit`/`elygpui` approved for chrome, never for editor/buffer/diff surfaces.

## Consequences

- Every feature must answer "who decides (OMP) / who renders (cedian)".
- The "must not build" list (§88) is the enforcement tool for this ADR.
- One deliberate exception exists for verification checking: ADR-0010.
- **Amended by ADR-0027:** "TS only in the OMP repo" no longer applies — OMP is never forked, so cedian has no TypeScript in scope at all.
