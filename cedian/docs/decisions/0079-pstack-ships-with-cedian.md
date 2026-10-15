# ADR-0079: pstack ships with cedian, on by default, with its own settings page

- **Status:** Accepted (owner, 2026-10-15, in chat: "Pstack จะฝังเข้าไป cedian as default for option to eneble/disable ได้เลยไหม", then "ตกลงครับ" on the proposal below, and asked for a settings page to set it up and choose models)
- **Date:** 2026-10-15
- **Rule text:** `cedian.toml` (`pstack`), the agent settings page; `vendor/pstack/`
- **Supersedes in part:** ADR-0051 decision 1 (pstack as the user's OMP config only) and ADR-0052 decisions 1 and 2 (where the pinned copy lives and how it updates)
- **Builds on:** ADR-0078 decision 5 (the agent keeps cedian's own storage, never `~/.omp`)

## Context

Since ADR-0051 and ADR-0052, pstack lived in the owner's `~/.omp/agent/`: Lauren Tan's skills from `cursor/plugins` (0.15.24, `1e5c90a`), the Cursor-to-OMP mapping `omp-tools.md`, the poteto-mode mandate `APPEND_SYSTEM.md`, and the panel agents. cedian's agent read it because it shared `~/.omp`. ADR-0078 moves the agent to cedian's own directories, so the agent would lose pstack, and the import command copies only plain skills.

pstack is MIT-licensed (copyright Lauren Tan), under 1 MB, 54 skills.

## Decision

1. **pstack is bundled.** The pinned upstream copy lives in `vendor/pstack/` (skills, agents, `LICENSE`, `README.md`, the commit it came from) and ships inside the app next to the agent. Upstream files are never edited.
2. **cedian's adaptations are separate.** The mapping file, the mandate and the panel agents live in a cedian-owned folder beside it, with their history in this repository.
3. **On by default, with a switch.** `pstack = true` is the default. The switch lives in the user's `cedian.toml`, optionally per project, never in a file a repository carries, so a repository cannot turn it on or off (as ADR-0035 keeps `policy = "omp"`). Off means the agent runs as plain OMP: no mandate and no pstack skills.
4. **A settings page sets it up.** It has:
   - the switch;
   - the mandate on or off;
   - a model for each role: feature and refactoring, bug fix, judgment and prose, strongest judgment, and the multi-model panels (arena, architect, interrogate), each picked from the models the agent lists;
   - the default reasoning effort;
   - the pinned version, with a check for a newer upstream version.

   The page writes the overrides the pstack skills already read, so no upstream file changes.
5. **Updates go through a PR.** A script fetches a newer upstream version into `vendor/pstack/`, lists the changed files against the mapping, and opens a PR the owner merges. Nothing updates silently.
6. **What ADR-0051 kept still holds:**
   - cedian's gates decide when work is done;
   - reviewers never get the mandate or the skills;
   - the behavior is measured, not assumed.
7. **Credits.** About and the licenses list name pstack by Lauren Tan (MIT).

## Consequences

- poteto-mode works in cedian from the first launch, with no setup.
- The owner's own OMP outside cedian keeps its own pstack copy; the two are independent.
- A newer pstack reaches cedian only when the owner merges the update PR, so behavior never changes unnoticed.
- Upstream renaming a skill's `name:` breaks the mandate or the mapping. The update script's file list is where that is caught, as before.
