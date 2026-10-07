# ADR-0036: S2 readings of ADR-0024 — historical reproduction evidence, turn-end block, headless code state

- **Status:** Accepted (owner, 2026-10-07)
- **Date:** 2026-10-07
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §53, §54, §55; [ROADMAP.md](../ROADMAP.md) S2 exit, stand-in row H
- **Refines:** ADR-0024 (decision 1), ADR-0010 (completion boundary), ADR-0022 (decision 3)

## Context

Planning S2 (`docs/plans/s2-workflow-core.md`) hit three places where the accepted text does not say enough to implement:

1. **Reproduction evidence is about old code by design.** ADR-0024 says a later change to a bound file makes evidence `stale` and "a required gate never counts stale evidence". The bug-fix playbook's required `reproduce` gate needs evidence that the bug exists *before* the fix. Read literally, the fix itself makes that evidence stale, and no bug fix could ever complete.
2. **When does a refused completion become `blocked`?** The S2 exit says an OMP turn that claims completion with a required gate unmet is blocked (status `blocked`). Today `cedian_complete` refuses and spends one continue; `blocked` only arrives after `MAX_CONTINUE` refusals in a row. A model that stops after one refusal leaves the workflow `running`, so nothing shows the user that the claim failed.
3. **Headless code state.** ADR-0024 binds evidence to "the headless `Version`" before S9. Headless `Version`s are per-process counters: a one-shot `cedian prompt` reloads buffers from disk at version 0, so a stored version means nothing to the next process.

## Decision

1. **Gates declare freshness.** `GatePredicate` gets `fresh: bool`.
   - `true` (the default, and every gate kind except `reproduction`): only evidence whose `code_state` still matches the current state counts.
   - `false` for `reproduction` gates: the evidence records the state it saw, and staleness is shown, but it still counts. What it proves ("the bug existed") cannot go stale.
   - OMP-added gates can't set `fresh: false` on a non-reproduction kind.
2. **Blocked at the turn boundary.** When a turn ends and the last `cedian_complete` call in it was refused, cedian sets the workflow `blocked` and the CLI prints the missing gates. `MAX_CONTINUE` still bounds retries inside a turn. Only the user unblocks it, with `cedian workflow resume`, which sets the status back to `running` and resets the continue budget. A user prompt alone does not unblock it.
   - **Exception:** if the agent already failed a phase itself (`advance passed: false`), the workflow stays `failed`. That statement is more exact, so it isn't overwritten. The missing gates are still printed. A failed workflow isn't resumable: the agent or user starts a new one.
3. **Headless code state is a content hash.** Until S9 binds `clock::Global`, `code_state` stores a stable 64-bit FNV-1a hash per file, or a tree fingerprint over workspace text files for repo-wide calls. Stand-in row H in ROADMAP. It's deleted at S9.

## Consequences

- Bug fixes can complete: the repro is captured before the fix, and verify evidence after the fix must be fresh.
- The S2 exit becomes observable after one refused claim. The recorded turn does not need three refusals in a row. The exit reads "blocked (or failed when the agent failed the phase itself)".
- Code state survives across one-shot CLI processes and `cedian shell` alike.
- If the owner rejects decision 1, `reproduce` would have to accept stale evidence through some other route. Decision 1 is the smallest one.
