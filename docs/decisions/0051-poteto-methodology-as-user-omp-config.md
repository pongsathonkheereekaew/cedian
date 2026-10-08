# ADR-0051: poteto's methodology (pstack) runs as the user's OMP config; cedian's gates stay the judge

- **Status:** Accepted (owner, 2026-10-09, in chat: "ให้ระบบและวิธีคิดแบบ poteto มาใน cedian", then "ทำเองหมดเลยครับ" on the proposal below)
- **Date:** 2026-10-09
- **Rule text:** ROADMAP Follow-ups (reviewer prompt files); the user's OMP config, outside this repo
- **Builds on:** ADR-0025 (playbooks are OMP skills), ADR-0027 (no OMP fork), ADR-0033 (what is not adopted from pstack), ADR-0039 (model roles), ADR-0043 (reviewer profile)

## Context

cedian took pstack's mechanisms as code: evidence bound to code state (ADR-0024), the correction ledger (ADR-0032), the worker brief (ADR-0033), model roles and independent review (ADR-0039), the arena, swarm and interrogate presets (§44–§45). It left the method itself, the skills and principles an agent thinks with, to "the user's OMP skills" (ADR-0025, ADR-0033 decision 6). Nothing put them there: the owner's OMP had no pstack, so an agent under cedian meets gates that refuse unproven work without the method that teaches it to prove work.

Probe on OMP 18.6.1 (the pin), 2026-10-09, with an isolated agent dir, a fake local provider and the outgoing model request captured:

- pstack's Pi extension (port `michael-denyer/pstack-claude` 0.9.74) does not work on OMP. Its prompt hook throws (`event.systemPromptOptions.sections` is undefined on OMP), so the routing mandate never reaches the model; its child agents run `omp --skill <dir>`, a flag OMP does not have; and they would be bare spawns outside the spawn profile (ADR-0020) making their own worktrees (against ADR-0050).
- pstack's 58 skills load as plain OMP skills from `skills.customDirectories`. The model reads one through `read skill://<name>`.
- OMP appends the first `APPEND_SYSTEM.md` it finds, project dirs (`.omp`, `.claude`, `.codex`, `.gemini` under the cwd) before user dirs (`~/.omp/agent`, resolved from `$HOME`, not from `PI_CODING_AGENT_DIR`). `--append-system-prompt` on the command line replaces that lookup.
- Live turns on the owner's default model (`muse-spark-1.3-contributor`), the same two-file request with the mandate in a project `.omp/APPEND_SYSTEM.md` and without it, one run each: with it, the first and only tool call was `read skill://poteto-mode` and the plan named the Feature playbook; without it, the model read the two source files and no skill.

## Decision

1. **pstack is the user's OMP config, not cedian code.** It lives in `~/.omp/agent/`: the skills through `skills.customDirectories` pointing at a pinned copy, and the mandate as `~/.omp/agent/APPEND_SYSTEM.md`. cedian ships none of it and never writes it (ADR-0025 decision 3). The same setup serves OMP outside cedian.
2. **Skills only; the Pi extension is not used.** Subagents go through OMP's `task`, questions through OMP's `ask` or cedian's dialogs, worktrees through `cedian_worktree_request`. An OMP-specific mapping file (`~/.omp/agent/pstack/omp-tools.md`) translates the Claude Code tool names, agent types and model aliases the skills use; the mandate points at it.
3. **One home for each fact.**
   - Models: skills' aliases map to OMP roles (`fable` → `@slow`, `opus` → `@default`, `sonnet` → `@plan`, `haiku` → `@smol`); `~/.claude/pstack-models.md` does not apply on OMP (ADR-0039).
   - Done: cedian's gates decide. The mandate says a skill's verification steps produce evidence for `cedian_complete` and never replace the gate.
4. **Reviewers never get it.** A reviewer is spawned with `--no-skills` already (ADR-0043); it also gets `--append-system-prompt ""`, so neither the user's mandate nor a reviewed workspace's `APPEND_SYSTEM.md` reaches it.
5. **Pinned like OMP.** The copy in `customDirectories` is a fixed version, updated on purpose; the mapping file is checked against it on each update.
6. **Measured, not assumed** (ADR-0026). The U12 benchmark runs B1–B10 with and without the mandate and skills, and records false-done, time and review time for both.

## Consequences

- Order of rollout: the skills and the mapping file are installed (2026-10-09, `~/.omp/agent/pstack/`, `skills.customDirectories`); the mandate is staged at `~/.omp/agent/pstack/APPEND_SYSTEM.md` and goes live by copying it to `~/.omp/agent/APPEND_SYSTEM.md` only after decision 4 ships, because OMP reads that path from `$HOME` for every process, reviewers and live tests included.
- An agent under cedian routes multi-file, design and unknown-cause work through poteto-mode; small turns stay on the fast lane because the mandate exempts them. If U12 shows small turns slow down, the skill list is cut, not the gate.
- The probe found that a reviewed workspace can replace the reviewer's whole system prompt with `.omp/SYSTEM.md` (or `.claude/`, `.codex/`, `.gemini/`): the reviewer runs with the workspace as its cwd and OMP has no flag that turns `SYSTEM.md` discovery off. Decision 4 closes the append file only. The fix needs an owner ruling and is a ROADMAP follow-up.
- pstack's upstream is `cursor/plugins` (0.15.15 when ADR-0032 was written); the port pinned here is older. Moving the pin is the owner's call.
