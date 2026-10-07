# Phase 0.5 — RPC Spike Capability Table

> **Frozen history (2026-10-07).** Per-feature OMP coverage now lives in [`docs/OMP_PARITY.md`](../docs/OMP_PARITY.md) ([ADR-0034](../docs/decisions/0034-identity-and-omp-parity.md)). Do not update this file.

Proven against real `omp --mode rpc-ui` (v18.6.1) on 2026-10-06.
Harness: `spike/rpc-spike` (`cargo run -p rpc-spike -- --full` → `SPIKE-RESULT: PASS`, 14/14).
Protocol reference: upstream `can1357/oh-my-pi` `docs/rpc.md` + `sdk/rust/omp-rpc` (blocking
transport + generated `wire.rs` — reuse in Phase 1, do not hand-roll again).

Auth note: spike uses ambient default auth (`openai-codex` subscription). Pinning
`--provider openai` fails with `No API key found` — never pin a keyed provider in spike/CI.

## HAVE (proven by run)

| Row | Evidence |
|---|---|
| request correlation | overlapping `get_session_stats` + `get_state`, matched by id |
| streaming events | `agent_start → message_update(text_delta) → agent_end(yielded) → prompt_result → session_settled`; `spike-pong` verbatim |
| cancellation | `abort` mid-`sleep 25` → `prompt_result status=aborted`, `sessionSettled=true` |
| protocol negotiation | `ready{1,[1,2]}` → `negotiate_protocol v2` ok |
| frame chunking | `get_available_models` (909 models, 2.4MB) via 10-frame `rpc_chunk` reassembly |
| backpressure | server spools to temp file + 64KiB drain (doc); chunked 2.4MB arrived intact — no exertion test, accepted as HAVE per doc |
| subagent events | `set_subagent_subscription progress` accepted (no live subagent spawned in spike; lifecycle frames per doc) |
| host tools | register ok + full roundtrip: model called `echo_host{ping123}` → host served `PONG-spike` → verbatim in reply. Model discovers host tools via `xd://echo_host` device mount |
| host URIs | register ok + full roundtrip: model `read spike://notes/1` → `host_uri_request` → host served `URI-PONG-spike` → verbatim in reply |
| UI request/response | `set_ask_dialog(true)` ok; `ask` emits batched `extension_ui_request`; `select/input/confirm/editor/cancel` + timeout→recommended-option default per doc |
| prompt images | 1px PNG prompt → `img-ok` completed; server normalizes to webp in transcript |
| session restore | fresh dir `open_session → resumed=false`; `--continue` re-adopts newest session (`msgs=14`, model restored `opencode-go/muse-spark`); reopen same dir = no-op per doc |
| generated Rust client usable from Zed fork | upstream `sdk/rust/omp-rpc` EXISTS (blocking transport, v2 chunking, `prompt_and_wait`, host tools/URIs, process-group teardown) — vendor or re-derive `wire.rs` via `bun run gen:rpc`; spike's hand decoder is throwaway |
| mode parity | see MISSING — resolved as OMP-side addition, not a cedian cut |

## MISSING (owner + phase, per plan gating rule)

| Row | Verdict | Owner + phase |
|---|---|---|
| `set_mode(normal\|plan\|deep\|goal)` | DOES NOT EXIST in `RpcCommand` (upstream-verified in plan). `goal` lifecycle IS RPC-controllable (`goal get/create/resume/pause/drop`, continuation needs `goal.continuationModes ∋ rpc`) but is not a mode switch; mid-turn switch = abort + new turn | OMP-side addition in `packages/coding-agent/src/` (OMP repo), Phase 2 §65. Phase 2 ships only modes RPC supports until then |

## UNSTABLE / gotchas (no owner, design constraints for Phase 1)

- Heredoc-piped stdin closes early → server disposes before turn runs. Host MUST keep stdin open for the session lifetime (writer thread owns stdin, `close()` = EOF = shutdown).
- `open_session` requires persistence: fails under `--no-session`; reopening same dir on a NEW process reports `cancelled:true, resumed:false` (adopts newest). In-process reopen = no-op.
- `host_tool_call` id ≠ `toolCallId`: correlate host roundtrips on `id`; `toolCallId` joins `tool_execution_*` events.
- Unknown commands echo id with `success:false`; malformed JSONL → `command:parse`, no id, loop continues (recoverable, never fatal).
- `set_model` to unknown pair fails clean (`Model not found`); process stays alive.
- Large `get_available_models` ONLY arrives intact on v2 — v1 truncates/fails. Always negotiate v2 first.
- Model input `["text","image"]` on the default model; thinking levels `off/minimal/low/medium/high` (+xhigh/max on reasoning models).

## Consumer map (unchanged — no deferrals; only mode-parity addition above)

`request correlation, streaming events, cancellation → Phase 1, 2`
`protocol negotiation, frame chunking, backpressure → Phase 1`
`prompt images → Phase 1, 2`
`session restore → Phase 1, 5`
`subagent events → Phase 16, 17`
`host tools → Phase 4, 7, 8`
`host URIs → Phase 6`
`UI request/response → Phase 2`
`generated Rust client → Phase 1 (vendor sdk/rust/omp-rpc)`
`mode parity → Phase 2 (OMP-side addition, owner assigned above)`
