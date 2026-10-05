# Repository Guidelines

> Status: plan-only repo. No `src/`, `crates/`, `packages/`, configs, or tests exist. Sole source of truth is `CEDIAN_AGENTIC_IDE_PLAN.md` (~3550 lines, §§1–90). Do not invent build/test/lint commands until configs land.

## Project Overview

> **Stack lock: Rust + GPUI only in the cedian process.** No TypeScript/Electron/WebView/Tauri. TS exists only as OMP-side additions (`packages/coding-agent/src/`, lives in OMP repo). Approved UI accelerators: `gpui-kit` (gpui-base + 75+ gpui-component, Apache-2.0) and `elygpui.com` (Ely components, MIT/Apache-2.0) for panel/composer/tool-card/message/settings/dialog/toast — editor/buffer/multibuffer/diff stay Zed-native.

Cedian = Zed-fork (GPUI/Rust) environment + OMP sidecar as sole intelligence runtime. Rule: `cedian = environment, OMP = intelligence` (plan §§1–2).

> **Upstream-verified facts (2026-10-06, see plan inline notes).** `omp-rpc` Rust client is BLOCKING — bridge handler threads to GPUI async, never UI thread. OMP has NO WorkspaceBackend/LspHost/DapHost seam — integrate via `set_host_tools` + `set_host_uri_schemes` (`cedian://`) + extensions/MCP. OMP approvals default YOLO — Phase 0.5 must configure `always-ask|write` + `set_ask_dialog(true)` first. `set_mode` doesn't exist; `Deep` dropped, `Plan` needs OMP-side addition. `open_session` takes a DIRECTORY (adopt-newest), leaf is ephemeral. Zed versions are `clock::Global` (no `BufferVersion`); review over `DiffPatch`/`BufferDiff`, agent identity via `ReplicaId::AGENT`. `cursor/plugins/pstack` = prompt-only, no mechanism adopted. herdr lessons: pane tri-state from EventRouter, validated respawn argv, `snapshot_version` on all persisted state. Phases 18–19: PR workspace (`gh`, PR-base baselines, merge `Deny`-by-default) + local-only automations (same gates/provenance, no cloud EVER — §91). OMP-only is deliberate: multi-provider lives inside OMP routing, not cedian adapters.

## Architecture & Data Flow

Two processes, tight semantic / loose process coupling (plan §§3–6):
`cedian.app` spawns bundled `omp --mode rpc-ui` over stdin/stdout → OMP RPC v2 typed client → `ready` → negotiate `cedianProtocol`/OMP revision (mismatch = hard fail) → `set_host_services` → subscribe subagents → `open_session` restore.

Core loop: `OMP decides → cedian executes IDE-native op → structured result → OMP continues`.
Example: `OMP edit → cedianWorkspaceHost::apply_edit(path, expected clock::Global, TextEdit) → Zed undo stack → review tracker`.

Streaming: `OMP stdout → background reader → frame decode → EventRouter → channel → batched GPUI updates`. Never mutate GPUI from RPC reader thread; batch text/diffs (~16–33ms flush); CDP screencast with backpressure.

State split: OMP owns transcripts/sessions; cedian stores only `workspace ↔ OMP session ↔ cedian task` mappings + review state. No full-chat duplication.

Hard constraints (plan §88 must-not-build): one harness only. No second agent loop/scheduler/context/subagent/workflow engines, no duplicate browser/LSP/DAP tools, no CLI-stdout parsing, no in-GPUI-process OMP, no generic harness abstraction, no ACP-for-OMP.

## Key Directories

Planned layout (plan §§6–8), none exist yet:

- `crates/cedian_omp/` — process + protocol boundary: `runtime.rs` (`OmpRuntime { client, child, state }`), `rpc.rs`, `event_router.rs`, `session.rs`, `context.rs`, `permissions.rs`, `host_services.rs`
- `crates/cedian_agent/` — `task.rs`, `thread.rs`, `state.rs` (one thread = one OMP session)
- `crates/cedian_agent_ui/` — `panel.rs`, `composer.rs`, `message.rs`, `tool_card.rs`, `workflow.rs`, `subagents.rs`
- `crates/cedian_review/` — `baseline.rs`, `tracker.rs`, `diff.rs`, `provenance.rs`, `findings.rs`; baseline = task baseline, not git HEAD; accept = keep-buffer, reject = inverse patch
- `crates/cedian_browser/` — `session.rs`, `cdp.rs`, `screencast.rs`, `input.rs`, `view.rs` (shared Chromium via CDP, never second engine)
- `crates/cedian_ios/` — `simulator.rs`, `xcodebuild.rs`, `simctl.rs`, `wda.rs`, `capture.rs`, `logs.rs`, `view.rs`
- `packages/coding-agent/src/integrations/cedian/` — OMP-side TS: `host-services.ts`, `context.ts`, `events.ts`, `workspace.ts`, `lsp.ts`, `debug.ts`
- `packages/coding-agent/src/workflow/` + `tools/ios.ts` — `profile.ts`, `playbook.ts`, `gate.ts`, `evidence.ts`, `review.ts`, `parallel.ts`
- `vendor/omp-revision` + `script/build-omp` — pinned OMP commit built to `cedian.app/Contents/Resources/omp`

## Development Commands

> Gate: Phase 0.5 RPC spike must pass before Phase 1 — spawn real `omp --mode rpc-ui`, fill capability table (have/missing/unstable), cut scope or assign owners for missing rows.

No commands exist. Planned/illustrative only (from plan, not runnable):

## Code Conventions & Common Patterns

Prescribed by plan sketches (§§5, 10–12); enforce on new code:

- Rust: `cedian_<area>` snake_case crates, one-concept-per-file (`runtime.rs`, `event_router.rs`, `host_services.rs`, `tool_card.rs`, `simctl.rs`). `async fn prompt/abort/steer/new_session/open_session/set_model`; `trait cedianWorkspaceHost { active_file/selection/buffer_version/apply_edit/write_file/diagnostics }`.
- TypeScript: `integrations/cedian/` kebab files, `PascalCase` interfaces + `camelCase` methods: `HostServices`, `WorkspaceBackend { readText/applyEdits/writeText }`.
- Tool naming: never `cedian_*` duplicates — keep OMP tool names; cedian implements the HOST side via `set_host_tools` + `cedian://` URIs + extensions/MCP (no backend seam inside OMP to swap).
- DI: capability advertisement `{"type":"set_host_services","services":["workspace","lsp","dap","browser_surface","ios"]}`; tools resolve backend at runtime.
- Async/errors: `async` RPC throughout; OMP owns retries/cancellation (`abort`/steer); tolerate unknown events; protocol mismatch / OMP disconnect = explicit user-visible state + restart, never silent fallback.
- Grill R1 rulings (inviolable): worktrees cedian-owned/OMP-requested, mechanism + UI land together in Phase 16 (conflict → STALE, explicit merge-back); provenance cedian-owned, snapshotted at `tool_execution_end` (post-compaction backfill impossible — compaction keeps summaries/TTSR/todos/goal-journal, drops verbatim turns + fine attribution), UNATTRIBUTED via namespaced records + backward scan; permissions strict-wins at the CEDIAN GATE on canonical `Allow|Ask|Deny` + system-only `Abstain` (`Deny > Ask > Allow`; OMP's own resolver is tool-deny → user-deny → yolo/non-yolo tiers — cedian can't reorder it, only refuse at its boundary); `computer` = CUA `cua-driver` backend only, default-deny + per-action ask + audit log; same-tab browser = user input preempts agent (`browser_input_preempted` + resume banner).
- Grill R3 rulings: `computer` lands atomic — driver + Seatbelt profile + bypass-proof hermetic test ship together or not at all (adapt `seatbelt_base_policy.sbpl` + `seatbelt.rs` composition; hardenings: canonicalize-then-compare, literal+subpath write-exclusions, fail-closed proxy, only `/usr/bin/sandbox-exec`). Policy self-tests agent-drafted, CI-gated (red blocks merge); dev reviews `Deny` justifications. Execpolicy exact semantics: 3 builtins, `deny` valid ONLY in `network_rule`, per-rule ephemeral validation, no-match ⇒ decision omitted. Audit = Codex `{timestamp, ordinal, item}` envelope + cedian decision-tuple extension.
- Scrutinize R2/R3 fixes: Phase 4 (edit backend) precedes Phase 5 (review); gates are pure functions (no I/O) with `max_continue: 3` anti-loop; reviewers under OS-enforced `Deny` profile + allow-listed bash; every `Evidence` carries `provenance` (required gates reject `unattributed`); hunk states ordered `INTERRUPTED > UNATTRIBUTED > STALE`; `ask` is a lease (timeout/disconnect → `Abstain` = deny-for-execution, blocked-for-gates).
- Scrutinize R4 fixes: crash-during-stream reconciles via EventRouter log (`INTERRUPTED` → `UNATTRIBUTED`, `max_continue` reset); `set_mode` parity measured in Phase 0.5 (mid-turn switch = abort + new turn); lifecycle steps 9–10 are typed protocol (unknown events counted, filter mismatch fails loud); bundled omp verified by `{commit, tree-hash, builder}` triple + first-run update banner; browser evidence bound to CDP frame id (`stale-frame` re-capture); agent bash ≠ user PTY (scrubbed env, grants bind agent-context only).
- Reviewers read-only with structured findings; completion gates require evidence (`evidence-as-data`).

## Important Files

- `CEDIAN_AGENTIC_IDE_PLAN.md` — the spec: §§1–12 architecture/ownership, §§63–77 lifecycle/threading, §§25–37 browser/iOS, §§39–62 workflow/verification, §84 roadmap Phases 0–17, §85 MVPs v0.1–v0.3, §86 test layers, §87 native checklist, §88 must-not-build
- Planned entry: `crates/cedian_omp/runtime.rs` (spawn `omp --mode rpc-ui`, handshake, crash respawn + `open_session` restore)
- Planned contracts: `crates/cedian_omp/rpc.rs`, `event_router.rs`, `host_services.rs`

## Runtime/Tooling Preferences

- Planned runtime: Rust/Cargo (Zed/GPUI fork) + pinned OMP sidecar; OMP-side additions in TypeScript. No toolchain pinned, no `Cargo.toml`/`package.json`/lockfiles yet.
- No linter/formatter, Dockerfile, CI, `.editorconfig`, `LICENSE`, `.gitignore` configured.
- Package manager undecided. Do not add Bun/Node/Cargo conventions until the workspace lands.

## Testing & QA

> Anti-flake rule: hermetic by default — fake `omp --mode rpc-ui` replaying recorded frames + session fixtures. Layers 1–2 never call models; live-model cases tagged + nightly-only, never gating merge.

No harness configured. Do not invent `npm test`/vitest/jest commands.

Intended five layers when code lands (plan §86):
1. RPC contract: spawn, ready, negotiation, chunking, event order, cancel, restart, large output, unknown events, session restore
2. Editor integration: open/dirty-file edits, undo, hunk accept/reject, concurrent edits, delete/rename, large/multi-file
3. Agent lifecycle: prompt, steer, follow-up, abort, restore, compaction, subagents, background jobs, crash/reconnect
4. Browser: navigate/DOM/click/type/reload/screenshot/console/network, rapid frames, tab switching, restart
5. iOS: boot/build/install/launch, UI tree, tap/type/swipe, screenshot, logs, crash/simulator-restart/build-failure

No coverage tool or threshold. §87 definition-of-native checklist (crash recovery, mismatch fails safely, hunk accept/reject) serves as acceptance proxy.
