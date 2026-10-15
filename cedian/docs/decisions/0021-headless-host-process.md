# ADR-0021: Headless work runs in one long-lived `cedian shell` process

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) chapter 4 (process note); [ROADMAP.md](../ROADMAP.md) prerequisites

## Context

The CLI harness is one process per command: every command spawns its own OMP runtime, Chrome or rust-analyzer and exits. That shape caused most of the S0–S5 gaps found on 2026-10-06:

- accept/reject state was lost between commands
- `browser shot` re-opens a fresh Chrome, so frame sequence numbers are meaningless
- `worker steer` can only write a note, because there is no live session to steer
- rust-analyzer pays a cold start on every `diagnostics`

The upcoming slices need live state even more: a running OMP session for steer and abort (S5), reviewer subagent sessions (S3), and a process that stays up for the scheduler (S7). The S9 app will be exactly one long-lived process holding all of this, plus GPUI.

## Decision

1. **`cedian shell`** is a foreground, interactive process for one workspace. It holds the `OmpRuntime`, `HostTools`, `Panel`, `ReviewTracker`, one long-lived browser session, the LSP bridge, and worker sessions. It reads commands from stdin using the same verbs as the one-shot CLI and prints events as they stream.
2. **Shell-only commands** are those that need live state: prompt follow-ups/steer/abort, browser actions that produce evidence, worker steer, warm diagnostics, and the automation scheduler (S7).
3. **One-shot commands stay** for store-backed or stateless operations: `review`, `accept`/`reject`, `workflow status`, `worker list`/`preview`/`merge-back`/`remove`, `palette`. They read and write the same `.cedian/*.json` stores, using atomic writes and `snapshot_version`.
4. **Coordination.** The shell holds `.cedian/shell.lock` (pid + start time). Mutating one-shot commands refuse while a live shell holds it and tell the user to run the command inside the shell. A stale lock (dead pid) is reclaimed.
5. **Not a daemon.** The shell dies with its terminal: no IPC server, no launchd, no surviving quit or reboot. The daemon track in ADR-0019 is untouched.

## Consequences

- The headless harness gets the S9 process shape early, so the GPUI binding later replaces stdin/stdout rather than redesigning lifecycles.
- Unblocks real exits for S4 (frame seq within a session), S5 (steer reaches a live session) and S7 (scheduler runs while the shell is open and the machine is awake).
- The one-shot `prompt` stays for scripting, but it can never satisfy an exit that needs state held across commands.
- Cost: a small REPL loop plus the lock. No new crates; the wiring lives in `cedian_cli` and dies at S9 like the rest of it.
