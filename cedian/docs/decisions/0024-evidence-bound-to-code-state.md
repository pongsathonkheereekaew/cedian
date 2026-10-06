# ADR-0024: Evidence is bound to code state, has three outcomes, and claims must cite it

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §29, §52, §53, §55, §56
- **Extends:** ADR-0007 (frame binding), ADR-0010 (gate checker)

## Context

Studied `cursor/plugins` pstack (poteto, 2026-10-06), a methodology plugin whose verification rules are prompts. Three of its rules expose gaps in cedian's gate model:

- **Shipping playbook.** A PR verdict records head SHA, base SHA and `git patch-id`. A rebase silently invalidates the verdict even when checks stay green. cedian binds evidence to state only for browser frames (§29). A test-run evidence item stays valid after the agent edits code again, so a gate can pass on evidence about code that no longer exists.
- **Verify guide.** A check that could not run is "inconclusive", and "a confident reply without evidence is a red flag". cedian evidence is `ok: bool`, so "could not run" collapses into fail, or worse, gets reported as pass in prose.
- **Benchmark checklist.** A measured number needs a run count, median, range, a named limiter and a production-like build, or it is inconclusive.

## Decision

1. **Code-state binding.** Every evidence item records the state it verified:
   - `code_state`: buffer versions (`clock::Global` under Zed; the headless `Version` until then) of the files the producing tool call could observe, or a worktree tree fingerprint for repo-wide checks such as test suites.
   - A later change to any bound file makes the item `stale`. A required gate never counts stale evidence; the agent must re-capture.
   - Browser frame binding (§29) is the presentation-layer case of this rule.
2. **Three outcomes.** Evidence has `outcome ∈ pass | fail | inconclusive`.
   - `inconclusive` never satisfies a gate that requires passing evidence.
   - `inconclusive` is shown distinctly, never as pass.
3. **Claims ledger.** `cedian_complete` (ADR-0022) takes the agent's claims. Each claim is labelled `measured | inferred | guess` and lists evidence ids.
   - A `measured` claim needs fresh, attributed `pass` evidence.
   - The completion view shows every claim with its label and evidence. Claims without evidence are flagged, never hidden.
4. **Measurement evidence** (performance gates) carries `{runs, median, range, limiter, build_profile}`. A performance gate treats a measurement missing any field as `inconclusive`.

## Consequences

- Closes the "test passed, then the agent kept editing" hole.
- `cedian_workflow::Evidence` grows `code_state` and `outcome`, and gate evaluation stays a pure function: staleness is computed by comparing stored state to the current versions passed in as data, never by I/O.
- The store schema changes, so `workflow.json` bumps `snapshot_version` (P3).
