# ADR-0052: pstack is pinned straight from Lauren Tan's upstream, with no Pi extension

- **Status:** Accepted (owner, 2026-10-09, in chat: "plugin pi extension เอาออกเลยครับ", "ใช้ pstack ล่าสุดวันนี้ครับ แล้วผมจะมาอัพเดทเรื่อยๆ", then "ตกลงครับ" on pinning from `cursor/plugins` instead of the port)
- **Date:** 2026-10-09
- **Rule text:** the user's OMP config (`~/.omp/agent/pstack/`), outside this repo
- **Supersedes in part:** ADR-0051 decision 2 (the mapping translates Cursor names, not Claude Code names) and decision 5 (the pin's source)

## Context

ADR-0051 installed pstack from `michael-denyer/pstack-claude`, a port that rewrites upstream for Claude Code. Against upstream v0.15.15 (`cursor/plugins` commit `df58112`, 2026-10-05, the newest on 2026-10-09) the port's 0.9.78 differs in 96 skill files, adds 8 skills imported from `cursor-team-kit`, lacks `make-bot-ui`, and keeps its own model panel where upstream changed it. The owner wants Lauren Tan's text as she ships it.

Facts from installing upstream on OMP 18.6.1, 2026-10-09:

- Upstream ships skills, two agent files (`poteto-agent`, `comment-sicko`) and an MIT license; it has no session-start hook and no Pi extension.
- Every upstream skill sets `disable-model-invocation: true` (Cursor runs them only when typed). OMP still serves them through `read skill://<name>`, keyed by the `name:` field. Two names differ from their folders: `poteto-mode` is `Poteto Mode`, `make-bot-ui` is `Make Bot UI`. A mandate naming `skill://poteto-mode` failed with "Unknown skill" until it named `skill://Poteto Mode`.
- Live turns on the owner's default model, the same two-file request with and without the mandate, one run each: with it, the only tool call was `read skill://Poteto Mode` (ok); without it, the model read the two source files.
- Upstream's scripts (`worktree-audit.sh`, `watch-pr`, `orch`, `check-plan.mjs`, `log.sh`) run under `node` and `bun` on this machine, and their 52 tests pass. `worktree-audit.sh` reads Cursor transcripts only, so its last-chat column is empty for OMP sessions. `orch` stack commands need Graphite (`gt`), which is not installed.

## Decision

1. **Source.** pstack comes from `cursor/plugins`, folder `pstack/`, pinned by version and commit (`~/.omp/agent/pstack/<version>-<commit>/`), skills, agents, `LICENSE` and `README.md` only. No port and no Pi extension.
2. **Updates are the owner's act.** `~/.omp/agent/pstack/update.sh [commit]` installs a version beside the old ones, points `skills.customDirectories` at it, and lists the changed files so the mapping can be checked. Upstream files are never edited.
3. **The mapping translates Cursor.** `~/.omp/agent/pstack/omp-tools.md` maps Cursor's tools (`Task`, `AskQuestion`, `subagent_type`, `readonly`, `/loop`), model slugs (to OMP roles), paths (`.cursor/rules`, `.cursor/skills`, `.cursor/projects`, `.cursor/worktrees`) and skill names to OMP. The mandate names `skill://Poteto Mode`.

## Consequences

- An upstream rename of a skill's `name:` breaks the mandate or the mapping silently; `update.sh`'s file list is where to catch it.
- ADR-0051's other decisions hold: user config only, cedian's gates decide done, reviewers get no mandate, and U12 measures with and without.
