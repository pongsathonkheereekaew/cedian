# ADR-0032: Correction ledger — recorded corrections become enforced rules

- **Status:** Proposed
- **Date:** 2026-10-07
- **Rule text:** none yet (proposed). On acceptance: [ARCHITECTURE.md](../ARCHITECTURE.md) §18, §59, §64; [ROADMAP.md](../ROADMAP.md) S3
- **Builds on:** ADR-0006 (provenance), ADR-0024 (evidence), ADR-0025 (methodology is OMP skills), ADR-0026 (fast lane)

## Context

pstack (`cursor/plugins`, v0.15.15) and its author's talk (Matt Pocock live with Lauren Tan, 2026-10, ~31:22 and ~50:16) make one loop central to trusting agents more over time:
- watch where agents fail
- when the same mistake repeats across agents, fix the environment, not the agent
- prefer the strongest mechanism: architecture, then types, then a lint whose error names the fix, then a test, docs last (`/correct`, principle `encode-lessons-in-structure`)
- prove each new check fails on a real past mistake

In pstack this loop runs on prompts: an agent greps commits, reverts, review comments and transcripts and guesses what the corrections were.

cedian already records the corrections as structured state, and then lets them sit unused:
- a hunk the user rejected (`HunkStatus::Rejected`, §18)
- a turn the user reverted (P6 `revert-turn`)
- an agent hunk the user edited afterwards (`HunkStatus::Stale`)
- a completion `cedian_complete` refused, and `max_continue` escalations (ADR-0010)
- a review finding dismissed with a recorded reason (ADR-0011, S3)

Each event carries the producing `tool_call_id`, the file, the turn, and the task (ADR-0006, ADR-0031). Cursor has no equivalent record. This is data only the environment has, so it belongs to cedian. Deciding what the data means is judgment, so that part belongs to OMP.

## Decision

1. **Correction events are recorded, never asked for.**
   - cedian appends one row to `.cedian/corrections.jsonl` when a correction happens: `{ts, task, turn, kind, path, hunk_key, tool_call_id, model, excerpt_hash}`.
   - `kind ∈ hunk_rejected | turn_reverted | user_edited_agent_hunk | completion_refused | continue_escalated | finding_dismissed`.
   - `finding_dismissed` records reviewer noise, not implementer error, and is kept apart in every view.
   - Rows are derived from state cedian already holds. The agent cannot write, edit or delete them.
   - Append-only, local to the repo, never sent anywhere.
2. **cedian checks mistake classes; OMP proposes them.**
   - Grouping events into a class is judgment. An OMP skill (`correct`, the user's own config per ADR-0025) reads `cedian://corrections` and proposes classes through a new host tool, `cedian_correction_class`: `{name, event_ids, level, enforcer}`.
   - `level ∈ architecture | types | lint | test | docs`.
   - cedian rejects a class whose events are unknown ids, or that has fewer than two events from at least two different turns. One-offs are not classes.
3. **A class is `enforced` only with proof.**
   - The proof is ADR-0024 evidence: the enforcer `fail`s on the recorded mistake's code state and `pass`es at the current head.
   - A class without that proof is `documented`. The UI shows `documented` as weaker than `enforced`, never as equal.
   - A `docs`-level class is always `documented`.
4. **Repeats escalate.** A new event matching an existing class marks the class `repeated`. For a `documented` class, the UI proposes the next stronger level. For an `enforced` class, the enforcer is flagged as leaky.
5. **cedian never applies a fix.** Every lint, type change, test or rule-file edit goes through an ordinary OMP turn and shows up as reviewable hunks. The rule table (class → level → enforcer → status) is a cedian view. It is exported to the repo's agent instruction file only through such a turn.
6. **Zero cost on the fast lane.** Recording is a side effect of resolutions that already happen. It adds no prompt, tool call or gate to a normal turn (ADR-0026). The overhead is measured with the other per-turn costs.

## Consequences

- cedian can answer "what do agents get wrong in this repo, how often, and what now prevents it" from recorded data, not recollection. This is a review-rigor claim Cursor cannot make (ADR-0023).
- S3 gains the ledger, the `cedian://corrections` URI and the `cedian_correction_class` tool. It ships with the S3 audit log (`.cedian/audit.jsonl`), which uses the same append-only, replay-tested JSONL mechanism.
- A later trust-ladder view (reject, revert and false-done rates per task kind × risk) can read the same file. That view may *suggest* changing a gate floor. It never lowers one (ADR-0010).
- `excerpt_hash` instead of excerpt text keeps the ledger from becoming a second copy of the code. Showing a past mistake re-reads it from the task store while that store exists.
- Open question: whether user edits to agent text count as corrections by default. Many such edits are refinements, not fixes. The proposal records them under their own kind, and the `correct` skill decides what they mean.
