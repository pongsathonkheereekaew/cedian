# cedian — Roadmap

**cedian is a Cursor-style agentic IDE — the same agent + IDE workflow and product features — built for OMP: OMP runs as upstream ships it, every OMP feature gets a native surface, and Zed is forked only as far as OMP needs. OMP decides; cedian executes, renders, and verifies.** ([ADR-0034](decisions/0034-identity-and-omp-parity.md), scope [ADR-0023](decisions/0023-product-scope.md)).

The ONE schedule: vertical slices S0–S9. Each slice is a thin cut that ends triable in the `cedian` CLI (or the app once S9 lands). Never start a slice whose exit can't be exercised in this repo — integration risk first, never accumulate untested layers. Old phase numbers survive only as the "Scope from Phase N" blocks under each slice.

**Exit decides, Scope describes.** A slice is judged ONLY by its **Exit**. The "Scope from Phase N" blocks record the FINAL target of the old phase; anything in them that needs Zed or GPUI (buffer transactions, panes, native undo, screencast) is delivered at S9, not by the headless slice it sits under.

Rules live in [`ARCHITECTURE.md`](ARCHITECTURE.md); reasons in [`decisions/`](decisions/) (method: [ADR-0017](decisions/0017-slices-exit-rule-stand-ins.md)); **status lives only in [`../README.md`](../README.md)** — this file never says done/partial.

> **Exit-criterion rule.** Every exit criterion names WHO acts and WHAT is observed: "an OMP turn is blocked by X" is an exit; "the CLI prints blocked" is not. A slice is ✅ in `README.md` only when its exit holds through an OMP turn (live lane) or a hermetic fake-omp replay (§86). Anything weaker is marked `◐ partial` with the gap named. Persisted state (the `*.json` stores in cedian's state dir, [ADR-0044](decisions/0044-cedian-state-outside-the-workspace.md)) carries `snapshot_version` from the slice that introduces it (§75).

## Execution order

Slice numbers are names, not order ([ADR-0023](decisions/0023-product-scope.md)):

```text
P1 spawn profile → P2 fake-omp → P3 snapshot_version
  → S9a app spike
  → P4 cedian shell → P5 host-tool channel → P6 revert turn + inline edit
  → P7 OMP parity ledger + test
  → S2 workflow core → `cedian.toml` move ([ADR-0018](decisions/0018-settings-in-one-toml.md)) → P8 OMP policy opt-in → S3 review agents
  → S9 real app, closing S4 browser evidence and S5 parallel workers  (= v0.1, ADR-0038)
  → S6 PR workspace → S7 local automations  (= v0.2)
  → S8 iOS (extension)
```

S0 and S1 are finished up to their open gaps (README); their gaps close inside P1/P2 and S9.

## Releases

Version numbers exist only once the app exists:

| Release | = | Notes |
|---|---|---|
| — (pre-S9) | S0–S8 headless work | no version numbers |
| **v0.1** | **S9 exit** | binds every slice that is ✅ in README at that point (target: S0–S5); gate = the "fully native" checklist under S9 |
| **v0.2** | S6 + S7 | PR workspace + local automations |
| iOS | S8 | extension track, after v0.1, never gating |

OMP-only multi-provider note: cedian speaks to ONE harness. Multi-model choice lives INSIDE OMP (provider/model routing, `set_model`) — cedian never adds a second harness adapter to chase providers. If OMP gains a provider, cedian gains it for free.

## Cursor workflow coverage

"Cursor-style" means Cursor's workflow and product features, agent + IDE in one app ([ADR-0034](decisions/0034-identity-and-omp-parity.md)). Each row is a Cursor workflow the owner uses and the slice that delivers it in cedian. A row is added only for a workflow the owner actually uses. Status stays in README.

| Cursor workflow | cedian | Delivered by |
|---|---|---|
| Agent panel: prompt, stream, stop, image input | agent panel over one OMP session per task (§41) | S0 (headless), S9 |
| Steer and queue follow-ups while the agent runs | `steer`, `follow_up`, queued-message chips | P4, S9 |
| Agent edits as diffs, accept/reject per hunk | Review Changes on the task baseline (§15–§18) | S0, S9 |
| Checkpoints / restore | revert turn ([ADR-0026](decisions/0026-fast-lane.md)) + OMP `branch`/`fork` thread tree | P6, S9 |
| ⌘K inline edit | inline edit on a selection | P6, S9 |
| Tab completion | Zed edit prediction, not routed through OMP ([ADR-0023](decisions/0023-product-scope.md)) | S9 |
| Plan mode | separate plan runtime ([ADR-0014](decisions/0014-agent-modes.md)) | S9 |
| Rules, `AGENTS.md`, skills, MCP, hooks | OMP config used as-is (§77); listed in settings | S9 |
| Model picker | OMP routing via `set_model` | S9 |
| Agent runs terminal commands | OMP `bash` tool cards; user-run `bash` in agent context | S0, S9 |
| Parallel agents in worktrees, best-of-N | OMP subagents + cedian worktrees; arena preset | S5 |
| Agents window (every running agent) | subagent tree + session manager | S5, S9 |
| Built-in browser the agent drives | one shared Chromium, agent and user on the same tab | S4, S9 |
| Point at a page element to give the agent context (Design Mode) | element pick in the browser pane → prompt context | S4 |
| PR review bot | review agents + PR workspace | S3, S6 |
| Background agents | local only: `cedian shell` + scheduled runs; no cloud ([ADR-0019](decisions/0019-scope-cuts.md)) | P4, S7 |
| Usage and cost | `get_session_stats` meter | S9 |

Not targets: Cursor's own models (OMP routes models), cloud VMs (ADR-0019), the VS Code extension marketplace (Zed's extensions stay as upstream ships them).

## Headless stand-ins (pre-S9)

Before the Zed fork lands (S9), some capabilities the plan assigns to Zed are built as **headless stand-ins** inside cedian crates so slices stay triable. A stand-in is allowed only if it has a row below: owner crate, the plan section it temporarily deviates from, and the slice that deletes or moves it. A stand-in with no row is a violation of ARCHITECTURE §88. Rationale: [ADR-0017](decisions/0017-slices-exit-rule-stand-ins.md).

| Row | Stand-in (today) | Deviates from | Fate |
|---|---|---|---|
| A | `cedian_workflow` — TaskProfile/Playbook/phase state driven from the CLI | §46 split (OMP owns profile/playbook/phases) | **Decided: A1-narrow ([ADR-0010](decisions/0010-gate-checker-a1-narrow.md)).** KEEP permanently: evidence store, `Gate::evaluate`, `can_complete`, `max_continue`, gate-floor policy. MOVE to OMP (§8 `workflow/`): task classification, playbook choice, phase advancement. CLI `workflow run/advance` stay as stand-ins until OMP reports through host tools `cedian_workflow_update` / `cedian_complete` (P5, [ADR-0022](decisions/0022-host-tool-first.md)). |
| B | `cedian_lsp` / `cedian_dap` — own rust-analyzer / lldb-dap subprocesses | §21/§22 (one LSP/DAP, Zed's), §88 "custom LSP/DAP" | **Deleted (S9 U6, fork `224b092227`).** The app serves code intelligence from Zed's project through `cedian://` (selection, diagnostics, definitions, references, symbols); headless, those reads answer an error and OMP's own `lsp` tool serves the CLI. OMP's `lsp` still runs next to Zed's in the app until Zed covers rename and diagnostics after a write ([ADR-0048](decisions/0048-one-lsp-zeds-staged.md)). |
| C | `cedian_worker` — user-driven `git worktree` registry | §43 (OMP requests via host tool), §42 (subagent tree) | Stays as the mechanism. Since S9 U8 the app registers `cedian_worktree_request` (ADR-0033 brief) and shows OMP's subagents with a real steer and cancel ([ADR-0050](decisions/0050-workers-are-omp-subagents.md)); CLI `worker steer` and `list` are deleted. `spawn`, `preview`, `merge-back` and `remove` stay until the app has them (Follow-ups). |
| D | `cedian_browser` — separate cedian-owned headless Chrome, one per CLI invocation | §25 (one shared Chromium with OMP), §29 R4 (frame binding) | **Deleted (S9 U7, fork `bcc37a9269`).** The app owns one Chromium per workspace and OMP attaches to it over CDP ([ADR-0049](decisions/0049-one-chromium-owned-by-the-app.md)); captures are bound to the browser's frames and earlier evidence reads `stale-frame`. The CLI has no browser command; headless browser use is OMP's own. |
| G | Headless form of the agent-edit import: no Zed buffers yet, so writes stay on disk | §12 (agent writes imported as Zed transactions, [ADR-0027](decisions/0027-zero-omp-fork.md)) | **Deleted (S9, fork `c41e5623c8`).** The app imports each OMP write as one agent transaction and reviews it on Zed buffers (U5). The CLI's `review` hunk listing, `accept`, `reject`, `accept-all`, `turns` and `revert-turn`, the string `AgentEdit` provenance and the disk-based tracker are gone (owner ruling, 2026-10-08); the S3 reviewer reads plain task diffs. |
| H | Headless code state: evidence binds to FNV-1a content hashes per file (or a workspace tree fingerprint), not buffer versions | §53 / [ADR-0024](decisions/0024-evidence-bound-to-code-state.md) (`clock::Global`) | [ADR-0036](decisions/0036-s2-evidence-freshness-and-turn-end-block.md). Deleted at S9: evidence binds to Zed's `clock::Global`. Until then, staleness compares hashes the CLI computes from disk, passed into the pure gate engine. |
| I | `cedian_workspace` `buffer.rs` — in-memory buffer store: versioned text, transactions, undo; since S9 U5 its version is a `clock::Global` and the host-tool wire carries it as a version token; the app reviews on Zed buffers, the store remains only as the headless CLI's host | §12 / editor and buffer stay Zed-native (`text::Buffer` + `History`, `clock::Global`) | Deleted at S9: host tools rebind to Zed buffers and `Version` becomes `clock::Global` with the same call shapes (`buffer_version`, `apply_edit(path, expected, edit)`). Until then: `apply_edit` checks `expected_version` and fails closed on a concurrent edit. |

## Follow-ups

Known gaps that no slice exit names yet. Each row has the slice it must be settled before. A row is deleted in the change that settles it; this table never says done.

| Follow-up | Found in | Settle before |
|---|---|---|
| cedian spawns the first `omp` on PATH (or `CEDIAN_OMP_BINARY`) and never checks its version against the pin (ADR-0003). This machine has the pinned 18.6.1 in `~/.local/bin` and Homebrew's 18.7.0; on 18.7.0 the live smoke test fails at P3, because `read` adds a `1:` line prefix to host-URI content | `crates/cedian_cli/src/main.rs` `resolve_on_path`, `crates/cedian_omp/tests/smoke_real_omp.rs:89` | S9 (bundled pinned OMP); the 18.7 prefix with the next pin bump |
| The one-shot prompt timeout is a fixed 600 s; both B6 runs were cut there mid-task | `crates/cedian_cli/src/main.rs` (`prompt_timeout`), S2 benchmark | S9 (long turns in the app) |
| Correction ledger gaps (ADR-0032): no `cedian://corrections` URI, no `repeated` escalation, no `continue_escalated` rows, `model` left empty, and a class proof cannot check that both evidence runs were the enforcer's command | [S3 plan](plans/done/s3-review-agents.md) | S9 |
| Reviewer panels by role ([ADR-0039](decisions/0039-model-roles-and-independent-review.md) decision 5): `[[workflow.floor]] review_roles` and `maxReviewers` are not built; one reviewer runs on the `review` role | ADR-0039 | S9 |
| A call headless denied still makes earlier evidence born stale, though it changed nothing | [S2 plan](plans/done/s2-workflow-core.md) findings | S9 (row H) |
| S5 exit items not built in U8 ([ADR-0050](decisions/0050-workers-are-omp-subagents.md)): an edit outside the brief's `scope.write` is not `OutOfScope`; a worker past its timebox does not show `stuck`; cleanup does not classify trees as merged / wip / scratch; OMP's subagent events name no worktree, so the app cannot show which subagent works in which tree | [S9 plan](plans/s9-real-app.md#progress) U8 | S9 (S5 exit) |
| Worktrees in the app: no merge-back or remove (CLI `worker spawn`, `preview`, `merge-back`, `remove` stand in); `cedian_worktree_request` is registered only when project writes are `allow`, with no approval dialog under `ask`; CLI `worker spawn` takes no brief, so ADR-0033's one validator is not shared | same | S9 (S5 exit) |
| Subagent and queue gaps (U8): no subagent transcript pane (`get_subagent_messages`); a follow-up's text is not shown in the thread as a user message; composer images are not sent with a follow-up or steer; a new session clearing subagent rows is checked by reading only; Stop's notice can name a display twin instead of a message taken into a run, and a dead link can put both texts of one message in the composer | same | U11 (parity) |
| Hand-made fixture frames (U8): `p5_worktree.jsonl` was edited to carry a brief, `live_approval.jsonl` has two inserted pairs, `turn_lifecycle_1/3/4` are ordered from OMP 18.6.1's handlers, not recorded; re-record them on the next live pass | same | U11 (parity) |
| The fork's pre-commit hook fails inside a git worktree: with `GIT_DIR` set, `script/cedian-check` changes to the wrong top level and cannot find `script/cedian-prune` | owner's session, 2026-10-09 | U13 (packaging) |
| Inline edit runs on the session model: OMP has no RPC model-role switch, so it cannot use `smol` as ADR-0026 asks | README P6 row | next OMP pin bump |
| An OMP write that lands before cedian reads the file's disk text at the call's start (a file not open, or a buffer dirty at the start) imports as Unchanged: the write is on disk and in the buffer but not a review hunk. A synchronous read at ToolStart, or OMP reporting the pre-write text, would close it | [S9 plan](plans/s9-real-app.md#progress) U5 | U9 |
| Nothing writes the ADR-0032 hunk-level correction rows (`hunk_rejected`, `user_edited_agent_hunk`, `turn_reverted`) since the CLI's review commands were deleted (fork `c41e5623c8`): the panel emits the events but has no `corrections.jsonl` writer, and the CLI's variants are kept only for that | [S9 plan](plans/s9-real-app.md#u5-design) U5 | U9 (workflow and findings in the app) |
| Files changed by non-edit tools (`bash`: formatters, `sed -i`, scripts) are not review hunks; their reload counts as a user edit, so overlapping agent hunks turn STALE. Safe but noisy. Owner ruling (2026-10-08, [ADR-0047](decisions/0047-bash-writes-are-agent-edits.md)): a file a `bash` call changes is the agent's edit, imported as that call's transaction | [S9 plan](plans/s9-real-app.md#u5-design) U5 | U9 |
| `script/cedian-bench/run.sh` task b4 restores `fake_lsp_bridge.rs` and names `attach_lsp`, both deleted in U6; it runs only against a `$REF` from before the deletion. Retire or rewrite b4 | [S9 plan](plans/s9-real-app.md) U6 | U12 (benchmark re-run) |
| Browser items left after U7 ([ADR-0049](decisions/0049-one-chromium-owned-by-the-app.md) decision 6): an inline screencast pane; promoting the browser gate to required (captures come from the panel's Capture button, not yet tied to OMP's `browser` call); nothing in the app evaluates gates yet, so `stale-frame` is computed by `CedianPanel::evaluate_gate` but used only once the workflow is visible | [S9 plan](plans/s9-real-app.md) U7 | U9 (workflow and evidence in the app) |
| Person-input detection gaps (U7): out-of-process iframes are not followed, and agent input through `Input.synthesizeTapGesture`, `synthesizeScrollGesture` or touch emulation is not modelled, so it could preempt the agent itself; untested against real Chromium: the isolated-world binding | [S9 plan](plans/s9-real-app.md) U7 | U11 (parity) |
| `script/cedian-bench/run.sh` task b10 splices `browser_store.rs` (deleted in U7) from a pinned commit; it runs only against that commit | [S9 plan](plans/s9-real-app.md) U7 | U12 (benchmark re-run) |
| The app still runs OMP's own language servers next to Zed's ([ADR-0048](decisions/0048-one-lsp-zeds-staged.md)): `--no-lsp` is added to the app's spawn profile only when Zed covers rename and diagnostics returned after a write (U6 covers definitions, references, symbols and diagnostics reads) | [S9 plan](plans/s9-real-app.md) U6 | U11 (parity) |
| One driver per session is enforced from the app's side only ([ADR-0046](decisions/0046-one-driver-per-session-by-file-holders.md)): OMP's CLI still resumes a session the app drives, and a process that resumed a session but has not written yet holds no file the app can see. The fix is upstream: `open_session` refusing or reporting a held session (an OMP request) | [S9 plan](plans/s9-real-app.md#u4-design) U4 | next OMP pin bump |
| A dialog that times out (§63, 5 minutes) is cancelled and audited `abstain`, but the §54 escalation that should follow is not built | [S9 plan](plans/s9-real-app.md#u4-design) U4 | U9 (workflow visible in the app) |
| The reviewer's network is open ([ADR-0043](decisions/0043-reviewer-sandbox-private-state-no-credentials.md) decision 4): a steered reviewer can still send out what it reads, OMP's auth token included. Seatbelt cannot filter by host; the fix is a cedian egress proxy that passes only the provider's hosts | [S3 security review](plans/done/s3-review-agents.md#security-review-2026-10-07) | S6 (reviewers on untrusted PRs) |
| The reviewer profile's `mach-lookup` and `iokit-open` allows are unfiltered; narrowing them needs a logged probe of the services OMP uses | same | S6 |
| ADR-0012's other protected paths are not enforced: cedian does not deny implementer writes to `.git/` (for example `.git/hooks/*`, which git runs as the user outside any sandbox) or to the OMP session dir, and whether OMP's `write` mode refuses `.git/` on its own is unverified. ADR-0044 closed `.cedian/` only | same | S9 (before anyone else runs cedian) |

## Cross-cutting prerequisites

Shared foundations that several slice exits depend on. Each is small; build it before the first exit that needs it, never inside a slice as a side task.

| ID | Prerequisite | Decision | Needed by |
|---|---|---|---|
| P1 | **OMP spawn profile.** One argv builder with `--approval-mode`, plus a generated `--config` overlay (`computer.enabled: false`, approval policy, `bash.patterns`) and the precedence verification test | [ADR-0020](decisions/0020-omp-spawn-profile.md) | every slice — fixes two hard rules violated today (S0); S3 gate 1 |
| P2 | **Hermetic fake-omp replay harness.** Recorded RPC frames → `EventRouter` → thread/panel/review, run in `cargo test` | ARCHITECTURE §86 | S0 exit; S2, S3, S5 exits (workflow and subagent events) |
| P3 | **`snapshot_version` on every store.** `workflow.json`, `workers.json`, `browser.json`, session manager; user-edited `cedian.toml` carries a `schema` key instead; mismatch fails closed | ADR-0016 | before any slice adds a new store |
| P4 | **`cedian shell`.** One long-lived headless process + `shell.lock` in the state dir | [ADR-0021](decisions/0021-headless-host-process.md) | S4 exit (frame seq), S5 exit (steer), S7 (scheduler), S3 (reviewer sessions) |
| P6 | **Revert turn + headless inline edit.** Turn-grouped provenance → one-action revert (skips `STALE`); `cedian shell` `edit <path> <range> <instruction>` | [ADR-0026](decisions/0026-fast-lane.md) | S2 exit (benchmark), S9 (⌘K + revert UI) |
| P7 | **OMP parity ledger + test.** [`cedian/OMP_PARITY.md`](https://github.com/pongsathonkheereekaew/zed/blob/cedian/s9/cedian/OMP_PARITY.md) in the fork has a row for every RPC command, server notification (agent events included) and UI request in the vendored `wire.rs` (by wire name) plus hand-reviewed tool and config rows; `cargo test -p cedian_omp --test omp_parity` fails on any missing row. Runs in pre-commit and on every pin bump | [ADR-0034](decisions/0034-identity-and-omp-parity.md) | every OMP pin bump; S9 (v0.1 gate) |
| P8 | **OMP policy opt-in.** `[projects."<path>"] policy = "omp"` in `cedian.toml` → spawn profile variant without approval or `computer` keys; badge, `approved by OMP` tool-card label, `decision_source: omp` audit rows; reviewers and automations always get the default profile. Live test: opted-in project + project yolo config → exec-tier call runs without a prompt and is audited | [ADR-0035](decisions/0035-omp-native-approval-opt-in.md) | S3 (audit log shape, reviewer profile), S7, S9 settings UI; needs the user's `cedian.toml` ([ADR-0018](decisions/0018-settings-in-one-toml.md)) |
| P5 | **Host-tool channel.** `cedian_workflow_update`, `cedian_complete`, `cedian_worktree_request`; evidence checked against the router log | [ADR-0022](decisions/0022-host-tool-first.md) | S2 exit, S5 exit, S3 findings (`cedian_review_finding`) |

## Slice dependencies

| Slice | Exit depends on |
|---|---|
| S0 | P1, P2 |
| S9a | P1 |
| S2 | P2, P3, P5, P6 |
| S3 | gate (below) — gate 1 = P1; gate 3 = `cedian.toml` ([ADR-0018](decisions/0018-settings-in-one-toml.md)); plus P2, P4, P5 |
| S4 | P4 (frame seq is only real inside one live session) |
| S5 | P2, P4, P5 (`cedian_worktree_request` + subagent events) |
| S6 | S3 gate items 1, 2, 4 (merge is `Deny`-by-default and audited) |
| S7 | P4 (scheduler runs inside the shell), S3 gate 4 (run history = audit log) |
| S9 | all ✅ slices it binds; closes S4 and S5 ([ADR-0038](decisions/0038-s4-s5-close-inside-s9.md)); deletes stand-ins B and D; P7 (parity rows), P8 (opt-in UI) |

## OMP changes

None. cedian runs pinned upstream OMP and never forks it ([ADR-0027](decisions/0027-zero-omp-fork.md)). If a real need for an OMP change appears, open an upstream PR and wait; record the workaround (or the cut) as an ADR meanwhile.

## Slices

### S0 — Foundation loop

**Exit:** `prompt → cards → edit → review → accept/reject` in one CLI process, where: streamed text reaches the thread (not only `prompt_result`); review shows only task-attributed hunks (ARCHITECTURE §16), user edits after the agent show `STALE` (§18), accept/reject persist across invocations, and no OMP disk edit is ever overwritten by a buffer write-back (row G). A hermetic fake-omp replay (§86) covers router → thread.

#### Scope from Phase 0.5 — RPC Spike (gating)

> Historical: completed 2026-10-06 (`spike/CAPABILITY_TABLE.md`). Phase numbers and the `deep` mode below are from the original plan; current mode rules are in ARCHITECTURE §65.

Nothing in §§4–8 / 71–73 is locked until this spike passes. Prove it against the real `omp --mode rpc-ui` binary (observed present in `omp v18.6.1`):

```text
spawn omp --mode rpc-ui from a minimal Rust host
ready handshake
RPC v2 negotiation via generated Rust client
prompt → streamed text reply
abort mid-stream
open_session restore
```

Record a capability table (have / missing / unstable):

```text
request correlation
streaming events
cancellation
protocol negotiation
frame chunking
backpressure
subagent events
host tools
host URIs
UI request/response
prompt images
session restore
generated Rust client usable from Zed fork
mode parity (normal | plan | deep | goal)
```

If any row is missing: either scope Phase 1 down to what exists, or explicitly schedule the OMP-side addition in `packages/coding-agent/src/` (§8) before proceeding. Do not start Phase 1 on assumed capabilities.

Consumer map — a missing row auto-defers its consumers, no separate decision needed:

```text
request correlation, streaming events, cancellation → Phase 1, 2
protocol negotiation, frame chunking, backpressure → Phase 1
prompt images → Phase 1, 2
session restore → Phase 1, 5 (provenance restore)
subagent events → Phase 16, 17
host tools → Phase 4, 7, 8
host URIs → Phase 6
UI request/response → Phase 2 (ask dialog)
generated Rust client usable from Zed fork → Phase 1 (all)
mode parity → Phase 2 (§65)
```

Acceptance:

> Capability table filled from a real run. Every "missing" row has an owner + phase, or Phase 1 scope is cut to match.

#### Scope from Phase 1 — OMP Runtime Inside cedian

Implement:

```text
spawn bundled OMP
Rust RPC client
v2 negotiation
prompt
stream text
abort
session restore
images
```

Acceptance:

> Open cedian → type prompt → OMP replies without a terminal.

#### Scope from Phase 2 — Native Agent Panel

Implement:

```text
OMP thread
composer
streaming markdown
images
tool calls
tool outputs
ask
model state
thinking state
```

Acceptance:

> Full OMP session usable entirely inside cedian.

#### Scope from Phase 3 — Tool Card Registry

Support at least:

```text
read
edit
write
bash
grep
glob
find
lsp
debug
task
eval
browser
todo
```

Acceptance:

> Normal UX shows no raw RPC JSON or ugly terminal transcript for ordinary tools.

#### Scope from Phase 4 — Editor-Native Edit Surface (host tools + cedian:// URIs; no OMP backend seam — §10)

> **Ordering.** this phase MUST precede Review (Phase 5). `clock::Global` baselines (§16) have no meaning before `cedianWorkspaceHost::buffer_version` exists.

Implement:

```text
HostService protocol
WorkspaceHost
Zed buffer transactions
undo support
save synchronization
```

Acceptance:

> OMP `edit` modifies the Zed buffer transaction directly and native undo works.

#### Scope from Phase 5 — Edit Provenance + Review Changes

> **Ordering.** requires Phase 4 (baseline type + transaction hook). Do not start without `buffer_version` + `apply_edit` landing.

Implement:

```text
task baseline
toolCallId provenance
agent edit tracker
DiffPatch/MultiBuffer projection (acp_thread::diff — no "AgentDiff" symbol exists)
accept/reject
stale detection
```

Acceptance:

> OMP changes 10 files → Review Changes accurately shows only OMP task changes.

This is one of the most important milestones.

#### Scope from Phase 6 — Native Context

Implement:

```text
active file
selection
diagnostics
open editors
host URI
@selection
@file
```

Acceptance:

> Highlight code → say “fix this” → OMP operates on the correct target.

### S1 — Language services

Stand-in row B. OMP `lsp`/`debug` tools stay unchanged.

**Exit:** CLI shows REAL diagnostics/symbols for a Rust file; an agent breakpoint round-trips on a headless debug adapter.

#### Scope from Phase 7 — Zed LSP Surface (no OMP backend seam — §10)

Implement (cedian host tools + cedian:// URIs surfacing Zed LSP state; OMP `lsp` tool unchanged):

```text
cedian host tools (definitions/references/hover/symbols/rename/code-action/diagnostics)
cedian:// diagnostics + symbol URIs
```

Acceptance:

> OMP and cedian use the same language server state.

#### Scope from Phase 8 — Zed DAP Surface (no OMP backend seam — §10)

Implement (cedian host tools + cedian:// URIs surfacing Zed DAP state; OMP `debug` tool unchanged):

```text
cedian host tools (breakpoint sync/stack/variables/step/continue)
```

Acceptance:

> Agent-created breakpoints appear in native debugger and both user/agent control the same session.

### S9a — App spike

Retire the biggest unknown early: does the headless core bind to the Zed fork and GPUI at all?

**Exit:** the Zed fork builds as `cedian` on this machine; one GPUI panel inside it spawns OMP through the P1 spawn profile and renders a streamed reply from the existing `Thread` model (no CLI involved); `Version` is mapped to `clock::Global` for one buffer, and a file that OMP's own `edit` writes to disk is imported as ONE agent-attributed Zed transaction (keyed by its `tool_call_id`) that native undo reverts — no OMP change ([ADR-0027](decisions/0027-zero-omp-fork.md)). Findings that change the plan become ADRs before S2 starts.

### S2 — Workflow core

Split per [ADR-0010](decisions/0010-gate-checker-a1-narrow.md) (A1-narrow), stand-in row A.

**Exit:** a bugfix playbook (an OMP skill, [ADR-0025](decisions/0025-playbooks-are-omp-skills.md)) runs reproduce → verify; OMP drives profile/playbook/phases through host tools (P5), cedian stores evidence and evaluates gates (pure, `max_continue: 3`), and an OMP turn that claims completion while a required gate is unmet is blocked (status `blocked`, or `failed` when the agent failed the phase itself — [ADR-0036](decisions/0036-s2-evidence-freshness-and-turn-end-block.md); missing gates surfaced). Evidence is attributed only when it carries a real `tool_call_id` from the router log — CLI-typed evidence is `unattributed` by default. Evidence captured before a later edit to a bound file is `stale` and does not count; `inconclusive` never counts as pass; completion shows the claims ledger ([ADR-0024](decisions/0024-evidence-bound-to-code-state.md)). **Verification profile** ([ADR-0025](decisions/0025-playbooks-are-omp-skills.md)): a gate can require evidence for a feature-map id (`feature <id> proven`); evidence captured from an instance that has not passed the profile's Doctor check since its last failed or surprising drive is `inconclusive`; a profile its generator never ran end to end (launch → doctor → drive → evidence → cleanup) is a draft and cannot satisfy a gate. Fast lane holds: a trivial/small task with no floor gate completes with no workflow at all. **Benchmark** ([ADR-0026](decisions/0026-fast-lane.md), [ADR-0037](decisions/0037-benchmark-without-cursor.md)): 10 real tasks (proposed by the agent from this repo's own commits and issues, approved by the owner before any run), run in cedian alone — time-to-usable-result (first run = baseline), false-done count (budget 0), cedian overhead per turn (budget: median < 1 s, any turn < 3 s, OMP's time excluded), optional owner review time — recorded in the slice notes.

#### Scope from Phase 10 — Workflow Engine

Implement:

```text
TaskProfile
Playbook
Gate
Evidence
WorkflowState
workflow events (generated by cedian from host-tool calls, not emitted by OMP)
```

Acceptance:

```text
Bug Fix
✓ Reproduce
✓ Investigate
✓ Implement
● Verify
○ Review
```

### S3 — Review agents

**Gate (must ALL hold before S3 work starts):**
1. OMP resolver made strict at runtime spawn through the spawn profile (P1, [ADR-0020](decisions/0020-omp-spawn-profile.md)): `--approval-mode` (`always-ask` or `write`), generated overlay with `tools.approval.*`, `bash.patterns` and `computer.enabled: false`, then `set_ask_dialog(true)` over RPC. Verified by the precedence test ([ADR-0028](decisions/0028-spawn-profile-approval-findings.md)): a project `.omp/config.yml` that says yolo / computer-on / `bash: allow` still yields an approval request for `bash` (exec tier, the default `write` profile) and no computer prelude. Open before this item counts: project `tools.approval` allows for tools the overlay does not name (ADR-0028 known gap).
2. Reviewer Seatbelt profile at `policy/reviewer.sbpl`, generated per §64 mechanism 2 (deny-default, only `/usr/bin/sandbox-exec`), plus a **bypass-proof test**: a reviewer process attempting a write inside the workspace and a non-allow-listed `bash` command gets `Deny` from the kernel, not from a policy check.
3. Reviewer `bash` allow-list lives in the user's `cedian.toml` ([ADR-0018](decisions/0018-settings-in-one-toml.md)) beside `[permissions]` — one file, one CI gate.
4. Audit tuple `{timestamp, ordinal, tool, command/prefix, decision, scope}` appended as JSONL to `audit.jsonl` in the state dir for every cedian-gate decision, including `Abstain`; a test replays the file.

**Exit:** an OMP turn spawns at least one reviewer subagent under the reviewer profile (fresh context, a different model from the implementer where OMP routing allows — [ADR-0011](decisions/0011-reviewers-read-only-sandbox.md)); each finding arrives through the `cedian_review_finding` host tool and attaches to the hunk it names; a `blocker` finding keeps the review gate unmet until it is fixed or dismissed with a recorded reason (audit log); triggered from `cedian shell` with `review --agent`. **Correction ledger** ([ADR-0032](decisions/0032-correction-ledger.md)): each rejected hunk, reverted turn, user edit of an agent hunk, refused completion and dismissed finding appends a row to `corrections.jsonl` in the state dir; `cedian_correction_class` refuses a class with fewer than two events from two turns; a class shows `enforced` only with evidence that its check fails on the recorded mistake and passes at head.

#### Scope from Phase 12 — Review Agents

Implement:

```text
ReviewPolicy
correctness reviewer
regression reviewer
security reviewer
architecture reviewer
structured findings
```

Acceptance:

> Findings annotate native diff/editor.

### S4 — Browser evidence

Stand-in row D.

**Exit:** CLI drives a page, captures screenshot + DOM, a gate consumes it as evidence; captures within one session carry a monotonic frame seq so `stale-frame` (ARCHITECTURE §29) is real, not always-fresh.

#### Scope from Phase 9 — Shared Browser

Implement:

```text
cedian-owned Chromium lifecycle
CDP endpoint
OMP browser connects to same Chromium
screencast
mouse/keyboard forwarding
browser pane
console/network integration
element pick → prompt context (Cursor Design Mode, ADR-0034 workflow map)
```

Acceptance:

> User and OMP interact with the same tab.

> User input preempts the agent in a shared tab — rule in [ARCHITECTURE §25](ARCHITECTURE.md#25-browser-architecture) (*[ADR-0007](decisions/0007-shared-browser-and-evidence-frames.md)*).

#### Scope from Phase 11 — Browser Verification

Browser becomes an evidence provider.

Acceptance:

> OMP cannot mark a browser-facing task complete while a required browser verification gate is missing.

### S5 — Parallel workers

Stand-in row C. Swarm/arena are presets — no new engine.

**Exit:** an OMP turn requests a worktree through a host tool and cedian creates it; parallel workers on worktrees are visible (from subagent events) and steerable (a steer reaches the worker's OMP session; a worker is an OMP subagent, [ADR-0050](decisions/0050-workers-are-omp-subagents.md)); merge-back merges into the stated base only, and removing an unmerged worker is refused. Cleanup reads `git worktree list` (never only the registry), classifies each tree as merged / wip (tracked uncommitted edits) / scratch (untracked only), and never deletes wip without an explicit per-tree decision. **Brief contract** ([ADR-0033](decisions/0033-worker-brief-contract.md)): a `cedian_worktree_request` without goal, `scope.write`, acceptance, verify and timebox is refused with no tree created (fake-omp replay); an edit outside `scope.write` is `OutOfScope` and blocks merge-back; a worker past its timebox with no side effect shows `stuck`.

#### Scope from Phase 16 — Subagent / Worktree UI

> Worktree mechanism and visualization land together — [ADR-0009](decisions/0009-worktree-mechanism-vs-policy.md).

Implement:

```text
subagent tree
progress
cancel
steer
worktree mechanism (add/remove, WorktreeId, `cedian_worktree_request` host tool)
worktree visualization
```

Acceptance:

> Parallel OMP workers are visible, while OMP remains the only orchestrator.

#### Scope from Phase 17 — Swarm / Arena

Implement last.

```text
Swarm = task partition preset
Arena = same-task parallel preset
Architect = design Arena preset
Interrogate = ReviewPolicy preset
```

No new engine.

### S6 — PR workspace

**Exit:** open PR → review → comment → Fix → CI green → merge via CLI; PR baselines labelled separately from task baselines; merge is `Deny`-by-default (explicit per-action `Ask`). Green is not verified: a PR lands only with an independent verdict bound to its head SHA, base SHA and `git patch-id` (a rebase voids the verdict), and a stack lands only as the contiguous verified run from the bottom ([ADR-0024](decisions/0024-evidence-bound-to-code-state.md)).

#### Scope from Phase 18 — PR Workspace

> **Rationale (market parity, 2026-10-06).** Synara + Claude Desktop both ship browse/review/merge PRs + stacked PRs + CI auto-fix in-app. cedian's Review Changes (§§15–20) stops at the working tree — the PR is where review actually ships. Cheap to build: `gh` CLI + existing review pipeline, no new engine. *(→ [ADR-0019](decisions/0019-scope-cuts.md))*

Implement (all via `gh`, cedian renders natively) — v1 scope ONLY (Fix-button, stacked-PR position, pinned list are follow-ups with their own acceptance, not this phase):

```text
PR list (per repo)
diff review (reuse §15–20 pipeline on PR range, not task baseline)
inline comments → ReviewFeedback payload (§19) → OMP Fix
CI status bar + auto-fix toggle (read check output → iterate, bounded like §54 max_continue)
merge (explicit confirm; method = squash default)
```

Rules: PR diffs use the PR base as baseline (§16 task-baseline stays for working-tree review — two baselines, labeled in UI, never mixed). Destructive actions (merge, close) are `Deny`-by-default in §64 policy (explicit per-action `Ask`, never auto-allow). CI auto-fix turns count against the task's `max_continue`.

> **Cut from v1 (2026-10-06).** Dropped from v1: Fix-button (comment grouping), stacked-PR position/readiness, pinned repos, safe-prefix merge — each is a separate feature needing its own acceptance; bundling them guarantees a half-done phase.

Acceptance:

> Open PR → review diff → comment → Fix → CI green → merge, without leaving cedian.

### S7 — Local automations

No cloud, ever (ARCHITECTURE §89).

**Exit:** a scheduled run fires while the machine is awake → history + evidence visible on return; same gates and provenance as interactive turns.

#### Scope from Phase 19 — Automations (Scheduled Runs)

> **Rationale + scope (user decision, 2026-10-06).** Synara ships scheduled recurring runs; cedian adopts a LOCAL-only version — no cloud runner, no remote queue. Justification: the machine stays on 24/7, so a local scheduler suffices and avoids an entire second infrastructure (server, auth, billing, remote sandbox). *(→ [ADR-0019](decisions/0019-scope-cuts.md))*

Implement:

```text
schedule (cron expr; plain-language → cron is a follow-up, not v1)
run history (reuse audit-tuple log, §64)
stop conditions (evaluated as pure predicates, §54-style: bounded, no I/O)
consecutive-failure limit (default 3 → auto-pause + notify, same shape as max_continue)
wake = spawn turn in existing workspace (same lifecycle as §71, steps 1–12)
```

Rules: automations run under the SAME permission profile as interactive turns (no privilege elevation for background); `computer` actuation is `Deny` for automation runs until the user explicitly allows per-automation. Every scheduled run emits the same provenance (§17) and evidence (§53) as interactive work — gates apply identically. No cloud/SSH execution, EVER (out of scope, ARCHITECTURE §89). Runs execute inside a live `cedian shell` (P4).

> **Cut from v1 (2026-10-06).** Dropped from v1: NL→cron parsing (cron expr typed directly; OMP translation is its own feature with its own misparse risk), lid-closed guarantee (acceptance is "history + evidence visible on return from sleep", not a power-management promise — no `IOPMAssertion`, no wake-from-sleep; machine-on-24/7 is the user's setup, not cedian's contract).

Acceptance:

> Scheduled run fires while the machine is awake → history + evidence visible on return.

### S8 — iOS extension track

Off the native critical path; ships after v0.1 and never gates it.

**Exit:** OMP builds, launches and drives the app on a simulator through the `ios` tool, semantic UI first, with the simulator visible in cedian.

#### Scope from Phase 13 — iOS Core [EXTENSION TRACK — off the fully-native critical path]

Implement:

```text
devices
boot
build
install
launch
terminate
screenshot
logs
```

Acceptance:

> OMP can build and launch the iOS app without manual external steps.

#### Scope from Phase 14 — iOS Semantic Interaction [EXTENSION TRACK]

Implement:

```text
WDA/XCTest
UI tree
tap
type
swipe
wait
```

Acceptance:

> OMP can reproduce UI flows without relying mainly on pixel coordinates.

#### Scope from Phase 15 — iOS Native Panel [EXTENSION TRACK]

Implement:

```text
ScreenCaptureKit
Simulator window capture
GPUI rendering
input forwarding
```

Acceptance:

> Simulator appears live inside cedian.

### S9 — The real app (fork + GPUI)

Fork hygiene, signing/notarization, app-shell UI, GPUI binding of every headless model, and deletion of stand-ins B and D. THIS — not S1–S8 — is what makes cedian triable as an app.

**Exit:** `cargo run -p cedian -- …` → onboard → prompt → answer with zero terminal, AND every non-extension item of the checklist below is true, AND ⌘K inline edit + revert turn work in the editor, AND the [ADR-0026](decisions/0026-fast-lane.md) benchmark is re-run on the whole app (frame time while streaming, per-turn cedian overhead against the [ADR-0037](decisions/0037-benchmark-without-cursor.md) budgets, the 10 tasks against the S2 baseline).

#### Definition of "fully native" (v0.1 gate)

Do not call cedian “fully native OMP-integrated” until all critical items below are true. iOS is on the extension track and NOT required for this label (tracked separately in §86 iOS Tests):

```text
□ cedian starts OMP automatically
□ no OMP terminal required

□ prompt/stream/cancel native
□ image input native
□ ask native
□ permissions native

□ every OMP tool has a structured UI fallback
□ edit/write integrate editor buffers
□ native undo works
□ Review Changes works per task
□ hunk accept/reject works
□ stale hunk detection works

□ active selection reaches OMP
□ diagnostics available
□ LSP shares Zed state
□ DAP shares Zed state

□ browser agent/user share same tab
□ browser screenshot inline
□ console/network available

□ iOS build/launch/screenshot [EXTENSION — not gating fully-native]
□ iOS semantic UI interaction [EXTENSION — not gating fully-native]
□ simulator state visible in cedian [EXTENSION — not gating fully-native]

□ subagents visible
□ isolated worktrees visible
□ cancel/steer works

□ workflow visible
□ verification evidence visible
□ review findings annotate code

□ OMP crash does not crash IDE
□ restart restores session
□ protocol mismatch fails safely

□ every `cedian/OMP_PARITY.md` row (in the fork) is native, gated with a working opt-in, or upstream-blocked
□ parity test green on the pinned OMP

□ no second harness
□ no ACP dependency in core architecture
□ no duplicate orchestration engine
□ no duplicate context engine
□ no duplicate subagent runtime
```

When this is true:

> **cedian = fully OMP-native agentic IDE.**

#### Scope from Phase 0 — Fork Hygiene

Build:

```text
Zed fork builds
OMP builds from the pinned upstream commit (never forked — [ADR-0027](decisions/0027-zero-omp-fork.md))
pinned upstream remotes
upstream rebase cadence + conflict owner
Zed license audit for redistributed binary (GPL terms)
macOS signing / notarization owner for cedian.app + bundled omp
cedian branding
CI build
UI kit evaluation: pin gpui-kit + elygpui revisions, verify GPUI version compat with Zed fork
keep Zed edit prediction as cedian's inline completion (no cedian-built engine, not routed through OMP — ADR-0023)
```

Acceptance:

```text
`cargo run -p cedian -- ...` (app binary; pre-S9 use `cargo run -p cedian_cli -- ...`)
```

works as a normal Zed-derived editor.

#### Scope from Phase 2.5 — App Shell (Onboarding, Palette, Settings, Updates)

> **Rationale (UI gap review, 2026-10-06).** Phases 1–2 build panes; nothing owns the APP around them (first-run, shortcuts, settings surface, updates). One phase, one owner — the shell, not four scattered features.

Implement:

```text
first-run onboarding (Zed import? → OMP bundled check → bundled-OMP trust banner (§6) → sample workspace)
command palette entries (every agent action: prompt/abort/steer/review/PR/automation — all palette-discoverable)
keybindings (agent panel/composer/tool-card/review navigation; no clash with Zed defaults; user-overridable, versioned with snapshot_version §75)
settings UI (permission TOML + reviewer allow-list + automation schedules + update channel + Seatbelt status line only)
model picker (dropdown beside composer: OMP provider/model routing via set_model; per-task override, persisted per workspace — OMP owns the catalog, cedian renders it)
session manager (new/switch/archive/delete task, resume-checkbox state per §76, storage meter per task — thin UI over open_session/new_session/switch_session)
auto-update (cedian.app + bundled omp as ONE unit: version check → download → verify triple (§6) → relaunch; OMP revision change shows [Review changes] [Continue], never silent)
```

> **Settings audit (2026-10-06).** ADDED vs draft: model picker (no UI existed for set_model/provider routing), session manager (no surface for session lifecycle). CUT: Seatbelt profile viewer — `.sbpl` is a generated artifact, unreadable in UI; replaced by a status line (`enforcing: Seatbelt profile vX`) + open-file button.

Rules: settings UI EDITS the same TOML/policy files the CI policy gate checks (ARCHITECTURE §64) — no second source of truth; palette/shortcut registry reuses gpui-kit action/keybinding primitives (ARCHITECTURE chapter 3, stack lock), never hand-rolled dispatch. Onboarding must complete WITHOUT network (bundled OMP is local; no account, no sign-in).

Acceptance:

> Fresh install → onboarded → prompt → answer, with zero terminal and zero config-file editing.
