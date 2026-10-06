# cedian — Fully OMP-Native Agentic IDE
## Master Architecture & Fully Detailed Implementation Plan

> **Goal:** Turn a minimal Zed fork into a seamless, solid, fully native agentic IDE named **cedian**, powered by **one and only one harness: OMP (Oh My Pi)**.
>
> cedian is a personal IDE. It does **not** need to support other agent harnesses, ACP agents, generic third-party runtimes, or a public plugin marketplace for agents.
>
> The product principle is:
>
> **cedian = environment. OMP = intelligence.**
>
> The user should experience only one product: **cedian**.

---

# 1. Final Product Vision

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

---

# 2. Core Architecture Rule

Use this ownership model everywhere:

## OMP owns

```text
agent loop
model calls
tool decisions
context/token budget
subagents
workflow
verification
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

## cedian owns

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

---

# 3. Process Architecture

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

---

# 4. Transport: Use OMP RPC v2

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

---

# 5. Use the Generated Rust OMP RPC Client

cedian should not manually parse OMP wire messages if a generated client exists.

> **Upstream fact (oh-my-pi `sdk/rust/omp-rpc`, verified 2026-10-06).** The real crate exposes a BLOCKING client (`Client::spawn`/`prompt_and_wait` on threads, v2 auto-negotiation, host tools/URIs dispatched on handler threads, SIGTERM→1s→SIGKILL teardown), types generated from `wire/rpc-wire.schema.json` via `bun run gen:rpc`. There is no `async omp_rpc::Client` — cedian MUST bridge blocking handler threads to GPUI async (never the UI thread). Staleness is CI-enforced upstream (`bun check`, `test/rpc-wire`); cedian pins the schema revision alongside `vendor/omp-revision` and re-runs generation on bump.

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

---

# 6. Pin OMP to cedian

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

> **Scrutinize R4 fix (bundled-binary trust).** The bundled `omp` is a silent privilege boundary: it inherits the user's uid + Seatbelt profile at spawn. Therefore: (a) `script/build-omp` records `{commit, normalized-source-tree hash, builder identity}` into `vendor/omp-revision` — the handshake verifies this triple, not just a version string; (b) at first run after an OMP revision change, cedian shows `OMP runtime updated <old→new> [Review changes] [Continue]` — protocol mismatch AND revision change both fail closed, never silently fall back.

If there is a mismatch:

```text
OMP runtime version mismatch
```

Fail clearly rather than silently falling back.

---

# 7. Zed Fork Strategy

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

Create cedian-owned crates instead of scattering custom logic everywhere.

Suggested layout:

```text
crates/

  cedian_omp/
    runtime.rs
    rpc.rs
    event_router.rs
    session.rs
    context.rs
    permissions.rs
    host_services.rs

  cedian_agent/
    task.rs
    thread.rs
    state.rs

  cedian_agent_ui/
    panel.rs
    composer.rs
    message.rs
    tool_card.rs
    workflow.rs
    subagents.rs

  cedian_review/
    baseline.rs
    tracker.rs
    diff.rs
    provenance.rs
    findings.rs

  cedian_browser/
    session.rs
    cdp.rs
    screencast.rs
    input.rs
    view.rs

  cedian_ios/
    simulator.rs
    xcodebuild.rs
    simctl.rs
    wda.rs
    capture.rs
    logs.rs
    view.rs
```

This minimizes upstream merge pain.

---

# 8. OMP-Side Layout

Avoid rewriting the OMP core.

Suggested additions:

```text
packages/coding-agent/src/

  integrations/
    cedian/
      host-services.ts
      context.ts
      events.ts
      workspace.ts
      lsp.ts
      debug.ts

  workflow/
    profile.ts
    playbook.ts
    gate.ts
    evidence.ts
    review.ts
    parallel.ts

  tools/
    ios.ts
```

Only add browser/cedian-specific support where actually needed.

Avoid modifying the agent loop unless required.

---

# 9. Tool Coverage

Target: OMP should retain access to all useful existing OMP tools while gaining direct IDE-native integration where cedian should be the source of truth.

| Capability | Owner | Integration |
|---|---|---|
| `read` | OMP | OMP tool + cedian workspace awareness |
| `edit` | OMP | OMP semantics → cedian editor transaction |
| `write` | OMP | OMP semantics → cedian project/buffer |
| `grep/glob/find` | OMP | existing OMP |
| `ast_grep/ast_edit` | OMP | existing OMP |
| `bash` | OMP | existing OMP + cedian tool card |
| `eval` | OMP | existing OMP |
| `lsp` | OMP API / cedian backend | use Zed LSP instance |
| `debug` | OMP API / cedian backend | use Zed DAP session |
| `task` | OMP | native OMP subagents |
| `hub/wait` | OMP | native |
| `todo` | OMP | projected into Workflow UI |
| `ask` | OMP | native GPUI dialog |
| `browser` | OMP | shared Chromium/CDP + cedian browser surface |
| `computer` | OMP | CUA-driver backend (`trycua/cua` `cua-driver` Rust crates, MIT) behind OMP tool semantics — default-deny, per-action `ask`, audit-logged; never raw OS input outside the driver contract. **Grill R2 (6a): driver-only first** — `cua-driver` (inspect + operate via typed contract) now; Lume/Spaces VM sandbox is a later phase, not Phase 0. **Pin story (7a):** pin `cua-driver` by git rev + cargo vendor, update on a fixed cadence alongside `vendor/omp-revision` — never float on latest. |
| Git/GitHub | OMP + cedian | OMP execution, cedian visualization |
| Images | OMP | native RPC image content |
| Review | OMP + cedian | OMP reasoning, cedian UI |
| iOS | new native OMP tool | cedian iOS host service |
| Workflow | new OMP-native layer | cedian renders it |
| Verification | new OMP-native layer | cedian renders evidence |
| Arena/Swarm | OMP | presets over existing `task` |

---

# 10. Do Not Duplicate Tool Names

> **Upstream fact (oh-my-pi has NO WorkspaceBackend/LspHost/DapHost seam, verified 2026-10-06).** OMP owns its `edit`/`lsp`/execution tools end-to-end; there is no backend interface to swap inside OMP. The integration surface is OUTSIDE OMP's tools: `set_host_tools` (cedian ops registered as host tools, mountable as `xd://`), `set_host_uri_schemes` (`cedian://` virtual files), and extensions/MCP. So "backend swap" below does NOT mean patching OMP's EditTool — it means (a) OMP-side additions under `packages/coding-agent/src/` (§8) route cedian-relevant calls out through host tools/URIs, and (b) cedian implements the host side. LSP/DAP stay OMP-owned tools reading Zed state via `cedian://` URIs + host tools, not injected backends.

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
OMP EditTool (unchanged, OMP-owned)
 ↓ (routes cedian-relevant paths out via host tool / cedian:// URI)
cedian host implementation (buffer transaction, undo, review)
```

Keep OMP tool semantics stable; cedian implements the host side of the boundary.

---
# 11. Introduce Internal Host Services

> **Upstream fact (concrete RPC ops, oh-my-pi `modes/rpc/host-tools.ts` + `host-uris.ts`, verified 2026-10-06).** `set_host_tools` REPLACES the whole set (survivor enabled-state kept; names must be unique, no clash); out: `host_tool_call{toolCallId,toolName,arguments}` + `host_tool_cancel`, in: `host_tool_update`/`host_tool_result{isError}`. `set_host_uri_schemes` likewise replaces (lowercased scheme names; `writable`/`immutable` default false); out: `host_uri_request{read|write,url,content?}`, in: `host_uri_result{content,contentType text/plain|markdown|json,notes,immutable,isError}`. Built-ins `local:// skill:// artifact:// security:// mcp://` are RESERVED — `cedian://` is free. Note: OMP `edit` does NOT target host URIs (use host tools for writes).

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

cedian announces capabilities (via `set_host_tools` + `set_host_uri_schemes`):

```json
{
  "type": "set_host_services",
  "services": [
    "workspace",
    "lsp",
    "dap",
    "browser_surface",
    "ios"
  ]
}
```

The OMP tool implementations then use those backends internally.

---

# 12. Editor and Buffer Integration

Final target:

```text
OMP edit
   ↓
cedian editor buffer transaction
   ↓
Zed undo stack
   ↓
Review Changes
```

Not merely:

```text
OMP writes filesystem
Zed notices file changed
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

> **Upstream fact (zed `crates/text`, `crates/clock`, `crates/buffer_diff`, `crates/acp_thread/src/diff.rs`, verified 2026-10-06).** No `BufferVersion` type exists — versions are `clock::Global` on `text::BufferSnapshot.version`; undo/txn API is `History::{start,end,push,group}` + `Transaction{id: Lamport, edit_ids, start: Global}`; there is no `AgentDiff` symbol — agent review is `acp_thread::diff::{DiffPatch,DiffPatchFile,DiffPatchHunk}` over `MultiBuffer` excerpts; the buffer review primitive is `buffer_diff::{BufferDiff,BufferDiffSnapshot}` with `DiffOperations::{stage,unstage,restore}`. The cedian review crate (§15–20) is a thin projection over THESE types, not a parallel model. Bonus: `clock::ReplicaId::AGENT` already reserves agent edit identity in the CRDT — provenance (§17) should key off it where possible.

OMP side:

```ts
interface WorkspaceBackend {
    readText(path: string): Promise<...>;
    applyEdits(...): Promise<...>;
    writeText(...): Promise<...>;
}
```

Benefits:

- native undo
- correct dirty state
- correct unsaved state
- multi-buffer support
- editor transactions
- reliable review history

---

# 13. Unsaved Buffer Strategy

Problem:

```text
user edits auth.rs
does not save
OMP read() reads filesystem
```

The agent sees stale content.

For a personal IDE, the simplest reliable V1 behavior is (V1 shortcut with an expiry — replace with overlay-filesystem unsaved semantics once Review Baseline on `clock::Global` (§16) is solid):

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

Acceptance: a crash between save and prompt start must lose nothing and must not double-apply on restart (idempotent turn start). This shortcut contradicts §12's "correct unsaved state" goal — track it as tech debt with an owner, do not let it become permanent.

Later, if desired, add an overlay filesystem for unsaved semantics.

Do not start with that complexity.

---

# 14. Keep OMP Read Capabilities

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
cedian workspace backend if required
```

---

# 15. Native Review Changes

Review must be first-class.

Do not rely on OMP printing markdown diffs in chat.

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

---

# 16. Review Must Use Task Baseline, Not Git HEAD

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

---

# 17. Edit Provenance

> **Grill decision R1 (C4): provenance lives in cedian, survives OMP compaction.** `AgentEdit` store is cedian-owned (keyed by `task_id + tool_call_id`), separate from the OMP transcript. OMP compaction/session-restore must never delete or rewrite it. If OMP emits edits after a compaction that cedian cannot correlate, cedian marks those hunks `UNATTRIBUTED` (visible, reviewable, but excluded from per-task accept-all) rather than misattributing them.
> **Grill R2 (8a): `UNATTRIBUTED` = per-hunk accept/reject allowed, accept-all skips.** The user can resolve each unattributed hunk individually; bulk accept-all never sweeps them in. No re-attribute button in V1 (keeps the invariant "only OMP-generated changes attributed to the task", §16).
>
> **Upstream fact (oh-my-pi `docs/compaction.md` + `docs/session.md`, verified 2026-10-06) — what compaction REALLY keeps.** Survives: compaction/branch summaries (`preserveData` replay, `details.readFiles`), TTSR dedup re-injection, todos (latest `user_todo_edit` or todo-tool result), session-exit diagnostics, goal journal (paused until resume). Lost VERBATIM: pre-compaction turns, pre-`reset_boundary` model context, dropped tool pairings, fine attribution inside the summary. CONSEQUENCE: (a) cedian MUST snapshot `tool_call_id → AgentEdit` into its OWN store at `tool_execution_end` time — post-compaction backfill from the transcript is impossible by design; (b) UNATTRIBUTED derivation = scan backward for the latest namespaced cedian record (custom `user`-authored entries only; NEVER synthesize snake_case roles — OMP's taxonomy is camelCase and `remove`/`promote` match user-authored only); (c) `reset_boundary` hides pre-boundary context from the model while full export retains it — cedian review reads the EXPORT view, never the model view.

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

---

# 18. Accept/Reject Semantics

## Accept

The agent's code is already in the working buffer.

Accept simply marks:

```text
accepted
```

## Reject

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

Show:

```text
This hunk changed after the agent edit.

[Compare]
[Restore Manually]
```

> **Scrutinize R3 fix (hunk-state precedence).** Three non-`accepted` states, strictly ordered: `INTERRUPTED` (crash left a half-tool-call, §74) > `UNATTRIBUTED` (no correlating `tool_call_id`, §17) > `STALE` (user edited after agent). A hunk shows exactly one badge — the highest present. Transitions allowed only: `INTERRUPTED → UNATTRIBUTED` (reconciled) → `STALE`/`accepted`/`rejected` (user resolved). No silent demotion: `UNATTRIBUTED` never becomes attributed without a re-driven tracked edit.

---

# 19. Review Feedback to OMP

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

---

# 20. Review Performance

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

# 21. LSP Integration

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

---

# 22. DAP / Debugger Integration

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

# 23. Bash and Terminal

Keep OMP's bash semantics.

Do not replace the harness shell engine merely for UI consistency.

> **Scrutinize R4 fix (two shells, one policy).** OMP `bash` (headless, sandboxed, §64-gated) and the user's interactive terminal PTY (§24) are DIFFERENT execution contexts sharing one screen. Rules: (a) agent commands never execute in the user's PTY — cedian renders agent output *as if* terminal, backed by the sandboxed exec channel; (b) user-typed commands never inherit agent grants — scopes (`once|task|session`, §64) bind to the agent context only; (c) env allow-list: the agent sees a scrubbed env (no `SSH_AUTH_SOCK`, no credential vars) even when cwd matches the user's terminal.

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

---

# 24. PTY Integration

If OMP RPC mode disables PTY behavior by default, add a cedian PTY backend.

Target:

```text
OMP BashTool
 ↓
cedian PTY service
 ↓
Zed terminal PTY
```

This gives interactive commands a real native terminal session.

---

# 25. Browser Architecture

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

---

# 26. Render Browser in cedian

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

---

# 27. Browser Workflow

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

---

# 28. Browser Features OMP Should Retain

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

---

# 29. Browser Frame Backpressure

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

> **Upstream fact (oh-my-pi `modes/rpc/rpc-output.ts`, verified 2026-10-06) — RPC backpressure is DISK-SPOOL, not credit-based.** While the reader keeps up, stdout flows direct; otherwise OMP spills to a private temp file and drains 64KiB preserving order — disk can grow UNBOUNDED, spool failure disposes with exit 1. Clients MUST keep reading after stdin close. CONSEQUENCE for cedian: (a) never assume bounded memory — the EventRouter reader thread must drain continuously even when GPUI is busy (backpressure at the GPUI batch layer, §73, never by pausing the read); (b) treat `exit 1` + spool-failure signature as a distinct crash class in §74 recovery (transcript may be truncated — reconcile, don't assume intact); (c) the CDP screencast latest-frame-wins above is a SEPARATE policy at the presentation layer and must not be confused with RPC transport backpressure.

> **Scrutinize R4 fix (evidence frame binding).** Dropped frames must never become evidence. Every `ScreenshotEvidence`/`BrowserEvidence.screenshot` carries the CDP frame id it was captured from; a gate holding evidence whose frame id is older than the latest rendered frame marks it `stale-frame` and requires re-capture before `required: true` passes. Displayed screenshots render their capture sequence number in the corner — what the user sees is what the gate evaluated.

---

# 30. Images in Agent Composer

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

---

# 31. Rich Thread Content

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

---

# 32. iOS as a First-Class OMP Tool

Add a native OMP tool:

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

---

# 33. iOS Execution Lives in cedian

OMP should not directly own all Xcode/macOS UI implementation details.

Use:

```text
OMP ios tool
    ↓
cedian iOS Host Service
    ↓
┌─────────────┬───────────────┐
│ xcodebuild  │ xcrun simctl  │
│ WDA/XCTest  │ log stream    │
└─────────────┴───────────────┘
```

---

# 34. iOS Device Lifecycle

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

---

# 35. iOS Build Tool

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

---

# 36. iOS Semantic UI Automation

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

---

# 37. iOS Visual Panel

Phase 1:

```text
simctl screenshot
```

Refresh on action.

Phase 2:

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

---

# 38. Keep OMP Computer Tool

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

> **Grill R2/R3 binding.** "Desktop automation" here means the CUA `cua-driver` contract ONLY (§9, AGENTS.md) — never raw AX calls outside the driver. The driver + macOS Seatbelt profile + bypass-proof test land atomically; until then `computer` stays hard-disabled, including as a fallback.

---

# 39. Ambient Context

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

---

# 40. Deep Context via Host URIs

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

---

# 41. One Thread = One OMP Session

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

---

# 42. Subagents

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

---

# 43. Worktrees

> **Grill decision R1 (C1): cedian owns worktrees, OMP requests.** Zed Project/worktree is the single owner of worktree lifecycle (`git worktree add/remove`, `WorktreeId`). OMP never creates OS-level worktrees directly — it emits a typed request via host service; cedian executes, returns `WorktreeId`, and renders it. OMP keeps orchestration/merge *policy*; cedian keeps *mechanism*. `cedianTask.worktree` stores the id, not the ownership.
>
> **Conflict fix (phase home).** Worktree *mechanism* (`git worktree add/remove` + `WorktreeId` plumbing + host-service request) lands in **Phase 16** together with the visualization — mechanism without UI is untestable by the hermetic contract (§86), UI without mechanism is a mock. v0.3's `✓ worktrees` (§85) therefore means Phase 16 complete, not a partial. No earlier phase may depend on isolated worktrees.

Flow:

```text
OMP requests isolated worktree (host service)
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

---

# 44. Swarm and Arena

Do not create separate orchestration engines.

## Swarm

```text
OMP task batch
 ├── worker A
 ├── worker B
 └── worker C
```

Different workers handle different slices.

## Arena

```text
same problem × N
     ↓
judge
```

Multiple workers solve the same problem, then OMP selects/synthesizes.

---

# 45. Architect, Interrogate, Blast Radius

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

# 46. OMP Engineering Workflow Layer

The workflow layer lives in OMP.

Target:

```text
TaskProfile
    ↓
Playbook
    ↓
Phases
    ↓
Verification Gates
    ↓
Review
    ↓
Complete
```

cedian only renders state.

---

# 47. Workflow Data Model

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

---

# 48. Workflow State

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

---

# 49. Start with a Small Playbook Set

Do not begin with 20+ playbooks.

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

---

# 50. Phase State

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

---

# 51. Workflow UI

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

---

# 52. Verification Gates

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

---

# 53. Evidence as First-Class Data

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

> **Scrutinize R2 fix (evidence carries provenance).** Every evidence variant carries `provenance: { tool_call_id: string } | { unattributed: true }` linking it to the §17 `AgentEdit` store. A `required: true` gate REJECTS `unattributed` evidence — the agent must reproduce the result inside a tracked edit before the gate passes. Optional gates may accept unattributed evidence but must surface it as `unverified-origin` in the UI.

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

---

# 54. Gate Engine Must Not Become a Second Agent

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

> **Scrutinize R2 fix (gate is a pure function, not a scheduler).** The gate engine NEVER performs I/O: no spawn, no browser, no test run, no RPC. It evaluates `GatePredicate` over the already-stored evidence map with a fixed evaluation budget (bounded time, bounded input size). Predicates requiring fresh I/O are rejected at registration — the agent must produce the evidence first, the gate only reads it. This keeps §88 "no second workflow engine" true.
>
> **Anti-loop:** `OMP continues` (§55) is bounded by `max_continue` (default 3) per gate. Each continue must attach at least one NEW evidence item referencing the missing gate, or the counter still advances. On exhaustion: `status = blocked` + force-escalate to the user with `{gate_id, missing_evidence, attempts}` — never infinite loop, never silent success.

---

# 55. Completion Gate

Before OMP says:

```text
Done.
```

It must pass:

```ts
workflow.canComplete()
```

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

---

# 56. Example Gate Policies

## Bug Fix

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

## Feature

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

## Refactor

```text
✓ behavior baseline captured
✓ implementation
✓ tests
✓ behavior after == before
```

---

# 57. Review Roles

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

---

# 58. Reviewers Must Be Read-Only

> **Scrutinize R2 fix (reviewer sandbox profile).** "Generally receive" is not enforcement. Reviewer subagents run under a dedicated OS-enforced profile: `edit`, `write`, and `computer`-actuate are `Deny` at the sandbox layer (same Seatbelt mechanism as §64/Q9), not merely absent from the tool list. `bash` for reviewers is allow-listed to read-only prefixes (`git diff`, `git status`, test-status queries) via execpolicy-style rules (§64 lesson 1) — a reviewer invoking anything outside the list gets `Deny`, not an `Ask`. A reviewer that needs a write to prove a point must file a finding; it cannot self-escalate. The reviewer `bash` allow-list lives in the same policy file as §64 (same CI gate, Q10b) — one owner, no drift.

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

---

# 59. Review Finding Schema

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

---

# 60. Review Aggregation

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

# 61. Workflow Events

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

cedian renders them directly.

---

# 62. Task Complexity Budget

Prevent over-engineering.

Suggested:

```ts
interface WorkflowBudget {
  maxReviewers: number;
  maxParallelWorkers: number;
  allowArena: boolean;
  requireLiveVerification: boolean;
}
```

## Trivial

```text
0 parallel workers
0 reviewers
no architecture
basic verification
```

## Small

```text
0–1 reviewer
no arena
focused tests
```

## Medium

```text
up to 2 reviewers
swarm if independent
live verification if user-facing
```

## Large / High Risk

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

# 63. Ask Tool

OMP `ask` should render as a native cedian GPUI dialog.

> **Upstream fact (oh-my-pi `docs/approval-mode.md` + RPC Extension UI Sub-Protocol, verified 2026-10-06).** OMP approvals DEFAULT TO YOLO (auto-allow all) — strictness must be CONFIGURED, never assumed. `ask` requires explicit opt-in via `set_ask_dialog(true)` (default off; without it, headless/no-UI approval-needing tools FAIL CLOSED — treat that as normal control flow, not an error). `ask` ≠ approval: `ask` is `extension_ui_request{method:ask, questions[{id,question,options[],multi?,recommended?}], timeout?}` with strictly-ordered answers; approvals run through a separate extension runner. Subagents are headless-yolo inside the parent task boundary (residual prompts REJECT). Phase 0.5/2 MUST set `approvalMode: always-ask|write` + `approval.*` + `bash.patterns` + `set_ask_dialog(true)` before any agent turn — the plan's strict-wins (§64) only has teeth once yolo is off.

Example:

```text
Which database?

○ PostgreSQL
● SQLite
○ Other...

[Continue]
```

No terminal prompts.

> **Scrutinize R3 fix (`ask` liveness).** A pending `ask` dialog is a lease, not a lock: if OMP disconnects mid-`ask`, cedian auto-dismisses with decision `abstain` (logged in the audit tuple, §64) and the turn resumes as `blocked` on reconnect — never a ghost dialog awaiting a dead runtime. `ask` timeout (default 5 min, no response) = `abstain` + force-escalate per §54. Strict-wins (R1-C2) applies to the *answer*, not to liveness: cedian may always dismiss a dead dialog.

---

# 64. Permissions

Because cedian is personal, use a simple policy.

> **Grill decision R1 (C2): strict-wins.** OMP `ask`/tool policy and the cedian policy TOML are evaluated independently; the stricter verdict wins (`Deny > Ask > Allow`). cedian policy can never be loosened by an OMP decision, and OMP can never be forced to act by a cedian `Allow`. Every `ask` renders as a native GPUI dialog (§63) with the requesting side labeled.
>
> **Upstream fact (oh-my-pi `docs/approval-mode.md` resolver, verified 2026-10-06) — strict-wins is NOT how OMP resolves.** OMP's real order: tool-`Deny` absolute → user-`Deny` absolute → yolo-mode: explicit tool allow/prompt wins, else user policy, else allow (bare `override` ignored) → non-yolo: `override:true` allows ONLY with tool-allow else prompt; then tool→user→mode-tier. Provider `pendingSafetyChecks` force-prompt EVEN UNDER YOLO; no-UI prompt-needing tools FAIL CLOSED. CONSEQUENCE: cedian strict-wins applies at the CEDIAN GATE (host-tool dispatch + sandbox), never inside OMP's resolver — cedian cannot reorder OMP's pipeline, only refuse at its own boundary. So: (a) Phase 0.5 MUST set `approvalMode: always-ask|write` + `approval.*` + `bash.patterns` + `eval`-gate + `set_ask_dialog(true)` to make OMP's resolver strict in the first place; (b) cedian's gate re-evaluates every granted action independently and can still `Deny` what OMP allowed; (c) fail-closed (no-UI, safety-check) is NORMAL CONTROL FLOW the UI renders, not an error path.

## Safe

```text
read
search
LSP
diagnostics
browser inspection
simulator inspection
```

Allow automatically.

## Project write

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

## Dangerous

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

> **Grill R2 — Codex lessons (`openai/codex`, Rust, 128k⭐, studied 2026-10-05; deep-dived 2026-10-06).** Adopt four mechanisms, adapted to the cedian ownership split. Corrections from the deep-dive are inline:
>
> 1. **Three-valued decisions, not boolean — with Codex's exact semantics.** Codex `execpolicy` (`codex-rs/execpolicy`) has THREE builtins: `prefix_rule(pattern, decision?, justification?, match?, not_match?)`, `network_rule(host, protocol, decision, justification?)`, `host_executable(name, paths)`. `decision` is `Allow|Prompt|Forbidden` in `prefix_rule` ONLY — `deny` is valid SOLELY as a `network_rule` alias for `forbidden`; `prefix_rule(decision=deny)` FAILS to parse. `match`/`not_match` are PER-RULE unit tests evaluated against an EPHEMERAL single-rule policy (basename resolution forced on), not policy-wide integration tests; violations report file+line/col (`ExampleDidNotMatch`/`ExampleDidMatch`). Evaluation: exact-first-token lookup → matchedRules lists EVERY matching rule → effective decision = STRICTEST across all matches → NO match yields decision OMITTED (not allow). cedian adopts: same three builtins, same per-rule validation, same strictest-wins, same no-match-omitted. Canonical spelling stays `Allow|Ask|Deny` (Codex `prompt|forbidden` ≡ `Ask|Deny`).
> 2. **OS-enforced sandbox under the policy, not policy alone — starter profile identified.** Codex layers `codex-rs/sandboxing` per-OS backends (macOS Seatbelt `.sbpl`, Linux Landlock/bwrap/namespaces, Windows restricted tokens) + `linux-sandbox` launcher + `windows-sandbox-rs` — so a `Deny` verdict is enforced by the kernel even if the agent bypasses the policy check. cedian mirrors this: the CUA `cua-driver` contract (R2-6a) is the enforcement floor for `computer`; filesystem/network/process verdicts in `permissions.rs` must map to an OS mechanism (Seatbelt profile on macOS first), never live as TOML-only intent. Adaptation seed: `codex-rs/sandboxing/src/seatbelt_base_policy.sbpl` (~3.9KB, deny-default + process-exec/fork + sysctl/mach/pty allows) composed per `seatbelt.rs` (BASE + generated read/write roots + unlink-deny anchors + network fragment + `sandbox-exec -p PARAMS`); hardenings to copy verbatim: canonicalize-then-compare paths, write-exclusions deny BOTH literal and subpath, proxy-unparseable ⇒ fail-closed empty network policy, execute ONLY `/usr/bin/sandbox-exec`. Linux replication is thousands of lines (`linux_run_main.rs` ~1.5k + `bwrap.rs` ~120KB) — macOS-only cedian has no such obligation.
> 3. **Protected metadata paths.** Codex `codex-protocol::permissions::PROTECTED_METADATA_PATH_NAMES` (imported by `seatbelt.rs`, NOT a local list) hard-blocks agent writes to `.git/.codex/.agents/.aws` even under writable roots. cedian adopts the same list (plus `.cedian/`, OMP session dir): metadata writes are `Deny`, not `Ask` — no prompt fatigue bypass. (Canonical spelling `Deny`; `forbidden` here cites the Codex symbol only.)
> 4. **Escalation as a first-class session — audit envelope clarified.** Codex `shell-escalation` (`EscalateServer`, `EscalationSession`, `PreparedExec`, `ResolvedPermissionProfile`) re-resolves permissions per exec; sessions persist as JSONL rollout lines `{timestamp: RFC3339, ordinal?, ...item}` under `$CODEX_HOME/sessions/` (`codex-rs/rollout/src/recorder.rs`, replayable via jq/fx). CAVEAT: no dedicated per-tool `(tool_call, approval_decision, policy_outcome)` tuple was confirmed upstream — cedian's `{tool, command/prefix, decision, scope (once|task|session), timestamp}` tuple is a cedian EXTENSION inside the Codex `{timestamp, ordinal, item}` envelope, not parity. `decision: Allow | Ask | Deny | Abstain` (canonical). `Abstain` is system-generated (lease expiry / dead runtime, §63): deny-for-execution, blocked-for-gates (§54), never writable in policy.

---

# 65. Plan / Normal / Deep Modes

> **Scrutinize R4 fix (mode parity is measured, not assumed).** `set_mode` is a Phase 0.5 capability-table row (`normal | plan | deep | goal` — add it). If RPC lacks a mode OMP's normal UI has: (a) Phase 2 ships with only the modes RPC supports, (b) the missing mode is an explicit OMP-side addition (§8) with owner + phase, NOT a silent fallback to `normal`. A mode switch mid-turn aborts the turn (same path as `abort`) and starts a new turn under the new mode — never mid-stream reinterpretation.
>
> **Upstream verdict (oh-my-pi, verified 2026-10-06): `set_mode(normal|plan|deep|goal)` DOES NOT EXIST in `RpcCommand` — and `deep`/`normal` are not OMP modes at all.** `--mode` is `text|json|rpc|acp|rpc-ui` (launch transports, `docs/cli-reference.md`); plan+rpc are MUTUALLY EXCLUSIVE at launch (`issues/5380` open, maintainer-gated). `goal` IS RPC-controllable (`get/create/resume/pause/drop`, continuation needs `goal.continuationModes ∋ rpc`) but is not a drop-in for plan/deep. CONSEQUENCE: delete `Deep` from the cedian mode switch NOW (no upstream to bind to); `Plan` ships only via an OMP-side addition (§8) with owner + phase, or Phase 2 cuts to `Normal + Goal`. The mid-turn-abort contract above stands for whichever modes land.

If RPC mode lacks parity with OMP's normal UI modes, extend the protocol.

cedian UI:

```text
[ Agent ▼ ]

Normal
Plan (only if OMP-side addition lands, §8 — else hidden)
Goal (only if continuationModes ∋ rpc)
```

No `Deep` entry: the mode does not exist upstream.

OMP still owns execution behavior.

---

# 66. Tool Card Registry

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

---

# 67. Tool Card Examples

## Edit

```text
Edited auth.rs

+18 −6

[Open]
[Review]
```

## Bash

```text
cargo test

✓ 182 passed
2.3s

[Output]
```

## LSP

```text
References

Session::cancel
14 references

[Open results]
```

## Debug

```text
Paused

session.rs:182

Thread: main
Reason: breakpoint

[Open Debugger]
```

## Browser

```text
Browser

localhost:3000/login

✓ clicked Login
✓ redirected /dashboard

[Open Browser]
```

## iOS

```text
iPhone 18 Pro

✓ Build
✓ Launch
✓ Login flow

[Open Simulator]
```

---

# 68. Generic Tool Card Fallback

If OMP adds a new tool unknown to cedian:

```text
Tool: some_new_tool

Running...
```

Render generic structured output.

Do not break the UI.

---

# 69. One Window by Default

Use one application/workspace.

## Code Mode

```text
Files | Editor | Agent
──────────────────────
Terminal
```

## Agent Mode

```text
Threads | Agent Task
        | Browser
        | Workflow
```

## Review Mode

```text
Changes | Diff
        | Findings
```

The workspace morphs between tasks.

---

# 70. Multi-Window as a Projection

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

---

# 71. Runtime Lifecycle

When opening a project:

```text
1. cedian opens workspace
2. start OMP child
3. wait ready
4. negotiate RPC v2
5. verify cedian protocol version
6. register host services
7. register host URIs
8. enable ask dialog
9. subscribe subagents
10. apply event filters
11. restore OMP session if one exists
12. ready
```

> **Scrutinize R4 fix (steps 9–10 are explicit protocol).** `subscribe subagents` and `apply event filters` are typed RPC calls with versioned schemas, not log lines. Unknown event types after filtering = tolerated + counted (metric `cedian.unknown_event_total{event_type}`), never crash, never silent-drop without the counter. A filter schema mismatch fails startup loudly (same path as protocol mismatch, §6) — filters must never silently narrow to zero events.

---

# 72. Event Router Threading

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

---

# 73. Streaming Text Batching

Do not repaint for every token.

Use:

```text
TextDeltaBuffer
```

Flush approximately every frame or every ~16–33 ms.

---

# 74. Crash Isolation

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

> **Scrutinize R3 fix (crash-during-stream).** `open_session` restores the transcript, NOT the in-flight stream. On reconnect cedian MUST: (1) discard the partial `TextDeltaBuffer` (never render half a tool call as complete), (2) reconcile: any `tool.started` without matching `tool.completed` in the EventRouter log is marked `INTERRUPTED` and its partial edits become `UNATTRIBUTED` (§17) until re-driven, (3) reset `max_continue` counters tied to the interrupted turn (§54) — a crash must not consume the agent's retries. Covered by Agent Lifecycle Tests (`runtime crash`, `runtime reconnect`).
>
> **Non-finding (cursor/plugins `pstack`, checked 2026-10-06).** `pstack` is a prompt-only Cursor plugin (poteto-mode playbooks, no daemon/supervisor/sandbox code) — contributes NO implementable mechanism for §§74–76 and is deliberately NOT cited as evidence anywhere in this plan. Its only transferable idea (checkpoint-on-`wip`-commit + off-context resume note) is already covered by `open_session` directory-adopt + `fork`/`branch` above.

---

# 75. Session Persistence

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

> **Lessons from `herdrdev/herdr` (Rust agent runtime, 42k⭐, studied 2026-10-06).** Adopt three mechanisms, adapted to cedian's GPUI (not terminal) surface:
>
> 1. **Pane status as first-class signal.** herdr marks every pane `working | blocked | idle` and pushes attention when an agent stops needing an answer (`agent_view_eval.rs`: `status()`, `attention()` seq per entry). cedian's subagent tree (§42) + tool cards (§66) adopt the tri-state, derived from EventRouter state (running turn → `working`, pending ask/abstain → `blocked`, else `idle`); a `blocked` task surfaces in the panel WITHOUT opening it, never via polling.
> **Ponytail cut (2026-10-06): no attention counter in v1.** herdr's monotonic attention-seq is a cross-process ordering mechanism — unnecessary single-process. A bool `needs_attention` reset on task-open suffices; upgrade when multi-window needs cross-window ordering.
> 2. **Agent resume as validated argv, not free text.** herdr's `agent_resume.rs` constrains resume commands (bare command name, ≤64 args, ≤8KiB, no control chars/apostrophes, cwd part of the dedupe identity). cedian's §74 respawn adopts the same discipline: the respawned `omp --mode rpc-ui` command line is constructed from a validated struct, never string-concatenated; `{binary_path, session_dir, cwd}` is the dedupe key so two workspaces never share a runtime by accident.
> 3. **Versioned snapshots.** herdr's `persist/snapshot.rs` carries `SNAPSHOT_VERSION` and rejects incompatible restores. cedian's `workspace ↔ session ↔ task` mapping store (§75) + review baseline (§16) carry a `snapshot_version: u32` from day one — old state fails closed with "state too old, re-baseline" instead of silently misreading.

---

# 76. cedian Crash / Shutdown

OMP is a child process.

> **Market pattern (Synara quit-dialog + guarded resume, verified 2026-10-06).** Synara lists running chats on quit (Cancel / Quit + persistent "Resume chats automatically" checkbox) and resumes ELIGIBLY on relaunch — skipping completed/archived/newer-work/dead-project chats, with a bounded quit-wait that falls back to plain interruption if the intent can't be recorded. cedian adopts the same shape, adapted to one harness:

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

Rules: resume defaults to BLOCKED requiring one click (never auto-continue mutating work while the user was away — matches the (b) decision); `computer` actuation from a resumed turn requires fresh `Ask` even if previously granted (grants don't survive restart). Crash (not quit) follows §74 reconcile, then the same blocked-resume.

> **Composer drafts (plan gap, added 2026-10-06).** The transcript restores via OMP — but unsent composer text had NO store. Every composer persists its draft per task (debounced ~500ms, `snapshot_version`-stamped §75) including attachments/refs; on reopen the draft restores verbatim with an "unsent draft" hint. Drafts die with their task (archived/deleted ⇒ draft deleted, never orphaned).

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

# 77. Config Ownership

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

Permission policy + sandbox profiles live on the cedian side (TOML + Seatbelt `.sbpl`, §64): they are enforced at the IDE process's OS layer, so they version with cedian, not `.omp/`. OMP never ships its own copy.

---

# 78. Zed Agent Code

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

# 79. No Generic Harness Abstraction

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

---

# 80. Browser + Agent Shared State

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

# 81. iOS + Agent Shared State

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

# 82. Workflow Verification Example — Web Bug

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

---

# 83. Workflow Verification Example — iOS Bug

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

---

# 84. Implementation Roadmap

> **Stack lock: Rust + GPUI only.** No TypeScript/Electron/WebView/Tauri in the cedian process. The only TS in scope is OMP-side additions under `packages/coding-agent/src/` (§8) — that code lives in the OMP repo, not cedian. `gpui-kit` (gpui-base unstyled primitives + gpui-component 75+ styled components, Apache-2.0, `github.com/longbridge/gpui-kit`) and `elygpui.com` (Ely GPUI components, MIT/Apache-2.0) are approved UI accelerators: prefer them over hand-rolling panel/composer/tool-card/message/settings/dialog/toast components, but editor/buffer/multibuffer/diff surfaces stay Zed-native (never reimplement on top of kit components).

## Phase 0 — Fork Hygiene

Build:

```text
Zed fork builds
OMP fork builds
pinned upstream remotes
upstream rebase cadence + conflict owner
Zed license audit for redistributed binary (GPL terms)
macOS signing / notarization owner for cedian.app + bundled omp
cedian branding
CI build
UI kit evaluation: pin gpui-kit + elygpui revisions, verify GPUI version compat with Zed fork
```

Acceptance:

```text
cargo run cedian
```

works as a normal Zed-derived editor.

---

## Phase 0.5 — RPC Spike (gating)

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

---

## Phase 1 — OMP Runtime Inside cedian

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

---

## Phase 2 — Native Agent Panel

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

---

## Phase 3 — Tool Card Registry

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

---

## Phase 4 — Editor-Native Edit Surface (host tools + cedian:// URIs; no OMP backend seam — §10)

> **Scrutinize R2 fix (ordering):** this phase MUST precede Review (Phase 5). `clock::Global` baselines (§16) have no meaning before `cedianWorkspaceHost::buffer_version` exists.

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

---

## Phase 5 — Edit Provenance + Review Changes

> **Scrutinize R2 fix (ordering):** requires Phase 4 (baseline type + transaction hook). Do not start without `buffer_version` + `apply_edit` landing.

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

---

## Phase 6 — Native Context

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

---

## Phase 7 — Zed LSP Surface (no OMP backend seam — §10)

Implement (cedian host tools + cedian:// URIs surfacing Zed LSP state; OMP `lsp` tool unchanged):

```text
cedian host tools (definitions/references/hover/symbols/rename/code-action/diagnostics)
cedian:// diagnostics + symbol URIs
```

Acceptance:

> OMP and cedian use the same language server state.

---

## Phase 8 — Zed DAP Surface (no OMP backend seam — §10)

Implement (cedian host tools + cedian:// URIs surfacing Zed DAP state; OMP `debug` tool unchanged):

```text
cedian host tools (breakpoint sync/stack/variables/step/continue)
```

Acceptance:

> Agent-created breakpoints appear in native debugger and both user/agent control the same session.

---

## Phase 9 — Shared Browser

Implement:

```text
cedian-owned Chromium lifecycle
CDP endpoint
OMP browser connects to same Chromium
screencast
mouse/keyboard forwarding
browser pane
console/network integration
```

Acceptance:

> User and OMP interact with the same tab.

> **Grill decision R1 (C5): user input preempts.** Same-tab sharing is view-sharing, not input-sharing. The moment cedian detects user pointer/keyboard activity in the shared tab, the agent's input stream pauses (event → OMP: `browser_input_preempted`), a banner shows `Agent paused — you took control [Resume agent]`, and OMP must re-request input focus before acting again. Agent never queues synthetic input ahead of live user input.

---

## Phase 10 — Workflow Engine [GATED TRACK — requires Phase 0.5 signal on `host tools` + gate pure-function contract (§54)]

Implement:

```text
TaskProfile
Playbook
Gate
Evidence
WorkflowState
RPC events
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

---

## Phase 11 — Browser Verification

Browser becomes an evidence provider.

Acceptance:

> OMP cannot mark a browser-facing task complete while a required browser verification gate is missing.

---

## Phase 12 — Review Agents [GATED TRACK — requires reviewer sandbox profile (§58) + audit-tuple logging (§64) landing first]

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

---

## Phase 13 — iOS Core [EXTENSION TRACK — off the fully-native critical path]

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

---

## Phase 14 — iOS Semantic Interaction [EXTENSION TRACK]

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

---

## Phase 15 — iOS Native Panel [EXTENSION TRACK]

Implement:

```text
ScreenCaptureKit
Simulator window capture
GPUI rendering
input forwarding
```

Acceptance:

> Simulator appears live inside cedian.

---

## Phase 16 — Subagent / Worktree UI

> **Conflict fix (phase home).** This phase owns BOTH worktree mechanism (§43: `git worktree add/remove`, `WorktreeId` plumbing, host-service request path) AND visualization. Mechanism and UI land together — neither ships alone.

Implement:

```text
subagent tree
progress
cancel
steer
worktree mechanism (add/remove, WorktreeId, host-service request)
worktree visualization
```

Acceptance:

> Parallel OMP workers are visible, while OMP remains the only orchestrator.

---

## Phase 17 — Swarm / Arena

Implement last.

```text
Swarm = task partition preset
Arena = same-task parallel preset
Architect = design Arena preset
Interrogate = ReviewPolicy preset
```

No new engine.

---

## Phase 18 — PR Workspace

> **Rationale (market parity, 2026-10-06).** Synara + Claude Desktop both ship browse/review/merge PRs + stacked PRs + CI auto-fix in-app. cedian's Review Changes (§§15–20) stops at the working tree — the PR is where review actually ships. Cheap to build: `gh` CLI + existing review pipeline, no new engine.

Implement (all via `gh`, cedian renders natively) — v1 scope ONLY (Fix-button, stacked-PR position, pinned list are follow-ups with their own acceptance, not this phase):

```text
PR list (per repo)
diff review (reuse §15–20 pipeline on PR range, not task baseline)
inline comments → ReviewFeedback payload (§19) → OMP Fix
CI status bar + auto-fix toggle (read check output → iterate, bounded like §54 max_continue)
merge (explicit confirm; method = squash default)
```

Rules: PR diffs use the PR base as baseline (§16 task-baseline stays for working-tree review — two baselines, labeled in UI, never mixed). Destructive actions (merge, close) are `Deny`-by-default in §64 policy (explicit per-action `Ask`, never auto-allow). CI auto-fix turns count against the task's `max_continue`.

> **Ponytail cut (2026-10-06).** Dropped from v1: Fix-button (comment grouping), stacked-PR position/readiness, pinned repos, safe-prefix merge — each is a separate feature needing its own acceptance; bundling them guarantees a half-done phase.

Acceptance:

> Open PR → review diff → comment → Fix → CI green → merge, without leaving cedian.

---

## Phase 19 — Automations (Scheduled Runs)

> **Rationale + scope (user decision, 2026-10-06).** Synara ships scheduled recurring runs; cedian adopts a LOCAL-only version — no cloud runner, no remote queue. Justification: the machine stays on 24/7, so a local scheduler suffices and avoids an entire second infrastructure (server, auth, billing, remote sandbox).

Implement:

```text
schedule (cron expr; plain-language → cron is a follow-up, not v1)
run history (reuse audit-tuple log, §64)
stop conditions (evaluated as pure predicates, §54-style: bounded, no I/O)
consecutive-failure limit (default 3 → auto-pause + notify, same shape as max_continue)
wake = spawn turn in existing workspace (same lifecycle as §71, steps 1–12)
```

Rules: automations run under the SAME permission profile as interactive turns (no privilege elevation for background); `computer` actuation is `Deny` for automation runs until the user explicitly allows per-automation. Every scheduled run emits the same provenance (§17) and evidence (§53) as interactive work — gates apply identically. No cloud/SSH execution, EVER (out of scope, §91).

> **Ponytail cut (2026-10-06).** Dropped from v1: NL→cron parsing (cron expr typed directly; OMP translation is its own feature with its own misparse risk), lid-closed guarantee (acceptance is "history + evidence visible on return from sleep", not a power-management promise — no `IOPMAssertion`, no wake-from-sleep; machine-on-24/7 is the user's setup, not cedian's contract).

Acceptance:

> Scheduled run fires while the machine is awake → history + evidence visible on return.

---

# 85. MVP Releases

## cedian v0.1

```text
✓ Zed fork
✓ bundled OMP
✓ native OMP Agent panel
✓ prompt/stream/cancel
✓ images
✓ native tool cards
✓ source edits
✓ Review Changes
✓ Git
✓ LSP
✓ DAP
✓ terminal
```

This should already be usable as the daily IDE.

---

## cedian v0.2

```text
✓ task profiling
✓ workflow UI
✓ verification gates
✓ review agent
✓ shared browser
✓ browser screenshots
✓ DOM/context
✓ browser evidence
```

This makes cedian meaningfully more agentic.

---

## cedian v0.3

```text
✓ worktrees (= Phase 16 complete: mechanism + visualization, §43)
✓ parallel subagents
✓ Swarm
✓ Architecture Arena
```

iOS (simulator, visual verification) moved to the extension track — ships after v0.3, not gating it.

---

## cedian v0.4

```text
✓ PR workspace (browse/review/merge, stacked PRs, CI auto-fix)
✓ automations (local scheduler, same gates + provenance as interactive)
```

OMP-only multi-provider note: cedian intentionally speaks to ONE harness. Multi-model choice lives INSIDE OMP (provider/model routing, `set_model`) — cedian never adds a second harness adapter to chase providers. If OMP gains a provider, cedian gains it for free.

---

# 86. Test Strategy

Use at least five major test layers.

> **Harness contract (anti-flake rule):** agent E2E must be hermetic. Provide a fake `omp --mode rpc-ui` that replays recorded RPC frames, plus recorded session fixtures. Layers 1–2 run fully hermetic (no model calls, ever). Layers 3–5 default hermetic; any live-model case is explicitly tagged `live-model`, runs in a separate nightly lane, and never gates merge. No test may depend on wall-clock timing looser than the debounce windows in §§20/73.

## RPC Contract Tests

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

---

## Editor Integration Tests

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

---

## Agent Lifecycle Tests

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

---

## Browser Tests

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

---

## iOS Tests

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

# 87. Definition of “Fully Native”

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

□ no second harness
□ no ACP dependency in core architecture
□ no duplicate orchestration engine
□ no duplicate context engine
□ no duplicate subagent runtime
```

When this is true:

> **cedian = fully OMP-native agentic IDE.**

---

# 88. Things We Explicitly Must Not Build

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

---

# 89. Out of Scope (Deliberate Cuts)

Decided 2026-10-06, not deferred — these return only if the destination is redrawn:

```text
❌ cloud/SSH sessions — machine stays on 24/7; local scheduler (Phase 19) covers recurrence. No remote runner, queue, or billing, EVER.
❌ second harness adapters — OMP already routes providers/models internally; cedian speaks OMP, OMP speaks the world.
❌ custom browser engine (Servo evaluated 2026-10-06: no full CDP, web-compat gap — revisit only if it speaks CDP completely)
❌ Ghostty-as-terminal (Zig FFI for what Zed PTY already gives — evaluated 2026-10-06)
❌ VM sandbox for computer tool (Lume/Spaces deferred past CUA driver-only landing)
```

> **Daemon track (future boundary, NOT scheduled).** Covers the last 10%: survive quit AND reboot like tern/herdr (engine as launchd daemon, not a child of the window). Trigger: real users complaining about reboot-lost work AFTER v0.4 quit-resume (§76) ships. Cost when triggered: §3 child→daemon+IPC redesign, §§71–76 lifecycle rewrite, IPC auth + grant expiry + stale-lock ownership (Synara 1.0.0 pattern), separate TCC/sandbox profile for the daemon, two-process dev loop. Do NOT start without the trigger — quit-resume covers 90%.

---

# 90. Final Architecture

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
                    │ Worktrees   │
                    │ Workflow    │
                    │ Verify      │
                    │ Review      │
                    └─────────────┘
```

---

# 91. Final Design Principle

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
