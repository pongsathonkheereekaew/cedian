# cedian — Architecture

**cedian is a Cursor-style agentic IDE — the same agent + IDE workflow and product features — built for OMP: OMP runs as upstream ships it, every OMP feature gets a native surface, and Zed is forked only as far as OMP needs. OMP decides; cedian executes, renders, and verifies.** Built in Rust + GPUI on a Zed fork. Identity and OMP parity: [ADR-0034](decisions/0034-identity-and-omp-parity.md); scope and priorities: [ADR-0023](decisions/0023-product-scope.md). Per-feature OMP coverage: [`OMP_PARITY.md`](OMP_PARITY.md).

> **cedian = environment. OMP = intelligence.** A minimal Zed fork turned into a fully native agentic IDE, powered by one and only one harness: OMP (Oh My Pi). cedian is personal: no other agent harnesses, no ACP agents, no generic third-party runtimes, no public agent marketplace. The user experiences one product: **cedian**.

This file holds the rules that are true NOW. Why a rule exists lives in [`decisions/`](decisions/) (ADRs); what is scheduled lives in [`ROADMAP.md`](ROADMAP.md); what is done lives in [`../README.md`](../README.md) only. Section numbers (`§NN`) are kept from the original plan so existing references in code and commits still resolve.

## Contents

1. [Principles & ownership](#1-principles--ownership)
2. [Non-goals](#2-non-goals)
3. [Stack & repo layout](#3-stack--repo-layout)
4. [Runtime & protocol](#4-runtime--protocol)
5. [Crash, persistence & resume](#5-crash-persistence--resume)
6. [Host boundary & context](#6-host-boundary--context)
7. [Editor, buffers & review](#7-editor-buffers--review)
8. [Language services](#8-language-services)
9. [Terminal](#9-terminal)
10. [Browser](#10-browser)
11. [Agent UI](#11-agent-ui)
12. [Parallel work](#12-parallel-work)
13. [Workflow & verification](#13-workflow--verification)
14. [Review agents](#14-review-agents)
15. [Safety: permissions & computer use](#15-safety-permissions--computer-use)
16. [iOS (extension track)](#16-ios-extension-track)
17. [Testing](#17-testing)
A. [Appendix: worked examples](#a-appendix-worked-examples)

---

## 1. Principles & ownership

### §1 Final Product Vision

cedian should not feel like:

```text
Zed
+ OMP CLI
+ Browser plugin
+ Simulator plugin
+ Agent sidebar
```

It should feel like:

```text
┌─────────────────────────────────────────────────────────────┐
│                         cedian                               │
│                    Zed fork + GPUI                          │
│                                                             │
│  Editor       Agent       Review       Browser              │
│  Terminal     Debugger    Git          iOS                  │
│  LSP          Search      Workflow     Subagents            │
│                                                             │
│               cedian Native Host Layer                       │
└──────────────────────────┬──────────────────────────────────┘
                           │
                       OMP RPC v2
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                           OMP                               │
│                                                             │
│  Agent loop       Models          Context                   │
│  Tool registry    Subagents       Sessions                  │
│  Memory           Task isolation  Skills                    │
│  Workflow         Verification    Review policy             │
│                                                             │
│                   ONE HARNESS ONLY                          │
└─────────────────────────────────────────────────────────────┘
```

From the user's perspective:

```text
cedian is the IDE.
OMP is cedian's native agent runtime.
```

There should be no visible concept of an “external agent” for the default workflow.

### §2 Core Architecture Rule

Use this ownership model everywhere:

##### OMP owns

```text
agent loop
model calls
tool decisions
context/token budget
subagents
workflow (profile, playbook, phases)
verification (gathering evidence)
review reasoning
task isolation
sessions
memory
skills/rules
model routing
retries
cancellation
execution semantics
```

##### cedian owns

```text
editor UI
buffers
panes/windows
review UI
browser presentation
iOS presentation
terminal presentation
debugger presentation
Git presentation
notifications
native dialogs
native GPUI rendering
workspace surfaces
```

The interaction pattern is:

```text
OMP decides
   ↓
cedian executes IDE-native operations
   ↓
cedian returns structured result
   ↓
OMP continues reasoning
```

This avoids creating two harnesses.

One boundary exception: **verification checking** — evaluating gates over stored evidence and refusing completion — sits at the cedian boundary, because the party being checked must not be its own checker. OMP still decides *how* to obtain evidence (§46, [ADR-0010](decisions/0010-gate-checker-a1-narrow.md)).

### §79 No Generic Harness Abstraction

Because OMP is the only harness:

Do not create:

```text
AgentRuntime trait
AcpRuntime
ClaudeRuntime
CodexRuntime
RemoteRuntime
```

unless a concrete OMP-internal need appears.

cedian may directly know:

```rust
struct OmpClient {
    ...
}
```

This is acceptable and desirable for this personal project.

### §90 Final Architecture

```text
                         cedian
                      Rust + GPUI
                           │
       ┌───────────────────┼────────────────────┐
       │                   │                    │
     Editor              Browser               iOS
       │                   │                    │
      LSP               Chromium           Simulator
       │                   │                    │
      DAP                  │                WDA/XCTest
       │                   │                    │
     Review                │                    │
       └───────────────────┼────────────────────┘
                           │
                    Native Host Services
                           │
                      OMP RPC v2
                           │
                           ▼
                    ┌─────────────┐
                    │     OMP     │
                    │             │
                    │ Agent Loop  │
                    │ Models      │
                    │ Context     │
                    │ Tools       │
                    │ Subagents   │
                    │ Worktree    │
                    │  policy*    │
                    │ Workflow    │
                    │ Verify      │
                    │ Review      │
                    └─────────────┘
```

\* Worktree *policy* (when to isolate, merge strategy) is OMP's; worktree *mechanism* (`git worktree add/remove`, `WorktreeId`) is cedian's (§43 R1).

### §91 Final Design Principle

The final product should satisfy:

```text
cedian = environment
OMP   = intelligence
```

cedian should not merely “run OMP inside Zed”.

OMP should understand and control the IDE's real primitives:

```text
editor
buffers
review
LSP
debugger
browser
simulator
terminal
worktrees
workflow
verification
```

cedian renders all of them as one coherent GPUI workspace.

The user should never need to think about whether a capability belongs to Zed, OMP, Chromium, Xcode, DAP, LSP, or the simulator.

They should only see:

```text
cedian
```

That is the target architecture.

---

## 2. Non-goals

### §88 Things We Explicitly Must Not Build

Avoid:

```text
❌ rewrite OMP agent loop

❌ cedian agent scheduler

❌ second context engine

❌ second subagent runtime

❌ second workflow engine

❌ duplicate browser tool

❌ duplicate LSP tool under another name

❌ duplicate debugger tool

❌ ACP for OMP

❌ CLI stdout parsing as the product protocol

❌ OMP running inside the GPUI process

❌ generic harness abstraction

❌ support for other agent harnesses “just in case”

❌ rewrite Zed editor

❌ rewrite GPUI

❌ custom browser engine

❌ custom LSP

❌ custom DAP

❌ dozens of playbooks on day one
```

### §89 Out of Scope (Deliberate Cuts)

Decided 2026-10-06, not deferred — these return only if the destination is redrawn ([ADR-0019](decisions/0019-scope-cuts.md)):

```text
❌ cloud/SSH sessions — machine stays on 24/7; local scheduler (slice S7) covers recurrence. No remote runner, queue, or billing, EVER.
❌ second harness adapters — OMP already routes providers/models internally; cedian speaks OMP, OMP speaks the world.
❌ custom browser engine (Servo evaluated 2026-10-06: no full CDP, web-compat gap — revisit only if it speaks CDP completely)
❌ Ghostty-as-terminal (Zig FFI for what Zed PTY already gives — evaluated 2026-10-06)
❌ VM sandbox for computer tool (Lume/Spaces deferred past CUA driver-only landing)
```

**Daemon track** (survive quit AND reboot via a launchd engine) is deferred until users report reboot-lost work after quit-resume ships — trigger and cost in the ADR. *(→ [ADR-0019](decisions/0019-scope-cuts.md))*

---

## 3. Stack & repo layout

### §7 Zed Fork Strategy

Do not rewrite Zed.

Keep as much upstream architecture as possible:

```text
GPUI
Editor
Buffers
Project/worktree
LSP
DAP
Terminal
Git
Tasks
Search
Language support
Keymaps
Window management
```

Patch Zed core only at clean integration points.

**Fork scope.** Zed is forked only as far as OMP needs: a core patch exists only to give an OMP capability a native surface or to bind a cedian crate at a registration point ([ADR-0030](decisions/0030-fork-layout-and-build-profile.md)). Every other Zed feature behaves as upstream ships it, and the fork rebases on upstream Zed on a fixed cadence (S9 fork hygiene). *(→ [ADR-0034](decisions/0034-identity-and-omp-parity.md))*

Create cedian-owned crates instead of scattering custom logic everywhere.

Real crate layout (one crate per area, one file per concept):

```text
crates/
  cedian_omp/        runtime.rs  event_router.rs  session.rs  respawn.rs  errors.rs
  cedian_agent/      task.rs  thread.rs  state.rs
  cedian_agent_ui/   panel.rs  composer.rs  message.rs  tool_card.rs  ask.rs  text_buffer.rs
  cedian_workspace/  host.rs  buffer.rs  uri.rs  ambient.rs  lsp_bridge.rs  service.rs
  cedian_review/     baseline.rs  tracker.rs  diff.rs  provenance.rs  findings.rs
  cedian_workflow/   profile.rs  playbook.rs  gate.rs  evidence.rs  state.rs
  cedian_browser/    process.rs  cdp.rs  session.rs
  cedian_worker/     registry.rs  worktree.rs
  cedian_shell/      settings.rs  palette.rs  session_manager.rs
  cedian_lsp/        headless stand-in (ROADMAP row B)
  cedian_dap/        headless stand-in (ROADMAP row B)
  cedian_cli/        throwaway harness — dies at S9
vendor/omp-rpc/      vendored upstream Rust RPC client (pin: vendor/omp-revision.json)
```

Planned, not yet created: `cedian_ios/` (S8: simulator, xcodebuild, simctl, wda, capture, logs, view), permissions/sandbox policy (S3), browser screencast/input/view (S9).

This minimizes upstream merge pain.

**Stack lock: Rust + GPUI only.** No TypeScript/Electron/WebView/Tauri in the cedian process. There is no TypeScript anywhere in cedian's scope: OMP is used as upstream ships it ([ADR-0027](decisions/0027-zero-omp-fork.md)). `gpui-kit` (gpui-base unstyled primitives + gpui-component, Apache-2.0, `github.com/longbridge/gpui-kit`) and `elygpui.com` (Ely GPUI components, MIT/Apache-2.0) are approved UI accelerators: prefer them over hand-rolling panel/composer/tool-card/message/settings/dialog/toast components, but editor/buffer/multibuffer/diff surfaces stay Zed-native. *(→ [ADR-0001](decisions/0001-environment-vs-intelligence.md))*

### §8 No OMP Fork

cedian runs the pinned upstream OMP and never forks it; only Zed is forked ([ADR-0027](decisions/0027-zero-omp-fork.md)). Everything cedian needs from OMP goes through, in order:

1. **Host tools + `cedian://` URIs** registered over RPC ([ADR-0022](decisions/0022-host-tool-first.md)) — including `ios` (§32).
2. **OMP skills** in `.omp/skills/` — playbooks and the project verification profile ([ADR-0025](decisions/0025-playbooks-are-omp-skills.md)).
3. **Spawn profile** — argv + config overlay ([ADR-0020](decisions/0020-omp-spawn-profile.md)).
4. **Importing OMP's own effects** — e.g. disk writes from `edit`/`write` become agent transactions (§12).

A real need for an OMP change goes upstream as a PR; until it ships, cedian works around it or does without.

---

## 4. Runtime & protocol

### §3 Process Architecture

Do **not** embed the entire OMP runtime into the GPUI/Zed process.

Use:

```text
cedian.app
   │
   ├── GPUI / Zed IDE process
   │
   └── spawn
        ↓
      OMP runtime process
```

Why:

- OMP crash must not crash the editor.
- Model-provider hangs must not freeze cedian.
- Tool deadlocks should remain isolated.
- OMP can be restarted independently.
- OMP can be rebuilt independently during development.
- Session restoration becomes easier.
- Memory leaks are isolated from the IDE.
- Runtime lifecycle is easier to test.

The principle is:

> **Tight semantic integration, loose process coupling.**

### §4 Transport: Use OMP RPC v2

Do not invent a second custom transport unless absolutely necessary.

Use OMP's RPC mode:

```text
cedian.app
   │
   │ spawn child
   ▼
omp --mode rpc-ui
   │
 stdin/stdout
   │
 RPC v2
```

Reuse OMP capabilities such as:

- request correlation
- streaming events
- cancellation
- protocol negotiation
- frame chunking
- backpressure
- subagent events
- host tools
- host URIs
- UI request/response
- prompt images
- session restore
- generated Rust client

### §5 Use the Generated Rust OMP RPC Client

cedian should not manually parse OMP wire messages if a generated client exists.

> **Upstream fact (oh-my-pi `sdk/rust/omp-rpc`).** The upstream crate is a BLOCKING client (`Client::spawn`/`prompt_and_wait` on threads, v2 auto-negotiation, host tools/URIs dispatched on handler threads, SIGTERM→1s→SIGKILL teardown), generated from `wire/rpc-wire.schema.json` via `bun run gen:rpc`. There is no async client: cedian bridges blocking handler threads to GPUI async — never on the UI thread — pins the schema revision alongside `vendor/omp-revision.json`, and re-runs generation on every bump. *(→ [ADR-0002](decisions/0002-omp-child-process-rpc-v2.md))*

Target shape (adapted — blocking client behind a thread bridge, not direct async):

```rust
struct OmpRuntime {
    client: omp_rpc::Client, // blocking; lives on a dedicated I/O thread
    child: Child,
    state: OmpRuntimeState,
}
```

Example methods (each hops blocking-call → GPUI async via channel; see §72):

```rust
impl OmpRuntime {
    async fn prompt(...)
    async fn abort(...)
    async fn steer(...)
    async fn new_session(...)
    async fn open_session(...) // directory-adopt, not file-restore (§75 caveat)
    async fn set_model(...)
}
```

The cedian side should depend on typed OMP RPC contracts rather than terminal text parsing.

### §6 Pin OMP to cedian

Because cedian is personal and OMP is the only harness, pin the known OMP build to the cedian build.

Example:

```text
cedian repo
│
├── crates/
│
├── vendor/
│    └── omp-revision
│
└── script/
     └── build-omp
```

Build pipeline:

```text
known OMP commit
    ↓
build
    ↓
cedian.app/Contents/Resources/omp
```

At startup:

```text
cedian
  ↓
spawn bundled OMP
  ↓
ready
  ↓
verify protocol
  ↓
start
```

Optional handshake:

```json
{
  "type": "ready",
  "cedianProtocol": 1,
  "ompRevision": "..."
}
```

> **Bundled-binary trust.** The bundled `omp` is a silent privilege boundary: it inherits the user's uid + Seatbelt profile at spawn. Therefore: (a) `script/build-omp` records `{commit, normalized-source-tree hash, builder identity}` into `vendor/omp-revision.json` — the handshake verifies this triple, not just a version string; (b) at first run after an OMP revision change, cedian shows `OMP runtime updated <old→new> [Review changes] [Continue]` — protocol mismatch AND revision change both fail closed, never silently fall back. *(→ [ADR-0003](decisions/0003-bundled-omp-pin-and-trust.md))*

The `ready` frame carries no OMP revision, so the triple is checked by hashing the bundled binary at startup against the hash recorded beside it in `vendor/omp-revision.json`; the handshake itself checks the protocol version ([ADR-0003](decisions/0003-bundled-omp-pin-and-trust.md)).

If there is a mismatch:

```text
OMP runtime version mismatch
```

Fail clearly rather than silently falling back.

### §71 Runtime Lifecycle

When opening a project:

```text
1. cedian opens workspace
2. start OMP child with the cedian spawn profile (argv + overlay, [ADR-0020](decisions/0020-omp-spawn-profile.md))
3. wait ready
4. negotiate RPC v2
5. verify cedian protocol version
6. register host tools (`set_host_tools`, the complete cedian set)
7. register host URI schemes (`set_host_uri_schemes`)
8. enable ask dialog
9. subscribe subagents
10. apply event filters
11. restore OMP session if one exists
12. ready
```

> **Steps 9–10 are explicit protocol.** `subscribe subagents` and `apply event filters` are typed RPC calls with versioned schemas, not log lines. Unknown event types after filtering = tolerated + counted (metric `cedian.unknown_event_total{event_type}`), never crash, never silent-drop without the counter. A filter schema mismatch fails startup loudly (same path as protocol mismatch, §6) — filters must never silently narrow to zero events. *(→ [ADR-0002](decisions/0002-omp-child-process-rpc-v2.md))*

### §72 Event Router Threading

Do not mutate GPUI directly from the RPC reader thread.

Use:

```text
OMP stdout
 ↓
background reader
 ↓
frame decoder
 ↓
EventRouter
 ↓
channel
 ↓
batched GPUI updates
```

### §73 Streaming Text Batching

Do not repaint for every token.

Use:

```text
TextDeltaBuffer
```

Flush approximately every frame or every ~16–33 ms.

Speed is measured, not claimed ([ADR-0026](decisions/0026-fast-lane.md)): UI frame time never blocks while an agent streams; cedian's per-turn overhead (spawn, context assembly, edit apply, diff rebuild) is recorded and must beat Cursor's with the same model; the OMP session stays warm (no spawn per prompt).

---

### Headless process shape (pre-S9)

All headless work that needs live state (an open OMP session, a browser tab, a warm language server, a scheduler) runs inside ONE foreground `cedian shell` process per workspace; one-shot CLI commands are limited to store-backed or stateless operations and refuse to mutate while a live shell holds `.cedian/shell.lock`. Not a daemon — it dies with its terminal ([ADR-0021](decisions/0021-headless-host-process.md)).

---

## 5. Crash, persistence & resume

### §74 Crash Isolation

If OMP crashes:

```text
cedian editor stays alive
```

Show:

```text
Agent runtime disconnected

[Restarting...]
```

Recovery:

```text
spawn OMP
open_session
restore transcript
re-register host services
restore review state
```

> **Crash-during-stream.** `open_session` restores the transcript, NOT the in-flight stream. On reconnect cedian MUST: (1) discard the partial `TextDeltaBuffer` (never render half a tool call as complete), (2) reconcile: any `tool.started` without matching `tool.completed` in the EventRouter log is marked `INTERRUPTED` and its partial edits become `UNATTRIBUTED` (§17) until re-driven, (3) reset `max_continue` counters tied to the interrupted turn (§54) — a crash must not consume the agent's retries. Covered by Agent Lifecycle Tests (`runtime crash`, `runtime reconnect`). *(→ [ADR-0016](decisions/0016-crash-persistence-resume.md))*

### §75 Session Persistence

OMP owns transcripts.

cedian stores mappings:

```text
workspace
 ↔
OMP session
 ↔
cedian task
```

Avoid duplicating full chat history unless necessary.

Mechanisms adopted from herdr (`herdrdev/herdr`); research in the ADR:

1. **Task status is a first-class signal.** Tri-state `working | blocked | idle`, derived from EventRouter state (running turn → `working`, pending ask/abstain → `blocked`, else `idle`). A `blocked` task surfaces in the panel without opening it — pushed, never polled. A bool `needs_attention` resets on task-open (no attention counter in v1; add one only when multi-window needs cross-window ordering).
2. **Respawn argv is validated, never concatenated.** The `omp --mode rpc-ui` command line is built from a validated struct (absolute paths, ≤64 args, ≤8KiB, no control characters/apostrophes); `{binary_path, session_dir, cwd}` is the dedupe key so two workspaces never share a runtime.
3. **Versioned snapshots.** Every persisted cedian store (workspace ↔ session ↔ task mapping, review baseline, composer drafts) carries `snapshot_version: u32`; a mismatch fails closed with "state too old, re-baseline". *(→ [ADR-0016](decisions/0016-crash-persistence-resume.md))*

### §76 cedian Crash / Shutdown

OMP is a child process.

Pattern adopted from Synara's quit dialog + guarded resume (bounded quit-wait, eligibility rules) — see the ADR. *(→ [ADR-0016](decisions/0016-crash-persistence-resume.md))*

When the user quits with running turns:

```text
quit requested
 ↓
dialog lists running tasks (Cancel / Quit)
 ↓
[✓] Resume on next launch (persisted choice)
 ↓
bounded wait (record resume-intent per task, else mark interrupted)
 ↓
stdin closes → OMP persists/disposes session
```

On restart:

```text
open_session(...) per task
 ↓
eligible ⇒ turn resumes as blocked "resumed after restart — [Continue] [Dismiss]"
ineligible (completed / archived / newer work exists / project gone / intent unrecorded) ⇒ stays interrupted, visible in history
```

Rules: resume defaults to BLOCKED requiring one click (never auto-continue mutating work while the user was away); `computer` actuation from a resumed turn requires fresh `Ask` even if previously granted (grants don't survive restart). Crash (not quit) follows §74 reconcile, then the same blocked-resume.

> **Composer drafts (plan gap, added 2026-10-06).** The transcript restores via OMP — but unsent composer text had NO store. Every composer persists its draft per task (debounced ~500ms, `snapshot_version`-stamped §75) including attachments/refs; on reopen the draft restores verbatim with an "unsent draft" hint. Drafts die with their task (archived/deleted ⇒ draft deleted, never orphaned). *(→ [ADR-0016](decisions/0016-crash-persistence-resume.md))*

When cedian closes:

```text
stdin closes
 ↓
OMP persists/disposes session
```

On restart:

```text
open_session(...)
```

---

## 6. Host boundary & context

### §9 Tool Coverage

Target: every OMP tool stays available and gains a native surface; cedian is the source of truth only where the IDE owns the state. Coverage of every OMP feature, tools included, is tracked in [`OMP_PARITY.md`](OMP_PARITY.md) ([ADR-0034](decisions/0034-identity-and-omp-parity.md)).

| Capability | Owner | Integration |
|---|---|---|
| `read` | OMP | OMP tool + cedian workspace awareness |
| `edit` | OMP | OMP writes disk; cedian imports each write as an agent-attributed Zed transaction — undo, review, provenance ([ADR-0027](decisions/0027-zero-omp-fork.md)) |
| `write` | OMP | same import path as `edit` |
| `grep/glob/find` | OMP | existing OMP |
| `ast_grep/ast_edit` | OMP | existing OMP |
| `bash` | OMP | existing OMP + cedian tool card |
| `eval` | OMP | existing OMP |
| `lsp` | OMP | reads Zed language-server state via `cedian://` + host tools (§21) |
| `debug` | OMP | reads/writes the Zed debug session via `cedian://` + host tools (§22) |
| `task` | OMP | native OMP subagents |
| `hub/wait` | OMP | native |
| `todo` | OMP | projected into Workflow UI |
| `ask` | OMP | native GPUI dialog |
| `browser` | OMP | shared Chromium/CDP + cedian browser surface |
| `computer` | OMP | CUA-driver backend (`trycua/cua` `cua-driver` Rust crates, MIT) behind OMP tool semantics — default-deny, per-action `ask`, audit-logged; never raw OS input outside the driver contract. Until the atomic landing it is disabled with `computer.enabled: false` in the cedian spawn overlay — it is an `eval` prelude, not a separate tool, so a tool allow-list cannot remove it ([ADR-0020](decisions/0020-omp-spawn-profile.md)). **Driver-only first** — `cua-driver` (inspect + operate via typed contract) now; Lume/Spaces VM sandbox is deferred (§89). **Pin:** pin `cua-driver` by git rev + cargo vendor, update on a fixed cadence alongside `vendor/omp-revision.json` — never float on latest. **Opt-in:** in a project with `policy = "omp"` in `cedian.toml`, the user's OMP config decides whether OMP's own prelude runs ([ADR-0035](decisions/0035-omp-native-approval-opt-in.md)). |
| Git/GitHub | OMP + cedian | OMP execution, cedian visualization |
| Images | OMP | native RPC image content |
| Review | OMP + cedian | OMP reasoning, cedian UI |
| iOS | cedian host tool `ios` (§32) | cedian iOS host service |
| Workflow | OMP (profile, playbook, phases, evidence gathering) | cedian renders it (§46) |
| Verification | cedian checks, OMP gathers | cedian stores evidence, evaluates gates, blocks completion (§54–55, [ADR-0010](decisions/0010-gate-checker-a1-narrow.md)) |
| Arena/Swarm | OMP | presets over existing `task` |

### §10 Do Not Duplicate Tool Names

> **Upstream fact (oh-my-pi has NO WorkspaceBackend/LspHost/DapHost seam, verified 2026-10-06).** OMP owns its `edit`/`lsp`/execution tools end-to-end; there is no backend interface to swap inside OMP. The integration surface is OUTSIDE OMP's tools: `set_host_tools` (cedian ops registered as host tools, mountable as `xd://`), `set_host_uri_schemes` (`cedian://` virtual files), and extensions/MCP. So "backend swap" below does NOT mean patching OMP's EditTool — cedian implements the host side and imports OMP's own effects; OMP itself is never modified ([ADR-0027](decisions/0027-zero-omp-fork.md)). LSP/DAP stay OMP-owned tools reading Zed state via `cedian://` URIs + host tools, not injected backends. *(→ [ADR-0004](decisions/0004-integrate-via-host-tools-and-uris.md))*

Do not create user-visible model tools such as:

```text
edit
cedian_edit

lsp
cedian_lsp

debug
zed_debug
```

That would create ambiguity for the model.

Instead:

```text
LLM
 ↓
edit
 ↓
OMP EditTool (unchanged, OMP-owned) writes the file
 ↓ (tool_execution_end carries tool_call_id)
cedian imports the write as an agent transaction (undo, review, provenance)
```

Keep OMP tool semantics stable; cedian implements the host side of the boundary.

### §11 Introduce Internal Host Services

> **Upstream fact (concrete RPC ops, oh-my-pi `modes/rpc/host-tools.ts` + `host-uris.ts`, verified 2026-10-06).** `set_host_tools` REPLACES the whole set (survivor enabled-state kept; names must be unique, no clash); out: `host_tool_call{toolCallId,toolName,arguments}` + `host_tool_cancel`, in: `host_tool_update`/`host_tool_result{isError}`. `set_host_uri_schemes` likewise replaces (lowercased scheme names; `writable`/`immutable` default false); out: `host_uri_request{read|write,url,content?}`, in: `host_uri_result{content,contentType text/plain|markdown|json,notes,immutable,isError}`. Built-ins `local:// skill:// artifact:// security:// mcp://` are RESERVED — `cedian://` is free. Note: OMP `edit` does NOT target host URIs (use host tools for writes). *(→ [ADR-0004](decisions/0004-integrate-via-host-tools-and-uris.md))*

Host services should not necessarily appear as separate LLM tools.

Concept:

```ts
interface HostServices {
  workspace?: WorkspaceHost;
  lsp?: LspHost;
  dap?: DapHost;
  browserSurface?: BrowserSurfaceHost;
  ios?: IosHost;
}
```

cedian announces capabilities via `set_host_tools` + `set_host_uri_schemes` ONLY. There is no `set_host_services` command upstream (see the upstream fact above) — "host service" is a cedian-internal grouping of host tools + URI kinds, never a wire message:

```text
workspace       → host tool cedian_apply_edit + cedian://buffer|selection|active-file|open-editors
lsp             → cedian://diagnostics + cedian://symbols
dap             → (host tools, slice S1)
browser_surface → cedian://browser/current (slice S4/S9)
ios             → (extension track)
```

OMP tools reach these through host tools / `cedian://` reads; OMP's own tool implementations are not swapped (§10).

### §14 Keep OMP Read Capabilities

OMP `read` likely supports much more than normal text files:

```text
files
directories
URLs
PDFs
images
archives
SQLite
internal URIs
notebooks
snapshots
```

Do not replace the entire tool.

Use routing:

```text
OMP ReadTool

URL            → OMP
PDF            → OMP
image          → OMP
archive        → OMP
artifact://    → OMP

normal project text
        ↓
cedian:// buffer reads when unsaved state matters (§13, §40)
```

### §39 Ambient Context

Do not dump the entire IDE state into every prompt.

Use a small snapshot:

```rust
struct AmbientContext {
    active_file: Option<Path>,
    selection: Option<TextRange>,
    diagnostics_summary: Vec<Diagnostic>,
    browser: Option<BrowserContext>,
    simulator: Option<IosContext>,
}
```

Before prompt:

```text
cedian
 ↓
small context snapshot
 ↓
OMP
```

### §40 Deep Context via Host URIs

Expose on-demand context:

```text
cedian://selection
cedian://active-file
cedian://diagnostics
cedian://browser/current
cedian://ios/current
cedian://review/current
```

This lets OMP retrieve richer context only when needed.

### §77 Config Ownership

Do not duplicate agent configuration.

Avoid:

```text
.cedian/rules
+
.omp/rules
```

Use OMP config for agent behavior:

```text
.omp/
  AGENTS.md
  RULES.md
  config.yml
  skills/
  agents/
```

cedian config should mainly cover IDE presentation:

```text
theme
layout
font
browser placement
simulator placement
keymaps
window state
```

The cedian spawn overlay ([ADR-0020](decisions/0020-omp-spawn-profile.md)) is GENERATED from `cedian.toml` into a cedian-owned path at spawn; cedian never writes the user's `.omp/`. The per-project opt-in to OMP's own policy (`policy = "omp"`, [ADR-0035](decisions/0035-omp-native-approval-opt-in.md)) lives in the user's `cedian.toml` only; no file inside a workspace can set it.

Permission policy + sandbox profiles live on the cedian side (TOML + Seatbelt `.sbpl`, §64): they are enforced at the IDE process's OS layer, so they version with cedian, not `.omp/`. OMP never ships its own copy.

---

## 7. Editor, buffers & review

### §12 Editor and Buffer Integration

Final target ([ADR-0027](decisions/0027-zero-omp-fork.md)):

```text
OMP edit (writes disk)
   ↓
cedian imports it as ONE agent transaction (Buffer::reload at tool end, keyed by tool_call_id — ADR-0029)
   ↓
Zed undo stack
   ↓
Review Changes
```

Not merely:

```text
OMP writes filesystem
Zed silently reloads the file (no agent identity, no undo entry, no provenance)
```

Add a workspace host abstraction.

Example (Zed-native types — `clock::Global`, not a `BufferVersion` newtype):

```rust
trait cedianWorkspaceHost {
    fn active_file() -> Option<ProjectPath>;

    fn selection() -> Option<Selection>;

    fn buffer_version(
        path: ProjectPath
    ) -> clock::Global; // text::BufferSnapshot.version (CRDT version vector)

    async fn apply_edit(
        path: ProjectPath,
        expected_version: clock::Global,
        edit: TextEdit, // applied as text::Transaction via History::start/end/push
    ) -> ApplyEditResult;

    async fn write_file(...);

    async fn save(...);

    async fn diagnostics(...);
}
```

> **Upstream fact (zed `crates/text`, `crates/clock`, `crates/buffer_diff`, `crates/acp_thread/src/diff.rs`, verified 2026-10-06).** No `BufferVersion` type exists — versions are `clock::Global` on `text::BufferSnapshot.version`; undo/txn API is `History::{start,end,push,group}` + `Transaction{id: Lamport, edit_ids, start: Global}`; there is no `AgentDiff` symbol — agent review is `acp_thread::diff::{DiffPatch,DiffPatchFile,DiffPatchHunk}` over `MultiBuffer` excerpts; the buffer review primitive is `buffer_diff::{BufferDiff,BufferDiffSnapshot}` with `DiffOperations::{stage,unstage,restore}`. The cedian review crate (§15–20) is a thin projection over THESE types, not a parallel model. Bonus: `clock::ReplicaId::AGENT` already reserves agent edit identity in the CRDT — provenance (§17) should key off it where possible. *(→ [ADR-0006](decisions/0006-review-baseline-provenance-precedence.md))*

OMP side: unchanged — there is no `WorkspaceBackend` seam inside OMP (§10) and cedian does not fork OMP. OMP's `edit`/`write` write the filesystem; on `tool_execution_end` cedian imports each changed file as one agent transaction. Disk is authoritative for agent writes; a user edit to the same region during the turn surfaces as `STALE`, never a silent overwrite. Headless (pre-S9) form: [ROADMAP](ROADMAP.md) row G. `cedian_apply_edit` remains as a host tool the model may choose, not the main path.

Benefits:

- native undo
- correct dirty state
- correct unsaved state
- multi-buffer support
- editor transactions
- reliable review history

### §13 Unsaved Buffer Strategy

Problem:

```text
user edits auth.rs
does not save
OMP read() reads filesystem
```

The agent sees stale content.

Because OMP reads the filesystem and is not forked ([ADR-0027](decisions/0027-zero-omp-fork.md)), the behavior is (permanent):

```text
Agent Sync = ON
```

Before an agent turn:

```text
dirty buffers
   ↓
save
   ↓
record buffer versions
   ↓
start OMP turn
```

Acceptance: a crash between save and prompt start must lose nothing and must not double-apply on restart (idempotent turn start). An overlay filesystem for unsaved semantics would need OMP to read cedian buffers, which requires an OMP change — out of scope unless upstream offers it.

### §15 Native Review Changes

Review must be first-class.

Do not rely on OMP printing markdown diffs in chat.

**Inline edit** ([ADR-0026](decisions/0026-fast-lane.md)): select code → ⌘K → instruction → one OMP turn targeting the selection (fast `smol` model by default) → diff in place → accept/reject in the editor. Same provenance and review rules as any turn.

Pipeline:

```text
OMP edits
   ↓
task baseline + edit provenance
   ↓
cedian diff model
   ↓
Zed multibuffer
   ↓
Review Changes
```

Actions:

```text
accept hunk
reject hunk

accept file
reject file

next finding
previous finding

Ask cedian
Fix
Explain
Dismiss
```

### §16 Review Must Use Task Baseline, Not Git HEAD

Example:

```diff
user:
+ manual user change
```

Then agent:

```diff
agent:
+ agent change
```

If Review Changes compares against Git HEAD, both changes appear.

Wrong.

At task start, snapshot per-buffer versions (Zed `clock::Global` version vectors, not wall-clock save):

```text
Task starts
   ↓
record baseline = map of path → clock::Global
```

Reuse Zed's diff/undo/multibuffer primitives for the review model — do NOT build a parallel diff engine. The cedian review crate is a thin projection (baseline + provenance → Zed multibuffer), not a reimplementation.

Agent review is:

```text
baseline → current
```

Only OMP-generated changes should be attributed to the task.

### §17 Edit Provenance

> **provenance lives in cedian, survives OMP compaction.** `AgentEdit` store is cedian-owned (keyed by `task_id + tool_call_id`), separate from the OMP transcript. OMP compaction/session-restore must never delete or rewrite it. If OMP emits edits after a compaction that cedian cannot correlate, cedian marks those hunks `UNATTRIBUTED` (visible, reviewable, but excluded from per-task accept-all) rather than misattributing them.
> **`UNATTRIBUTED` = per-hunk accept/reject allowed, accept-all skips.** The user can resolve each unattributed hunk individually; bulk accept-all never sweeps them in. No re-attribute button in V1 (keeps the invariant "only OMP-generated changes attributed to the task", §16).
>
> **Upstream fact (oh-my-pi `docs/compaction.md` + `docs/session.md`, verified 2026-10-06) — what compaction REALLY keeps.** Survives: compaction/branch summaries (`preserveData` replay, `details.readFiles`), TTSR dedup re-injection, todos (latest `user_todo_edit` or todo-tool result), session-exit diagnostics, goal journal (paused until resume). Lost VERBATIM: pre-compaction turns, pre-`reset_boundary` model context, dropped tool pairings, fine attribution inside the summary. CONSEQUENCE: (a) cedian MUST snapshot `tool_call_id → AgentEdit` into its OWN store at `tool_execution_end` time — post-compaction backfill from the transcript is impossible by design; (b) UNATTRIBUTED derivation = scan backward for the latest namespaced cedian record (custom `user`-authored entries only; NEVER synthesize snake_case roles — OMP's taxonomy is camelCase and `remove`/`promote` match user-authored only); (c) `reset_boundary` hides pre-boundary context from the model while full export retains it — cedian review reads the EXPORT view, never the model view. *(→ [ADR-0006](decisions/0006-review-baseline-provenance-precedence.md))*

Track every edit event:

```rust
AgentEdit {
    tool_call_id,
    task_id,
    file,
    before,
    after,
    diff,
    timestamp,
}
```

Use provenance for:

- accurate Review Changes
- conflict detection
- stale hunks
- reviewer findings
- workflow evidence
- task history

### §18 Accept/Reject Semantics

##### Accept

The agent's code is already in the working buffer.

Accept simply marks:

```text
accepted
```

##### Reject

Apply inverse patch:

```text
agent result
 ↓
inverse hunk
 ↓
task baseline state
```

If the user modifies the same hunk after the agent:

```text
STALE
```

Do not automatically reject.

**Revert turn** ([ADR-0026](decisions/0026-fast-lane.md)): one action rejects every hunk a turn produced (grouped by provenance); `STALE` hunks are skipped and listed, never overwritten; the revert is itself undoable.

Show:

```text
This hunk changed after the agent edit.

[Compare]
[Restore Manually]
```

> **Hunk-state precedence.** Three non-`accepted` states, strictly ordered: `INTERRUPTED` (crash left a half-tool-call, §74) > `UNATTRIBUTED` (no correlating `tool_call_id`, §17) > `STALE` (user edited after agent). A hunk shows exactly one badge — the highest present. Transitions allowed only: `INTERRUPTED → UNATTRIBUTED` (reconciled) → `STALE`/`accepted`/`rejected` (user resolved). No silent demotion: `UNATTRIBUTED` never becomes attributed without a re-driven tracked edit. *(→ [ADR-0006](decisions/0006-review-baseline-provenance-precedence.md))*

### §19 Review Feedback to OMP

Allow:

```text
[Ask cedian]
[Fix]
[Explain]
```

And inline comments.

Structured payload:

```text
ReviewFeedback {
  path,
  range,
  diff,
  comment,
  task_id
}
```

OMP receives exact review context rather than loosely pasted text.

### §20 Review Performance

Do not rebuild the diff on every streaming token.

Use:

```text
message token         → no diff update

tool_call partial     → no diff update

edit completed
        ↓
queue review update
        ↓
debounce 50–100ms
        ↓
one diff rebuild
```

Browser frames and tool progress must not trigger editor diff recomputation.

---

## 8. Language services

### §21 LSP Integration

Do not run duplicate LSP instances if Zed already owns one.

Bad:

```text
Zed rust-analyzer
+
OMP rust-analyzer
```

Final target (no backend seam inside OMP — §10):

```text
OMP `lsp` (unchanged, OMP-owned)
    ↓ (reads Zed state via cedian:// URIs + host tools)
Zed Language Server
```

Map operations:

```text
definition
type_definition
references
hover
symbols
rename
code_action
diagnostics
implementation
raw_request
```

The model still sees one tool:

```text
lsp
```

### §22 DAP / Debugger Integration

Use the same debugger session.

Target (no backend seam inside OMP — §10):

```text
OMP debug (unchanged, OMP-owned)
   ↓ (reads/writes Zed state via cedian:// URIs + host tools)
Zed Debugger Session
```

Effects:

- breakpoint set by OMP appears in editor
- step operation updates native debugger UI
- stack frames are shared
- variables are shared
- continue from user or agent affects the same session

No second debugger runtime.

---

## 9. Terminal

### §23 Bash and Terminal

Keep OMP's bash semantics.

Do not replace the harness shell engine merely for UI consistency.

> **Two shells, one policy.** OMP `bash` (headless, sandboxed, §64-gated) and the user's interactive terminal PTY (§24) are DIFFERENT execution contexts sharing one screen. Rules: (a) agent commands never execute in the user's PTY — cedian renders agent output *as if* terminal, backed by the sandboxed exec channel; (b) user-typed commands never inherit agent grants — scopes (`once|task|session`, §64) bind to the agent context only; (c) env allow-list: the agent sees a scrubbed env (no `SSH_AUTH_SOCK`, no credential vars) even when cwd matches the user's terminal. *(→ [ADR-0015](decisions/0015-two-shells-one-policy.md))*

OMP runs:

```text
cargo test
```

cedian renders:

```text
┌─ Terminal ────────────────────┐
│ cargo test                    │
│                               │
│ 182 passed                    │
│                               │
│ ✓ exit 0 · 2.8s               │
│                               │
│ [Open full output]            │
└───────────────────────────────┘
```

### §24 PTY Integration

Agent and user terminals stay separate (§23, [ADR-0015](decisions/0015-two-shells-one-policy.md)):

```text
OMP bash (incl. interactive commands)          user terminal
 ↓ OMP's own PTY / exec channel                 ↓
cedian renders output as a terminal card        Zed terminal PTY (user only)
```

- OMP keeps its own PTY execution (it has PTY support; `--no-pty` exists to turn it off). cedian never swaps OMP's `bash` backend (§10) and never runs agent commands in the user's PTY.
- "Open full output" on a terminal card opens a read-only view of the agent's output, not a shell the user types into.
- Headless (pre-S9): the agent PTY lives inside the `cedian shell` process lifetime ([ADR-0021](decisions/0021-headless-host-process.md)).

---

## 10. Browser

### §25 Browser Architecture

Do not create a second browser engine unnecessarily.

Do not make WKWebView the primary automation browser if it splits state from OMP's CDP/Chromium browser.

Use one shared Chromium.

```text
            Shared Chromium
                  │
          CDP tab/session
          ┌───────┴────────┐
          │                │
        OMP             cedian
  browser automation   Browser View
```

The user and OMP interact with the **same real tab**.

> **user input preempts.** Same-tab sharing is view-sharing, not input-sharing. The moment cedian detects user pointer/keyboard activity in the shared tab, the agent's input stream pauses (event → OMP: `browser_input_preempted`), a banner shows `Agent paused — you took control [Resume agent]`, and OMP must re-request input focus before acting again. Agent never queues synthetic input ahead of live user input. *(→ [ADR-0007](decisions/0007-shared-browser-and-evidence-frames.md))*

### §26 Render Browser in cedian

Use CDP screencast-style rendering:

```text
Chromium
 ↓ frames
CDP
 ↓
cedian BrowserSurface
 ↓
GPUI texture
```

cedian pane:

```text
┌────────────────────────────────┐
│ localhost:3000                 │
├────────────────────────────────┤
│                                │
│      actual browser frame      │
│                                │
└────────────────────────────────┘
```

Mouse:

```text
GPUI click
 ↓
CDP mouse event
 ↓
same Chromium tab
```

Keyboard:

```text
GPUI
 ↓
CDP key event
```

Agent and user share the same state.

### §27 Browser Workflow

User opens:

```text
localhost:3000/login
```

Then asks:

```text
Fix this spacing.
```

cedian provides ambient context:

```text
browser_tab = main
url = localhost:3000/login
viewport = ...
```

OMP can:

```text
observe
 ↓
screenshot
 ↓
inspect source
 ↓
edit
 ↓
reload
 ↓
screenshot
 ↓
verify
```

cedian Browser pane updates continuously.

### §28 Browser Features OMP Should Retain

OMP should continue to access capabilities such as:

```text
navigation
DOM/ARIA inspection
JavaScript
selectors
screenshots
click
type
file upload
console
network
reload
multiple tabs
```

cedian should visualize these, not reimplement agent logic.

### §29 Browser Frame Backpressure

For live screencast:

```text
frame A
frame B
frame C
```

If A is still being rendered, drop B and show C.

Policy:

```text
latest-frame wins
```

This keeps interaction responsive.

> **Upstream fact (oh-my-pi `modes/rpc/rpc-output.ts`, verified 2026-10-06) — RPC backpressure is DISK-SPOOL, not credit-based.** While the reader keeps up, stdout flows direct; otherwise OMP spills to a private temp file and drains 64KiB preserving order — disk can grow UNBOUNDED, spool failure disposes with exit 1. Clients MUST keep reading after stdin close. CONSEQUENCE for cedian: (a) never assume bounded memory — the EventRouter reader thread must drain continuously even when GPUI is busy (backpressure at the GPUI batch layer, §73, never by pausing the read); (b) treat `exit 1` + spool-failure signature as a distinct crash class in §74 recovery (transcript may be truncated — reconcile, don't assume intact); (c) the CDP screencast latest-frame-wins above is a SEPARATE policy at the presentation layer and must not be confused with RPC transport backpressure. *(→ [ADR-0002](decisions/0002-omp-child-process-rpc-v2.md))*

> **Evidence frame binding.** Dropped frames must never become evidence. Every `ScreenshotEvidence`/`BrowserEvidence.screenshot` carries the CDP frame id it was captured from; a gate holding evidence whose frame id is older than the latest rendered frame marks it `stale-frame` and requires re-capture before `required: true` passes. Displayed screenshots render their capture sequence number in the corner — what the user sees is what the gate evaluated. This is the presentation-layer case of the general rule that all evidence is bound to the code state it verified (§53, [ADR-0024](decisions/0024-evidence-bound-to-code-state.md)). *(→ [ADR-0007](decisions/0007-shared-browser-and-evidence-frames.md))*

### §80 Browser + Agent Shared State

The browser should be a first-class task object.

```text
cedian Browser
      ↕
BrowserSessionId
      ↕
OMP
```

OMP can inspect the user's current browser session directly.

Example:

User is on:

```text
localhost:3000/settings
```

Then asks:

```text
Fix this.
```

OMP gets:

```text
browser session
DOM
screenshot
console
URL
viewport
```

without manually attaching a screenshot.

---

## 11. Agent UI

### §30 Images in Agent Composer

Support:

```text
drag image
paste image
drop screenshot
```

Flow:

```text
GPUI attachment
 ↓
OMP RPC prompt.images
```

No need to convert everything into temporary file paths or OCR.

### §31 Rich Thread Content

Thread messages should be structured, not markdown-only.

Support:

```text
Text
Markdown
Code
Diff
Image
Tool Card
Browser Snapshot
Simulator Snapshot
Review Finding
Workflow
Subagent
Artifact
```

This is necessary for a native agentic IDE.

### §41 One Thread = One OMP Session

Do not maintain separate conceptual Zed threads and OMP sessions if not necessary.

Use:

```text
cedianThread
  │
  └── OmpSessionId
```

Task model:

```rust
struct cedianTask {
    id: TaskId,

    omp_session: OmpSessionId,

    workspace: WorkspaceId,

    worktree: WorktreeId,

    baseline: ReviewBaseline,

    workflow: WorkflowProjection,

    browser: Option<BrowserSessionId>,

    simulator: Option<SimulatorSessionId>,
}
```

### §63 Ask Tool

OMP `ask` should render as a native cedian GPUI dialog.

> **Upstream fact (oh-my-pi `docs/approval-mode.md` + RPC Extension UI Sub-Protocol, verified 2026-10-06).** OMP approvals DEFAULT TO YOLO (auto-allow all) — strictness must be CONFIGURED, never assumed. `ask` requires explicit opt-in via `set_ask_dialog(true)` (default off; without it, headless/no-UI approval-needing tools FAIL CLOSED — treat that as normal control flow, not an error). `ask` ≠ approval: `ask` is `extension_ui_request{method:ask, questions[{id,question,options[],multi?,recommended?}], timeout?}` with strictly-ordered answers; approvals run through a separate extension runner. Subagents are headless-yolo inside the parent task boundary (residual prompts REJECT). The runtime MUST set `approvalMode: always-ask|write` + `approval.*` + `bash.patterns` + `set_ask_dialog(true)` at spawn, before any agent turn (ROADMAP S3 gate 1). RPC has no command for these — they are set through the cedian spawn profile (argv + config overlay), [ADR-0020](decisions/0020-omp-spawn-profile.md) — the plan's strict-wins (§64) only has teeth once yolo is off. *(→ [ADR-0012](decisions/0012-permissions-strict-wins-at-cedian-gate.md))*

Example:

```text
Which database?

○ PostgreSQL
● SQLite
○ Other...

[Continue]
```

No terminal prompts.

> **`ask` liveness.** A pending `ask` dialog is a lease, not a lock: if OMP disconnects mid-`ask`, cedian auto-dismisses with decision `abstain` (logged in the audit tuple, §64) and the turn resumes as `blocked` on reconnect — never a ghost dialog awaiting a dead runtime. `ask` timeout (default 5 min, no response) = `abstain` + force-escalate per §54. Strict-wins (R1-C2) applies to the *answer*, not to liveness: cedian may always dismiss a dead dialog. *(→ [ADR-0013](decisions/0013-ask-is-a-lease.md))*

### §65 Agent Modes (Normal / Goal / Plan)

> **Modes are measured, not assumed.** `set_mode` does not exist in `RpcCommand`, and `deep`/`normal` are not OMP modes (`--mode` is a launch transport: `text|json|rpc|acp|rpc-ui`; plan+rpc are mutually exclusive at launch upstream). `goal` IS RPC-controllable (`get/create/resume/pause/drop`, continuation needs `goal.continuationModes ∋ rpc`). Therefore: no `Deep` entry; `Plan`, if wanted, runs as a separate runtime spawned with OMP's launch-time plan options through the spawn profile — no OMP change ([ADR-0027](decisions/0027-zero-omp-fork.md)); a mode switch mid-turn aborts the turn (same path as `abort`) and starts a new turn under the new mode — never mid-stream reinterpretation. *(→ [ADR-0014](decisions/0014-agent-modes.md))*

cedian UI:

```text
[ Agent ▼ ]

Normal
Plan (separate plan-mode runtime, if enabled — else hidden)
Goal (only if continuationModes ∋ rpc)
```

No `Deep` entry: the mode does not exist upstream.

OMP still owns execution behavior.

### §66 Tool Card Registry

Create:

```rust
ToolCardRegistry
```

Consume:

```text
tool.started
tool.update
tool.completed
```

Render native cards by tool type.

### §67 Tool Card Examples

##### Edit

```text
Edited auth.rs

+18 −6

[Open]
[Review]
```

##### Bash

```text
cargo test

✓ 182 passed
2.3s

[Output]
```

##### LSP

```text
References

Session::cancel
14 references

[Open results]
```

##### Debug

```text
Paused

session.rs:182

Thread: main
Reason: breakpoint

[Open Debugger]
```

##### Browser

```text
Browser

localhost:3000/login

✓ clicked Login
✓ redirected /dashboard

[Open Browser]
```

##### iOS

```text
iPhone 18 Pro

✓ Build
✓ Launch
✓ Login flow

[Open Simulator]
```

### §68 Generic Tool Card Fallback

If OMP adds a new tool unknown to cedian:

```text
Tool: some_new_tool

Running...
```

Render generic structured output.

Do not break the UI.

### §69 One Window by Default

Use one application/workspace.

##### Code Mode

```text
Files | Editor | Agent
──────────────────────
Terminal
```

##### Agent Mode

```text
Threads | Agent Task
        | Browser
        | Workflow
```

##### Review Mode

```text
Changes | Diff
        | Findings
```

The workspace morphs between tasks.

### §70 Multi-Window as a Projection

Optional for multi-monitor use:

```text
Window 1
Editor

Window 2
Agent + Browser
```

But both use:

```text
same task
same OMP session
same worktree
same review
same browser
same simulator
```

Never create a separate second-agent application architecture.

### §78 Zed Agent Code

Do not delete everything immediately.

Reuse useful concepts/components:

```text
Agent Panel UI concepts
Thread UI
Review Changes
DiffPatch/MultiBuffer review (acp_thread::diff)
image attachments
composer
notifications
```

But final backend becomes:

```text
Zed native agent backend
    ✕
OMP
    ✓
```

ACP can remain dormant in upstream code if deleting it would increase merge pain.

Do not design cedian around ACP.

---

## 12. Parallel work

### §42 Subagents

OMP owns subagents.

cedian visualizes them:

```text
Main Agent

├── Explore authentication      ✓
├── Inspect regression          ✓
├── Implement fix               ●
└── Security review             ○
```

Clicking a worker can show:

```text
messages
tools
files
worktree
status
```

cedian should not create a second subagent scheduler.

### §43 Worktrees

> **cedian owns worktrees, OMP requests.** Zed Project/worktree is the single owner of worktree lifecycle (`git worktree add/remove`, `WorktreeId`). OMP never creates OS-level worktrees directly — it requests one through the host tool `cedian_worktree_request`; cedian executes, returns `WorktreeId`, and renders it. OMP keeps orchestration/merge *policy*; cedian keeps *mechanism*. `cedianTask.worktree` stores the id, not the ownership.
>
> Worktree *mechanism* (`git worktree add/remove` + `WorktreeId` plumbing + the `cedian_worktree_request` host tool) lands together with the visualization in slice S5 — mechanism without UI is untestable by the hermetic contract (§86), UI without mechanism is a mock. *(→ [ADR-0009](decisions/0009-worktree-mechanism-vs-policy.md))*

Flow:

```text
OMP requests isolated worktree (host tool `cedian_worktree_request`, [ADR-0022](decisions/0022-host-tool-first.md))
 ↓
cedian creates worktree (Zed Project primitive)
 ↓
event { worktree_id } → OMP + cedian view attaches
```

Conflict rule: if the user opens the same file the subagent edits, cedian marks the review hunk STALE (§18) — never auto-merge, never silently overwrite. Merge-back into the main worktree requires explicit user accept.

cedian can expose:

```text
Agent working in isolated branch

[View]
```

### §44 Swarm and Arena

Do not create separate orchestration engines.

##### Swarm

```text
OMP task batch
 ├── worker A
 ├── worker B
 └── worker C
```

Different workers handle different slices.

##### Arena

```text
same problem × N
     ↓
judge
```

Multiple workers solve the same problem, then OMP selects/synthesizes.

### §45 Architect, Interrogate, Blast Radius

Treat these as presets/compositions, not subsystems.

```text
Architect
= design Arena preset

Interrogate
= ReviewPolicy preset

Swarm
= ParallelPolicy split-by-scope preset

Blast Radius
= analysis/review/verification preset
```

Do not create dedicated engines for each idea.

---

## 13. Workflow & verification

### §46 OMP Engineering Workflow Layer

The workflow layer is split at the trust boundary (decided in [ADR-0010](decisions/0010-gate-checker-a1-narrow.md)):

```text
OMP (intelligence)                       cedian (environment + checker)
──────────────────                       ──────────────────────────────
TaskProfile (classify the task)          evidence store (provenance-linked, §53)
Playbook choice                          gate evaluation — pure, no I/O (§54)
Phase progression                        completion block: can_complete (§55)
how to obtain evidence                   gate floor policy (cedian.toml, §64/§77)
workflow events (§61)          ──────▶   workflow UI (§51)
```

Flow:

```text
TaskProfile
    ↓
Playbook
    ↓
Phases
    ↓
Verification Gates   ← cedian evaluates over stored evidence
    ↓
Review
    ↓
Complete             ← cedian refuses while a required gate is unmet
```

Rules:

- OMP decides; cedian never schedules work, spawns agents, or gathers evidence itself (§54, §88 "second workflow engine").
- The gate floor (required gates per task kind × risk) lives in cedian policy beside permissions. OMP may ADD gates; it can never remove or weaken a floor gate.
- OMP reports profile, playbook, phase transitions and evidence claims through host tools (`cedian_workflow_update`, `cedian_complete`) — [ADR-0022](decisions/0022-host-tool-first.md). Evidence counts as attributed only when cedian binds it to a router-log call: the agent names the producing tool (`from_tool`, optional `match`), cedian resolves the newest successful, non-channel matching call and stores its `tool_call_id` — the model never sees ids ([ADR-0031](decisions/0031-evidence-cites-tool-not-call-id.md)).
- Playbooks and the project verification profile are OMP **skills** in `.omp/skills/`, not TypeScript and not cedian code ([ADR-0025](decisions/0025-playbooks-are-omp-skills.md)).
- Until those host tools land, `cedian_workflow`'s profile/playbook/phase code is a headless stand-in (ROADMAP row A) driven from the CLI.

### §47 Workflow Data Model

Suggested:

```ts
interface TaskProfile {
  kind:
    | "investigation"
    | "bug_fix"
    | "feature"
    | "refactor"
    | "performance"
    | "prototype";

  complexity: "trivial" | "small" | "medium" | "large";

  risk: "low" | "medium" | "high";

  surfaces: Surface[];

  constraints: string[];

  acceptanceCriteria: AcceptanceCriterion[];
}
```

Example surfaces:

```ts
type Surface =
  | "library"
  | "cli"
  | "web"
  | "desktop"
  | "ios"
  | "android"
  | "api"
  | "database";
```

### §48 Workflow State

```ts
interface WorkflowState {
  task: TaskProfile;

  playbook: string;

  currentPhase: PhaseId;

  phases: PhaseState[];

  gates: GateState[];

  reviews: ReviewState[];

  parallelRuns: ParallelRunState[];

  status:
    | "planning"
    | "running"
    | "blocked"
    | "ready_for_review"
    | "complete"
    | "failed";
}
```

### §49 Start with a Small Playbook Set

Do not begin with 20+ playbooks. Playbooks are OMP skills ([ADR-0025](decisions/0025-playbooks-are-omp-skills.md)); each tells the agent when to call `cedian_workflow_update` / `cedian_complete`.

V1:

```text
Investigation
Bug Fix
Feature
Refactor
Generic
```

Example Bug Fix:

```ts
const bugFix: Playbook = {
  id: "bug_fix",

  phases: [
    { id: "reproduce", required: true },
    { id: "investigate", required: true },
    { id: "implement", required: true },
    { id: "verify", required: true },
    {
      id: "review",
      when: ctx => ctx.task.risk !== "low",
    },
  ],
};
```

### §50 Phase State

Every phase supports:

```text
pending
running
passed
failed
blocked
skipped
```

Skipped phases include a reason:

```text
Architecture
Skipped: single-file local change with no API boundary changes
```

This prevents silent workflow ambiguity.

### §51 Workflow UI

cedian renders:

```text
Fix login redirect

Bug Fix · Medium risk

✓ Reproduce
✓ Investigate
✓ Implement
● Verify
○ Review
```

OMP does not need to narrate every transition in prose.

### §52 Verification Gates

Gate types:

```ts
interface Gate {
  id: string;

  kind:
    | "build"
    | "test"
    | "lint"
    | "reproduction"
    | "behavior"
    | "visual"
    | "performance"
    | "review"
    | "custom";

  required: boolean;

  predicate: GatePredicate;
}
```

Results:

```ts
interface GateResult {
  // evidence counted here is fresh, attributed, and outcome = "pass" (§53)
  status:
    | "pending"
    | "passed"
    | "failed"
    | "blocked"
    | "skipped";

  evidence: Evidence[];

  reason?: string;
}
```

### §53 Evidence as First-Class Data

Use:

```ts
type Evidence =
  | CommandEvidence
  | TestEvidence
  | BrowserEvidence
  | ScreenshotEvidence
  | SimulatorEvidence
  | DebuggerEvidence
  | FileEvidence;
```

> **Evidence carries provenance.** Every evidence variant carries `provenance: { tool_call_id: string } | { unattributed: true }`. Evidence is attributed only when its `tool_call_id` appears in the router log with a successful `ToolEnd` ([ADR-0022](decisions/0022-host-tool-first.md)); file evidence additionally links to the §17 `AgentEdit` store. A `required: true` gate REJECTS `unattributed` evidence — the agent must re-produce the result through a tracked tool call before the gate passes. Optional gates may accept unattributed evidence but must surface it as `unverified-origin` in the UI. *(→ [ADR-0010](decisions/0010-gate-checker-a1-narrow.md))*

Every evidence item also carries ([ADR-0024](decisions/0024-evidence-bound-to-code-state.md)):

- `code_state` — the buffer versions (or worktree tree fingerprint) it verified. Any later change to a bound file makes it `stale`; required gates never count stale evidence.
- `outcome ∈ pass | fail | inconclusive` — "could not run" is `inconclusive`, never pass and never silently fail.
- Measurements (performance) carry `{runs, median, range, limiter, build_profile}`; any missing field ⇒ `inconclusive`.
- Gates may name a feature-map id from the project verification profile (`verify-<app>` OMP skill, [ADR-0025](decisions/0025-playbooks-are-omp-skills.md)).

Example browser evidence:

```ts
interface BrowserEvidence {
  type: "browser";

  url: string;

  actionPath: string[];

  expected: string;

  observed: string;

  screenshot?: ArtifactRef;
}
```

### §54 Gate Engine Must Not Become a Second Agent

Wrong:

```text
gate.runTests()
gate.openBrowser()
gate.spawnAgent()
```

Correct:

```text
Gate Engine
  ↓
What evidence is missing?
  ↓
OMP agent decides how to obtain it
```

The workflow layer defines requirements.

OMP's normal agent/tool loop satisfies them.

> **Gate is a pure function, not a scheduler.** The gate engine NEVER performs I/O: no spawn, no browser, no test run, no RPC. It evaluates `GatePredicate` over the already-stored evidence map with a fixed evaluation budget (bounded time, bounded input size). Predicates requiring fresh I/O are rejected at registration — the agent must produce the evidence first, the gate only reads it. This keeps §88 "no second workflow engine" true.
>
> **Anti-loop:** `OMP continues` (§55) is bounded by `max_continue` (default 3) per gate. Each continue must attach at least one NEW evidence item referencing the missing gate, or the counter still advances. On exhaustion: `status = blocked` + force-escalate to the user with `{gate_id, missing_evidence, attempts}` — never infinite loop, never silent success. *(→ [ADR-0010](decisions/0010-gate-checker-a1-narrow.md))*

### §55 Completion Gate

Before OMP says:

```text
Done.
```

It must pass — the agent calls host tool `cedian_complete` ([ADR-0022](decisions/0022-host-tool-first.md)); a turn that never passes it leaves the workflow not complete (evaluated by cedian at the completion boundary, [ADR-0010](decisions/0010-gate-checker-a1-narrow.md)):

```ts
workflow.canComplete()
```

`cedian_complete` takes a **claims ledger**: each claim labelled `measured | inferred | guess` with evidence ids; a `measured` claim needs fresh attributed `pass` evidence, and the completion view shows every claim with its label and evidence — unbacked claims are flagged, never hidden ([ADR-0024](decisions/0024-evidence-bound-to-code-state.md)).

If required evidence is missing:

```text
false

reason:
required gate "browser_behavior" has no evidence
```

OMP continues (bounded by `max_continue`, see §54).

If it cannot complete:

```text
status = blocked
```

Do not pretend success.

### §56 Example Gate Policies

##### Bug Fix

```text
required:
✓ failing reproduction captured
✓ implementation complete
✓ original reproduction passes
✓ focused tests pass

conditional:
○ nearby regression test
○ live behavior
○ review
```

##### Feature

```text
required:
✓ build
✓ relevant tests
✓ acceptance criteria

conditional:
○ live app verification
○ visual verification
○ performance
○ review
```

##### Performance

```text
required:
✓ baseline measurement (runs, median, range, limiter, build profile)
✓ after measurement, same fields, same build profile
```

##### Refactor

```text
✓ behavior baseline captured
✓ implementation
✓ tests
✓ behavior after == before
```

### §61 Workflow Events

Add structured events such as:

```text
workflow_started
workflow_phase_started
workflow_phase_completed
workflow_phase_skipped

workflow_gate_updated
workflow_evidence_added

review_started
review_finding
review_completed

workflow_completed
workflow_blocked
```

cedian generates these events itself from host-tool calls (`cedian_workflow_update`, `cedian_complete`, `cedian_review_finding`) and from gate evaluations ([ADR-0022](decisions/0022-host-tool-first.md)); OMP does not need to emit them. cedian renders them directly.

### §62 Task Complexity Budget

Prevent over-engineering.

**Proportional rigor (fast lane, [ADR-0026](decisions/0026-fast-lane.md)).** A normal turn has no workflow and no gates — edits land as reviewable hunks immediately, as light as any agentic IDE. Workflow and gates engage only when the `cedian.toml` gate floor requires them for the task kind × risk, or when the user asks (`/verify`, a playbook). Trivial and small tasks never pass through gates unless asked.

Suggested:

```ts
interface WorkflowBudget {
  maxReviewers: number;
  maxParallelWorkers: number;
  allowArena: boolean;
  requireLiveVerification: boolean;
}
```

##### Trivial

```text
0 parallel workers
0 reviewers
no architecture
basic verification
```

##### Small

```text
0–1 reviewer
no arena
focused tests
```

##### Medium

```text
up to 2 reviewers
swarm if independent
live verification if user-facing
```

##### Large / High Risk

```text
architecture arena
parallel exploration
multiple reviewers
live verification mandatory
```

This avoids turning:

```text
rename this button
```

into an orchestration circus.

---

## 14. Review agents

### §57 Review Roles

Start with:

```text
Correctness
Regression
Architecture
Security
```

Do not use every reviewer on every task.

Example policy:

```text
small UI copy change
→ no review

normal feature
→ correctness

auth feature
→ correctness + security

major core refactor
→ correctness + regression + architecture
```

### §58 Reviewers Must Be Read-Only

> **Reviewer sandbox profile.** "Generally receive" is not enforcement. Reviewer subagents run under a dedicated OS-enforced profile: `edit`, `write`, and `computer`-actuate are `Deny` at the sandbox layer (same Seatbelt mechanism as §64 mechanism 2), not merely absent from the tool list. `bash` for reviewers is allow-listed to read-only prefixes (`git diff`, `git status`, test-status queries) via execpolicy-style rules (§64 lesson 1) — a reviewer invoking anything outside the list gets `Deny`, not an `Ask`. A reviewer that needs a write to prove a point must file a finding; it cannot self-escalate. Reviewers run with a fresh context and, where OMP routing allows, a different model from the implementer; a finding is dismissed only with a recorded reason (audit log). The reviewer `bash` allow-list lives in the same policy file as §64 (same CI policy gate, §64) — one owner, no drift. *(→ [ADR-0011](decisions/0011-reviewers-read-only-sandbox.md))*

Review workers receive:

```text
read
grep
glob
LSP query
git diff
read-only bash (allow-listed prefixes only)
```

Not:

```text
edit
write
computer actuation
bash outside the reviewer allow-list
```

Reviewers should identify issues, not silently rewrite the implementation.

### §59 Review Finding Schema

```ts
interface ReviewFinding {
  id: string;

  category:
    | "correctness"
    | "regression"
    | "architecture"
    | "security";

  severity:
    | "blocker"
    | "high"
    | "medium"
    | "low";

  confidence: number;

  file?: string;

  line?: number;

  title: string;

  explanation: string;

  evidence?: string[];

  suggestedFix?: string;
}
```

cedian annotates the editor/review diff.

### §60 Review Aggregation

Use OMP parent agent.

```text
Reviewer A ─┐
Reviewer B ─┼──> OMP parent
Reviewer C ─┘
              ↓
       dedupe / prioritize
```

No new review orchestration engine.

---

## 15. Safety: permissions & computer use

### §64 Permissions

Because cedian is personal, use a simple policy.

> **strict-wins.** OMP `ask`/tool policy and the cedian policy TOML are evaluated independently; the stricter verdict wins (`Deny > Ask > Allow`). cedian policy can never be loosened by an OMP decision, and OMP can never be forced to act by a cedian `Allow`. Every `ask` renders as a native GPUI dialog (§63) with the requesting side labeled.
>
> **Upstream fact (oh-my-pi `docs/approval-mode.md` resolver, verified 2026-10-06) — strict-wins is NOT how OMP resolves.** OMP's real order: tool-`Deny` absolute → user-`Deny` absolute → yolo-mode: explicit tool allow/prompt wins, else user policy, else allow (bare `override` ignored) → non-yolo: `override:true` allows ONLY with tool-allow else prompt; then tool→user→mode-tier. Provider `pendingSafetyChecks` force-prompt EVEN UNDER YOLO; no-UI prompt-needing tools FAIL CLOSED. CONSEQUENCE: cedian strict-wins applies at the CEDIAN GATE (host-tool dispatch + sandbox), never inside OMP's resolver — cedian cannot reorder OMP's pipeline, only refuse at its own boundary. So: (a) the runtime MUST set (at spawn, via the spawn profile — [ADR-0020](decisions/0020-omp-spawn-profile.md)) `approvalMode: always-ask|write` + `approval.*` + `bash.patterns` + `eval`-gate + `set_ask_dialog(true)` to make OMP's resolver strict in the first place; (b) cedian's gate re-evaluates every granted action independently and can still `Deny` what OMP allowed; (c) fail-closed (no-UI, safety-check) is NORMAL CONTROL FLOW the UI renders, not an error path. *(→ [ADR-0012](decisions/0012-permissions-strict-wins-at-cedian-gate.md))*

**Policy source (per project).** By default the spawn profile makes OMP's resolver strict and keeps `computer` off (`policy = "cedian"`, above). A project listed in the user's `cedian.toml` with `policy = "omp"` gets the user's OMP config instead, yolo and `computer` included: no `--approval-mode`, no approval or `computer` keys in the overlay. In both modes:

- strict-wins at the cedian gate (host-tool dispatch, protected metadata paths, Seatbelt) is unchanged
- OMP prompts still render as native dialogs
- every tool execution is audited; in opt-in mode with `decision_source: omp`
- the agent panel shows a persistent badge, and auto-approved tool cards say "approved by OMP"
- reviewers (§58) and automations (S7) always use the default profile
- a repository can never opt itself in: the key lives only in the user's `cedian.toml` (§77)

*(→ [ADR-0035](decisions/0035-omp-native-approval-opt-in.md))*

##### Safe

```text
read
search
LSP
diagnostics
browser inspection
simulator inspection
```

Allow automatically.

##### Project write

```text
edit
create file
git normal operations
tests
builds
browser localhost interactions
simulator interactions
```

Allow automatically if desired.

##### Dangerous

```text
sudo
delete outside workspace
credential access
destructive git
external side effects
```

Ask.

Example:

```toml
[permissions]
safe = "allow"
project_write = "allow"
dangerous = "ask"
```

How the pieces combine: the tiers above are the DEFAULT verdict per action class; prefix/network rules (below) refine individual commands; OMP's own resolver runs first (configured strict, [ADR-0020](decisions/0020-omp-spawn-profile.md)); the effective verdict is the strictest of all of them (`Deny > Ask > Allow`).

**Eval gate.** `eval` runs Python/JS and is as powerful as `bash`, but `bash.patterns` do not cover it — so `eval` is never auto-approved: `Ask` interactively, `Deny` for reviewers and automations ([ADR-0020](decisions/0020-omp-spawn-profile.md)).

**CI policy gate.** `cedian.toml` is validated in CI: it must parse, every rule's `match`/`not_match` examples must hold (mechanism 1), and the reviewer allow-list must contain only read-only prefixes. The settings UI edits the same file, so there is one source of truth.

Mechanisms adopted from Codex (`openai/codex`); research and exact upstream semantics in the ADR:

1. **Three-valued decisions.** Rules are `prefix_rule(pattern, decision?, justification?, match?, not_match?)`, `network_rule(host, protocol, decision, justification?)`, `host_executable(name, paths)`. Canonical spelling `Allow | Ask | Deny`. Each rule's `match`/`not_match` examples are validated per rule (violations report file + line/col). Effective decision = the STRICTEST of all matching rules; no match = decision omitted (never an implicit allow).
2. **OS-enforced sandbox under the policy.** Every filesystem/network/process verdict maps to an OS mechanism — macOS Seatbelt first (deny-default base, generated read/write roots, unlink-deny anchors, network fragment), never TOML-only intent. Hardening: canonicalize-then-compare paths, write exclusions deny BOTH literal and subpath, unparseable proxy ⇒ empty network policy (fail closed), execute ONLY `/usr/bin/sandbox-exec`. The CUA driver contract is the enforcement floor for `computer` (§38).
3. **Protected metadata paths.** Agent writes to `.git`, `.codex`, `.agents`, `.aws`, `.cedian/` and the OMP session dir are `Deny` even under writable roots — never `Ask`.
4. **Audit tuple.** Every cedian-gate decision appends `{tool, command/prefix, decision, scope (once|task|session), timestamp}` inside a `{timestamp, ordinal, item}` JSONL envelope. `decision ∈ Allow | Ask | Deny | Abstain`; `Abstain` is system-generated only (lease expiry / dead runtime, §63): deny-for-execution, blocked-for-gates (§54), never writable in policy. *(→ [ADR-0012](decisions/0012-permissions-strict-wins-at-cedian-gate.md))*

### §38 Keep OMP Computer Tool

OMP's desktop automation remains useful for:

```text
Xcode
Finder
System Settings
native macOS apps
other desktop UI
```

Use `computer` as a fallback.

Use explicit `ios` for iOS because it is more deterministic and structured.

> "Desktop automation" here means the CUA `cua-driver` contract ONLY (§9, AGENTS.md) — never raw AX calls outside the driver. The driver + macOS Seatbelt profile + bypass-proof test land atomically; until then `computer` stays disabled by default, including as a fallback — enforced by `computer.enabled: false` in the default spawn overlay ([ADR-0020](decisions/0020-omp-spawn-profile.md)). A project with `policy = "omp"` hands this to the user's OMP config instead, together with the approval mode ([ADR-0035](decisions/0035-omp-native-approval-opt-in.md)). *(→ [ADR-0008](decisions/0008-computer-tool-cua-driver-only.md))*

---

## 16. iOS (extension track)

### §32 iOS as a First-Class OMP Tool

cedian registers a host tool named `ios` via `set_host_tools` (no OMP change, [ADR-0027](decisions/0027-zero-omp-fork.md)):

```text
ios
```

Example operations:

```text
devices
boot
shutdown
build
install
launch
terminate
screenshot
ui_tree
tap
type
swipe
logs
open_url
test
```

The model sees one coherent iOS tool.

### §33 iOS Execution Lives in cedian

OMP should not directly own all Xcode/macOS UI implementation details.

Use:

```text
ios host tool (called by OMP)
    ↓
cedian iOS Host Service
    ↓
┌─────────────┬───────────────┐
│ xcodebuild  │ xcrun simctl  │
│ WDA/XCTest  │ log stream    │
└─────────────┴───────────────┘
```

### §34 iOS Device Lifecycle

cedian iOS service handles:

```text
xcrun simctl list
boot
shutdown
install
launch
terminate
erase
openurl
get_app_container
screenshot
```

Project config:

```yaml
ios:
  scheme: MyApp
  configuration: Debug
  device: iPhone 18 Pro
```

### §35 iOS Build Tool

OMP:

```text
ios.build
```

cedian:

```text
xcodebuild
 -scheme MyApp
 -sdk iphonesimulator
 -destination ...
```

Stream progress to the OMP tool event channel.

cedian card:

```text
Build

MyApp · iPhone 18 Pro

✓ Build succeeded
14.8s
```

### §36 iOS Semantic UI Automation

Do not rely mainly on pixel coordinates.

Prefer semantic automation:

```text
WebDriverAgent / XCTest UI
```

OMP should be able to inspect:

```text
Button
  label: Continue

TextField
  label: Email

Button
  label: Login
```

Then call:

```text
ios.tap(ref)
ios.type(ref, "...")
```

Pixel coordinates remain a fallback.

### §37 iOS Visual Panel

Step 1:

```text
simctl screenshot
```

Refresh on action.

Step 2:

```text
Simulator.app
 ↓
ScreenCaptureKit
 ↓
cedian GPUI texture
```

Result:

```text
┌──────── iPhone 18 Pro ────────┐
│                               │
│                               │
│       live simulator          │
│                               │
│                               │
└───────────────────────────────┘
```

User and OMP operate on the same simulator session.

### §81 iOS + Agent Shared State

Same concept:

```text
SimulatorSessionId
```

OMP can inspect:

```text
device
bundle
screen
accessibility tree
logs
build status
```

The user can say:

```text
Fix the button hidden under the keyboard.
```

OMP uses the currently active simulator.

---

## 17. Testing

### §86 Test Strategy

Use at least five major test layers.

> **Harness contract (anti-flake rule):** agent E2E must be hermetic. Provide a fake `omp --mode rpc-ui` that replays recorded RPC frames, plus recorded session fixtures. Layers 1–2 run fully hermetic (no model calls, ever). Layers 3–5 default hermetic; any live-model case is explicitly tagged `live-model`, runs in a separate nightly lane, and never gates merge. No test may depend on wall-clock timing looser than the debounce windows in §§20/73.

##### RPC Contract Tests

```text
spawn
ready
protocol negotiation
chunking
event order
cancel
restart
large output
unknown events
session restore
```

##### Editor Integration Tests

```text
edit closed file
edit open file
edit dirty file
undo
reject hunk
accept hunk
user edits same hunk
delete file
rename file
large file
multi-file change
```

##### Agent Lifecycle Tests

```text
prompt
steer
follow-up
abort
session restore
compaction
subagent
background job
runtime crash
runtime reconnect
```

##### Browser Tests

```text
open
navigate
DOM
click
type
reload
screenshot
console
network
same-tab user input
agent input
rapid frames
tab switching
browser restart
```

##### iOS Tests

```text
boot
build
install
launch
UI tree
tap
type
swipe
screenshot
logs
app crash
simulator restart
device unavailable
build failure
```

---

## A. Appendix: worked examples

### §82 Workflow Verification Example — Web Bug

```text
User:
Fix login redirect bug

OMP
 ↓
TaskProfile = BugFix
 ↓
Reproduce in Browser
 ↓
Capture baseline
 ↓
Inspect source
 ↓
Edit
 ↓
Run tests
 ↓
Reload same Browser tab
 ↓
Verify expected redirect
 ↓
Capture evidence
 ↓
Review
 ↓
Ready for review
```

cedian renders the whole process natively.

### §83 Workflow Verification Example — iOS Bug

```text
User:
Button overlaps keyboard

OMP
 ↓
Boot simulator
 ↓
Build
 ↓
Launch
 ↓
Navigate to screen
 ↓
Capture screenshot + UI tree
 ↓
Inspect SwiftUI code
 ↓
Edit
 ↓
Rebuild
 ↓
Navigate same flow
 ↓
Verify
 ↓
Capture new screenshot
 ↓
Review
 ↓
Ready
```
