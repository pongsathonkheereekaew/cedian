# P5 — Host-tool channel plan

Exit (ROADMAP P5, ADR-0022): OMP reports to cedian through three host tools — `cedian_workflow_update`, `cedian_complete`, `cedian_worktree_request` — and evidence submitted through them is `attributed` only when its `tool_call_id` is in the router log with a successful `ToolEnd` that is not itself a channel call. Proven by a hermetic fake-omp replay of a recorded OMP turn (§86). No OMP change.

**Timebox:** 2 working sessions. **Partial exit:** commit what is green (unit-level channel + router check first, replay second); anything unproven goes to README as the named gap.

## Out of scope (belongs to the slice that needs it)

- Claims ledger, `stale` / `inconclusive` evidence (ADR-0024) → S2 exit.
- Playbooks as OMP skills (ADR-0025), gate floor (ADR-0026) → S2.
- Subagent events, worker visualization, steer into a worker session → S5 (row C).
- `cedian_review_finding` → S3.

## Where the code lives

- `cedian_omp::EventRouter::finished_tool_call(id)` — log lookup (name, preview, is_error). Pure read of the existing log.
- `cedian_workflow/src/channel.rs` — `WorkflowChannel`: state + persist callback + verifier closure (`Fn(&str) -> bool`, so the crate never depends on `cedian_omp`). Logic is plain functions over JSON args (unit-testable without an RPC context); `host_tools()` wraps them as `omp_rpc::HostTool`s. Same pattern as `cedian_workspace::HostTools::apply_edit_tool`.
- `cedian_worker/src/host_tool.rs` — `cedian_worktree_request` over the existing `spawn` + `Registry`.
- `cedian_cli` — registers the complete set in one `set_host_tools` call (ADR-0004: it replaces), builds the verifier from the runtime's router, persists through `workflow_store`, and adds one ambient line when a workflow is active.

## Tool contracts

| Tool | Args | Effect / result |
|---|---|---|
| `cedian_workflow_update` | `op: start` + `kind`, `title`, `risk?` | starts a workflow (refused while one is running) |
| | `op: evidence` + `gate`, `summary`, `ok`, `tool_call_id?`, `kind?` | attaches one item; attributed iff the verifier accepts `tool_call_id`, else `unattributed` (the result says which) |
| | `op: advance` + `passed` | advances the current phase (gates block, as today) |
| `cedian_complete` | `summary?` | no workflow → ok (fast lane). `can_complete` → status `complete`. Else `isError` listing missing gates + attempts left; each failing required gate counts one continue; at `MAX_CONTINUE` the workflow is `blocked` and the result says to stop and escalate |
| `cedian_worktree_request` | `id`, `title`, `kind?`, `base?` (default `HEAD`) | `git worktree add` + registry row; returns id, path, branch. Refused when `project_write` is `Deny` (not registered) |

Verifier rule: an id counts only if the log has a `ToolEnd` for it with `is_error == false` AND the call is not a channel call (`cedian_workflow_update` / `cedian_complete` / `cedian_worktree_request`, by name or `xd://` preview) — the agent cannot self-certify by citing its own report.

## Tasks

| # | Task | Done when |
|---|---|---|
| T1 | Router `finished_tool_call` | unit test: success, error, unknown, started-only |
| T2 | `WorkflowChannel` (update/complete logic + verifier + persist) | unit tests: start/evidence/advance; attributed vs unattributed (unknown id, failed call, self-cite); complete ok / missing / blocked at 3; no-workflow fast lane |
| T3 | `cedian_worktree_request` | unit test in a temp git repo: creates `.worktrees/<id>` + registry row; bad id refused |
| T4 | CLI wiring (one-shot + shell): full set registered, ambient line, workflow persisted | `cargo test --workspace` green |
| T5 | Recorded OMP turn → hermetic replay: `bash` check → `cedian_workflow_update evidence` citing it → `cedian_complete`; plus a self-cite stored `unattributed` | `cargo test -p cedian_cli --test replay_cli` green in replay mode |

## Risk

The live model has refused `xd://` host tools as untrusted (seen recording P4). Tool descriptions + the ambient line must state they are cedian's own host tools. If the model still will not call them, T5 is the named gap and P5 stays ◐.
