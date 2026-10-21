# cedian architecture

Decided 2026-10-21 in a planning discussion with the owner. Open questions are listed at the end.

## What cedian is

Cedia's native agentic tools for one person. Coding is the core; the dev tools around it (browser, docs, GitHub, databases) come next, reached through native tools or MCP. The main way of working is to delegate a task and review the result, then to run several tasks in parallel, one worktree each.

## Shape

```text
 window (GPUI)            window (later: phone, other Mac)
      │  subscribe / command      │
      └──────────┬────────────────┘
                 │ cedian protocol: Unix socket now, QUIC later
          ┌──────▼──────┐
          │   daemon    │  sessions, agent loop, tools, terminals,
          │             │  browser, worktrees
          └──────┬──────┘
                 │
          SQLite event log (one append-only log per session)
```

- One binary, headed or headless. Headed with no daemon running starts the daemon in-process and still serves it on the socket, so another window can attach.
- The window draws panes and tabs only. Pane kinds: agent, terminal, file (text, image, video), editor, diff review, browser. No agent sidebar: the agent is a pane of blocks (text, tool call, diff, image, video, screenshot).
- One `open(target, how)` routes any file or URL to the right pane kind.
- Agent states (idle, working, waiting for you, done) show on tabs.

## Decisions

| # | Topic | Decision |
| --- | --- | --- |
| 1 | Why | Control, performance, differentiation, craft |
| 2 | Users | The owner only. No onboarding, auth or team features |
| 3 | Scope | Coding core plus surrounding dev tools |
| 4 | Working mode | Delegate first, then parallel worktrees |
| 5 | Surface | Daemon plus pane-based window |
| 6 | Editor | Syntax highlighting, multi-cursor, LSP. Not a full IDE |
| 7 | Remote | Protocol ready from day one; remote client later |
| 8 | Ownership | Own stack end to end; no outside agent harness |
| 9 | Language | Rust |
| 10 | UI | GPUI as a library; Zed's `editor`, `language`, `lsp`, `terminal` crates as pinned git dependencies where they save work |
| 11 | Agent loop | Own loop in Rust, calling model APIs directly; tools are native Rust |
| 12 | State | Append-only event log per session in SQLite; clients subscribe |

## Ideas carried over from the earlier cedian

- Review against a per-task baseline, never git HEAD. Accept keeps the text, reject applies the inverse patch, and a hunk the person edited after the agent is STALE and never rejected.
- Evidence is bound to code state and goes stale after a later edit; "done" needs its claims checked.
- Permissions are strict-wins (Deny > Ask > Allow), enforced by the OS sandbox, with protected paths such as `.git`.
- Computer use goes only through the CUA driver, off by default until its sandbox lands.
- One browser per workspace, shared by person and agent, with captures bound to a frame.
- A normal turn has no ceremony; extra checks engage by risk.
- An independent reviewer runs on a different model.

## Open questions

- Model providers first, and API key or subscription sign-in.
- Default safety: free writes inside a task's worktree and ask outside it, or ask on every write.
- MCP as client only, or also as a server for cedian's own tools.
- The v1 tool set: read, search, edit, bash, git, browser, image read, video read.
