# cedian architecture

Decided 2026-10-21 in a planning discussion with the owner, then corrected the same day by a review against cedia's code ("รีวิวแผน cedian ก่อน Phase A"). The full plan, with the roadmap and risks, is the "แผนสร้าง cedian v1" doc.

## What cedian is

The native macOS desktop for [cedia](https://github.com/pongsathonkheereekaew/cedia). cedia is the agent: loop, providers, tools, permission, sandbox, threads and trust rules all live there. cedian runs cedia, draws its threads as panes, and adds what only a desktop can give: a shared browser and terminal, editor buffers, and images and video shown inline. cedian never runs an agent loop of its own.

## Shape

```text
 window (Rust + GPUI)       later: phone, other Mac
      │                          │
      └──── Unix socket ─────────┘  (QUIC later)
                 │
          cedian daemon (Rust, its own process)
          runs cedia per prompt; terminals, worktrees,
          shared Chromium, keep-awake, settings watch
                 │ argv in, stream-json out
      ┌──────────┼──────────┐
   cedia -p   cedia -p   cedia threads send   (bundled, pinned build)
      └──────────┼──────────┘
          ~/.cedia: settings.json
                    projects/<slug>/threads/*.jsonl
                    sessions/<slug>/*.jsonl
                    snapshots/<worktree slug>/   (shadow git)
```

- The daemon is always its own process, registered as a LaunchAgent (`SMAppService`). The window starts it when it is not running and talks to it only over the socket, in development too. Quitting the window never stops a thread.
- One prompt is one cedia run: `cedia -p <text> --thread <id> --output-format stream-json`, with the thread's worktree as `--cwd`. A thread with no running prompt has no process. Text typed while a run works goes in with `cedia threads send <id> <text>` and reaches the model at the next turn boundary. SIGTERM aborts. A long-lived `cedia serve` is added only when host tools need it.
- `<slug>` is the project's: every worktree of a repository files its threads and sessions under the main worktree's root (cedia PR #19). Snapshots stay per worktree.
- Pane kinds in v1: agent, terminal, file (text, image, video, PDF and Office through Quick Look, with a file tree), editor, review, browser. No agent sidebar: the agent is a pane of blocks.
- One `open(target, how)` routes any file or URL to the right pane kind.

## Thread states

cedia writes four: `working`, `done`, `failed`, `stopped`. The daemon adds `idle` (no run in progress). There is no `waiting`, because cedia never stops to ask. A thread whose last state is `working` but whose process is gone (crash, reboot) shows as stopped and offers to continue it with `--thread`. Tabs show the state, with a notification and sound when a thread finishes or fails, and a `verified` badge when its last receipt has no unverified or flagged files.

## Decisions

| Topic | Decision |
| --- | --- |
| Users | The owner only. No onboarding, accounts or team features |
| Scope | Coding at the core, plus dev tools around it |
| Working mode | Delegate first, then parallel threads, a worktree each |
| Agent | cedia is the only agent |
| Stack | Rust + GPUI as a library; Zed's `editor`, `language`, `lsp`, `terminal` crates as pinned git dependencies where they save work |
| Zed pin | Zed v1.25.2 (`d25fa01`) with toolchain 1.98.1, the same as Zed's. Zed's `[patch.crates-io]` and its `Cargo.lock` are copied at the pin, and `util`'s `debug-embed` feature is on so dev builds embed Zed's assets. A bump moves every Zed rev, the patch section and the lock together |
| Process model | The daemon runs one cedia per prompt (above) and supervises it; the daemon itself restarts with backoff |
| Contract | cedia's `src/protocol/protocol.schema.json` is the source of cedian's Rust types (generated with `typify` in `cedian_protocol`'s build script, strict about unknown fields), and cedia's `test/fixtures/sessions/` are what cedian's replay tests read. `script/pin-cedia <checkout> <rev>` copies both into `crates/cedian_protocol/cedia` with the rev, so a pin bump is one reviewed diff |
| Tools | Agent tools live in cedia. cedian offers host tools only for desktop-owned things (open buffers, the shared terminal's output), never a second tool with a cedia tool's name. The browser tool is cedia's; cedian only hands it the shared Chromium's CDP endpoint |
| Media | Images and video go to a model that reads images: when a thread has media and the code model is text-only, cedia starts a new session of the thread on Haiku 5.5 |
| Bundling | cedian ships a pinned cedia build (`bun build --compile`) and `rg`, never the copies on PATH |
| Environment | The daemon reads the login shell's environment once at start, so cedia finds `git`, `gh` and `claude` as it does in a terminal |
| Repos | `cedia` and `cedian`. cedia's protocol types are the source; cedian's Rust types follow them |
| Settings | One file, `~/.cedia/settings.json`, with a `desktop` key for cedian (below) |
| Secrets | API keys live in the macOS Keychain. cedia reads them there itself after the environment, so the CLI and the desktop share one store; cedian never passes or copies them |
| Models, auth, permission, sandbox | As cedia's SPEC: API keys only, never stops to ask, `sandbox-runtime` around bash |
| Git | The daemon's own git commands (worktrees, the git pane) run with hooks off (`core.hooksPath` set to an empty directory); a cloned repo's hooks are code nobody trusted yet |
| Remote | Protocol ready from day one; remote client later |
| Platform | macOS on Apple silicon; standard macOS keys; macOS dictation in the composer |
| Tests | Replay cedia's recorded fixtures; no model calls in tests |
| Performance | Budgets are gates, measured from the outside the same way on cedian and the Claude desktop app: cold start, keystroke latency, scrolling a 10k-block thread at 120 fps, idle memory |
| Reactions | cedian draws cedia's `message` and `delivered` events as reactions on your messages; your 👍/👎 on an answer goes in through `cedia threads rate` as an event for picking eval cases and never reaches the model |
| Bots | A bot is a named profile in cedia's own `profiles` settings key (role, model, instructions, tools), started with `--profile`. Start a thread with one, or @mention one to send it work in its own thread through cedia's `message_thread`. Never several bots answering in one conversation (one session, one model) |

## Settings

`~/.cedia/settings.json` is the single source of truth for both programs. cedia owns its schema and precedence (flags > env > user > defaults; a repo's `.cedia/settings.json` only adds deny rules, `zdr` and checks), and a key it does not know fails the load. cedian's own keys live under `desktop` (theme, font, keymap, layout, default view mode, pinned and grouped threads, notifications, browser), which cedia accepts as an object of any shape. The settings UI edits that file, writing only the `desktop` key and replacing the file atomically; the daemon watches it, and the next run reads it. Agent concepts (roles, profiles) are cedia's keys, never `desktop`'s.

## Review and editing

- Review is against the thread's first snapshot, never git HEAD; diffs come from cedia's shadow git in `~/.cedia/snapshots`.
- Rejecting a hunk writes the file in the worktree, then tells the thread with `cedia threads send` (file, hunk, why), so the model sees the change at its next turn.
- A hunk the person edited after the agent is STALE and never rejected.
- When the agent writes a file whose buffer has unsaved edits, the disk keeps the agent's version and the buffer offers Override or Discard; Override tells the thread the same way.

## What cedia needs to add

Done in cedia PR #19: project identity across worktrees, the `desktop` key with unknown keys failing loudly, the protocol schema and session fixtures, `permission_reply` removed. Still to add, in the order cedian needs them:

1. `cedia --version --json` with version and capabilities (phase B).
2. `prompt.images` and the media route to Haiku 5.5 (phase B).
3. `threads rate`, `threads unsend` for queued messages, `threads delete`, and a `title` event for rename (phases B and C).
4. Keychain fallback for API keys (phase B).
5. `profiles` and `--profile` (phase C).
6. `open_thread` for a coordinator, and per-project memory and files (phase C).
7. Media references (video frames, original file) in tool results (phase D).
8. `cedia serve` with `host_tools`, `host_call` and `host_result`, once a host tool is needed (phase C).

## Projects

cedia's threads already follow Claude Projects: a thread is one line of work made of sessions joined by handoff and `recall`, and threads message each other. cedian adds the project layer above them: a project pane with a coordinator thread (a `coordinator` profile that opens threads with `open_thread`, each in its own worktree), pause for everything the coordinator opened, shared memory and files under `~/.cedia/projects/<slug>/`, an outputs card across threads, and each thread's todo list drawn as a checklist on its tab.

## Roadmap

| Phase | cedian | cedia | Exit gate |
| --- | --- | --- | --- |
| 0 Clean | Repo cleared | Contract (PR #19) | Both repos clean; schema and fixtures merged (done) |
| A Window | Spike first: GPUI window plus Zed `editor` and `terminal` compiling at one pinned rev, with Zed's `[patch]` entries copied and the toolchain matched; compiled cedia passing its sandbox tests on a Mac. Then the agent pane over the fixtures | Stage 2 | A fixture renders; replay test green; performance budgets measured |
| B One live thread | Daemon as a LaunchAgent, runs per prompt, reconcile and keep-awake; terminal (attach selection), review, settings UI; message queue with Send now; receipt card with next steps and the verified badge; reactions; view modes, context meter, thread details; window and layout restored after quit; worktree setup with progress and branch names from the first message; open in other apps | `--version --json`, prompt images, rate, unsend, Keychain | Delegate a task into a worktree, quit and reopen with the thread intact, review and accept |
| C Parallel | Many threads, tabs by state, notifications, search, pin and group; shared Chromium with capture and annotate; project pane with coordinator; bot profiles and @mention; side chat, background tasks pane, task chips, app preview with per-site approval, safe archive; file tree; usage page by day, model and project; storage view; export; scheduled runs; Open With from Finder; MCP card | Stage 5: sub-agents, MCP, web, host tools, profiles, `open_thread` | Three threads at once, memory measured |
| D Media, depth | Video and image panes, editor with LSP and line comments, git pane, surface protocol, pop-out panes | Stage 6 plus video read | The agent fixes a bug shown in a screen recording |
| Later | Remote over QUIC, computer use via `cua-driver`, voice mode | Classifier, goal mode | |

## Phase A spike

Done on Linux (2026-10-22): `cedian_window` opens a GPUI window with Zed's `editor` above a login shell from Zed's `terminal` crate, drawn as plain text lines. It builds and runs under Xvfb with Mesa's software Vulkan, and writes no Zed data directories. What it showed:

- `editor` is not a small crate: it pulls in Zed's `workspace`, `project`, `client`, `rpc`, `dap` and `db`, about 1,050 crates in all. A clean debug build takes about 5 minutes on 4 cores, and a rebuild after editing cedian's own crate about 20 seconds, so the size costs disk (a 5 GB `target`) more than time.
- A release build takes about 16 minutes and gives a 119 MB stripped binary on Linux. Idle memory with the window open is about 180 MB under software Vulkan, which is not yet the macOS number the performance budget needs.
- `terminal` stands alone (26 direct dependencies). `terminal_view` needs `workspace`, so cedian draws the terminal's cells itself.
- Zed's crates read their assets from the Zed checkout at run time in debug builds. `util/debug-embed` turns that off, so no dev build depends on a checkout path.

The agent pane followed on the same day. `cedian_agent` folds cedia's stream-json lines into blocks (user, text, thinking, tool call with its permission and result, session, receipt). Streamed text shows until the turn's assistant event replaces it, and a sub-thread's events, which arrive in the parent's stream, go to a transcript of their own, keyed by thread. `cargo test -p cedian_agent` replays every pinned fixture against an outline in `tests/replay/`, and `cedian --replay <stream.jsonl>` plays one into the window a line at a time.

Still to do on a Mac: the same window on macOS (it needs Xcode for Metal shaders, or GPUI's `runtime_shaders` until then), the compiled cedia passing its sandbox tests, and the performance budgets.

## Learned from the Claude desktop app

From Anthropic's public [desktop docs](https://code.claude.com/docs/en/desktop), the [Cowork changelog](https://claude.com/docs/cowork/changelog) and first-hand use, never from the app's code (Anthropic's Consumer Terms §3 forbid reverse engineering). Each maps onto something cedia has or the list above adds:

- Steering: text typed while a thread works goes in as a cedia `message`, delivered at the next turn boundary; queued messages can be edited, removed or sent now.
- View modes (answers, plus thinking, plus every tool call), a context meter from `usage`, and a thread details card (environment, context, outputs, changes, servers, background tasks).
- Side chat: cedia `fork` at the current event into a throwaway session. Editing the last message is a fork before it.
- Background tasks pane over sub-threads and running bash calls; task chips that open a new thread with its own worktree.
- App preview: the browser pane runs the project's preview command from `settings.json` and captures after writes, with a per-site allow list.
- Archive checks a worktree for uncommitted changes first; pop-out panes; keep-awake while working; recovery after a crash or sleep.

Where cedian aims to beat it rather than match it: proof of done from the log, cost per thread with fallback instead of waiting for limits, one timeline for runs started in a terminal or the window, a daemon that outlives the window, secrets redacted before the model sees them, and your ratings feeding cedia's evals.

## Ideas carried over from the earlier cedian

- Evidence is bound to code state and goes stale after a later edit.
- One browser per workspace, shared by person and agent, with captures bound to a frame.
- A normal turn has no ceremony; extra checks engage by risk.
