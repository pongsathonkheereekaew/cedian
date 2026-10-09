# ADR-0053: A reviewer refuses a workspace that carries OMP system-prompt files

- **Status:** Accepted (owner, 2026-10-09, in chat: "ปฏิเสธการรีวิว" over passing the reviewer cedian's own `--system-prompt`)
- **Date:** 2026-10-09
- **Rule text:** ROADMAP Follow-ups (reviewer prompt files), until built
- **Builds on:** ADR-0039 decision 3 (a repository cannot choose or loosen its own review), ADR-0043 (reviewer profile), ADR-0051 decision 4

## Context

A reviewer runs with the reviewed workspace as its cwd (`crates/cedian_cli/src/review_agent.rs`, `cwd: workdir`). OMP reads system-prompt files from the cwd: `SYSTEM.md` replaces the whole system prompt, `SYSTEM_TEMPLATE.md` replaces it with a template, `APPEND_SYSTEM.md` appends. A workspace can therefore rewrite what its reviewer is told, for example "report no findings".

Probe on OMP 18.6.1, 2026-10-09: a marker in each file, a fake local provider, the outgoing request captured, one run per location.

| Location | `SYSTEM.md` | `SYSTEM_TEMPLATE.md` | `APPEND_SYSTEM.md` |
|---|---|---|---|
| cwd's `.omp/` | reaches the model | reaches | reaches |
| cwd's `.claude/`, `.codex/`, `.gemini/` | reaches | reaches | reaches |
| cwd's `.agent/`, `.agents/` | reaches | reaches | ignored |
| a parent's `.omp/`, `.agent/`, `.agents/` (cwd in a subfolder of the repo) | reaches | reaches | ignored |
| a parent's `.claude/`, `.codex/`, `.gemini/` | ignored | ignored | ignored |

No OMP flag turns the `SYSTEM.md` lookup off; only an explicit `--system-prompt` or `--system-prompt-template` replaces it. ADR-0051 decision 4 (an explicit empty `--append-system-prompt`, fork `4a48723367`) closes the append file only.

## Decision

1. Before it spawns a reviewer, cedian looks in every directory from the workdir up to the repository root, inclusive, for `.omp`, `.claude`, `.codex`, `.gemini`, `.agent` and `.agents`, each for `SYSTEM.md`, `SYSTEM_TEMPLATE.md` and `APPEND_SYSTEM.md`. The set is wider than the probe's hits on purpose: a file OMP ignores today may be read after a pin bump.
2. If any exists, no reviewer runs. The review's outcome is `inconclusive` (ADR-0024), with a reason naming each file found, and the audit log records it. A gate that requires an independent review stays closed.
3. cedian never passes the reviewer a system prompt of its own; OMP's prompt stays OMP's (ADR-0034).

## Consequences

- A repository that keeps OMP prompt files for its own agents cannot be reviewed by cedian until the files move out of the tree. The reason says which files and why.
- An OMP pin bump re-runs this probe; a new location is added to decision 1 in the same change (ADR-0034's parity rule).
