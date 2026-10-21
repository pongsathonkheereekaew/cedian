# cedian architecture

Decided 2026-10-21 in a planning discussion with the owner. The full plan, with the roadmap and risks, is the "แผนสร้าง cedian v1" doc.

## What cedian is

The native macOS desktop for [cedia](https://github.com/pongsathonkheereekaew/cedia). cedia is the agent: loop, providers, tools, permission, sandbox, threads and trust rules all live there. cedian runs cedia, draws its threads as panes, and adds what only a desktop can give: a shared browser, editor buffers, and images and video shown inline. cedian never runs an agent loop of its own.

## Shape

```text
 window (Rust + GPUI)       later: phone, other Mac
      │                          │
      └──── Unix socket ─────────┘  (QUIC later)
                 │
          cedian daemon (Rust)
          spawns one cedia per thread; terminals, worktrees,
          shared Chromium, host tools, settings watch
                 │ stdio JSONL
      ┌──────────┼──────────┐
   cedia      cedia      cedia      (bundled, pinned build)
      └──────────┼──────────┘
          ~/.cedia: settings.json, projects/<slug>/threads/*.jsonl
```

- One binary, headed or headless. Headed with no daemon running starts the daemon in-process and still serves it on the socket.
- Pane kinds in v1: agent, terminal, file (text, image, video), editor, review, browser. No agent sidebar: the agent is a pane of blocks.
- One `open(target, how)` routes any file or URL to the right pane kind.
- Thread states (idle, working, waiting, done, failed) show on tabs, with a notification and sound when a thread finishes or fails.

## Decisions

| Topic | Decision |
| --- | --- |
| Users | The owner only. No onboarding, accounts or team features |
| Scope | Coding at the core, plus dev tools around it |
| Working mode | Delegate first, then parallel threads, a worktree each |
| Agent | cedia is the only agent |
| Stack | Rust + GPUI as a library; Zed's `editor`, `language`, `lsp`, `terminal` crates as pinned git dependencies where they save work |
| Process model | The daemon spawns one cedia process per thread over stdio |
| Tools | Agent tools live in cedia. cedian offers host tools only for desktop-owned things (open buffers, the shared browser), never a second tool with a cedia tool's name |
| Bundling | cedian ships a pinned cedia build (`bun build --compile`), never the copy on PATH |
| Repos | `cedia` and `cedian`. cedia's protocol types are the source; cedian's Rust types follow them |
| Settings | One file, `~/.cedia/settings.json`, with a `desktop` key for cedian (below) |
| Models, auth, permission, sandbox | As cedia's SPEC: API keys only, never stops to ask, `sandbox-runtime` around bash |
| Remote | Protocol ready from day one; remote client later |
| Platform | macOS on Apple silicon; standard macOS keys |
| Tests | Replay recorded cedia JSONL logs; no model calls in tests |

## Settings

`~/.cedia/settings.json` is the single source of truth for both programs. cedia owns its schema and precedence (flags > env > user > defaults; a repo's `.cedia/settings.json` can only tighten). cedian's own keys live under `desktop` (theme, font, keymap, layout, notifications, browser). The settings UI edits that file; the daemon watches it. API keys never go in the file: cedian keeps them in the macOS Keychain and passes them to the cedia processes it spawns as environment.

## What cedia needs to add

1. `prompt.images` for pasted screenshots.
2. A `hello` handshake with version and capabilities.
3. `host_tools` registration before the first request, with `host_call` and `host_result`.
4. Per-thread `state` events streamed to the client.
5. `settings_changed`, applied at the next turn boundary.
6. Media references (video frames, original file) in tool results.
7. Unknown settings keys fail loudly instead of being ignored.

## Roadmap

| Phase | cedian | cedia | Exit gate |
| --- | --- | --- | --- |
| 0 Clean | Repo cleared | Stage 0 | Both repos clean (done) |
| A Window | GPUI app, agent pane over recorded threads; spike on Zed editor and terminal | Stage 2: tools, sandbox, ledger | A recorded thread renders; replay test green |
| B One live thread | Daemon spawns bundled cedia; terminal, review, settings UI; Keychain | `hello`, prompt images, state events | Delegate a task into a worktree, quit and reopen with the thread intact, review and accept |
| C Parallel | Many threads, tabs by state, notifications, search, shared Chromium | Stage 5: sub-agents, MCP, web, host tools | Three threads at once, memory measured |
| D Media, depth | Video and image panes, editor with LSP, git pane, surface protocol | Stage 6 plus video read | The agent fixes a bug shown in a screen recording |
| Later | Remote over QUIC, computer use via `cua-driver`, scheduled runs | Classifier, goal mode | |

## Ideas carried over from the earlier cedian

- Review against a per-task baseline, never git HEAD; a hunk the person edited after the agent is STALE and never rejected.
- Evidence is bound to code state and goes stale after a later edit.
- One browser per workspace, shared by person and agent, with captures bound to a frame.
- A normal turn has no ceremony; extra checks engage by risk.
