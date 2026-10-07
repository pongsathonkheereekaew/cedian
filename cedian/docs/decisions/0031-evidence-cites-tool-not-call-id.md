# ADR-0031: Evidence names the tool; cedian resolves the call id from the router log

- **Status:** Accepted (owner, 2026-10-07)
- **Date:** 2026-10-07
- **Supersedes in part:** [ADR-0022](0022-host-tool-first.md) decision 2 (how evidence references a call; the invariant stays)
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) chapter on the workflow split ("OMP reports … through host tools"), §53, §55
- **Evidence:** P5 live recording, OMP 18.7.0 (`crates/cedian_cli/tests/replay_cli.rs` channel scenario)

## Context

ADR-0022 decision 2 had the agent cite earlier `tool_call_id`s in `cedian_workflow_update` evidence. Recording P5 against real OMP showed the model never sees those ids. OMP keeps them in the provider protocol (`call_…|fc_…`) and does not render them into the transcript the model reads. Asked to cite "the tool call id of your read", the model went looking for it: it read `history://Main`, then ran `bash` to list the session directory. That bash call waited on an approval prompt nobody answers in headless mode, and the turn hung until the prompt timeout. Evidence sent without an id was stored `unattributed`, as designed, so no required gate could ever pass.

## Decision

1. **The agent names the producing tool, not the call.** `op: evidence` takes `from_tool` (the OMP tool name, e.g. `bash`, `read`) and an optional `match` (a substring of that call's args preview, e.g. `cargo test`).
2. **cedian resolves the id.** The resolver walks the runtime's router log newest-first and binds the evidence to the first call that matches `from_tool`, contains `match`, finished with `is_error == false`, and is not a channel report (`cedian_workflow_update`, `cedian_complete`, `cedian_worktree_request`, by name or `xd://` device). The stored provenance carries that call's real `tool_call_id`; the stored summary gets the bound call's name and args preview appended, so a reviewer sees what it was bound to.
3. **Invariant unchanged.** Evidence is attributed only through a router-log entry. No match → `unattributed`, with the reason returned to the agent.

## Consequences

- The model can attribute evidence with information it actually has.
- Binding proves a matching call ran and succeeded. It does not prove the call supports the claim (e.g. a `read` cited as a `test`). Kind-to-tool consistency and freshness against later edits belong to the ADR-0024 work in S2.
- "Most recent matching call" can bind to a newer call than the one the agent meant. The agent can narrow it with `match`, and the result shows the bound call.
- The log lives with the runtime: one-shot `cedian prompt` only binds calls from its own turn; `cedian shell` binds across its turns.
