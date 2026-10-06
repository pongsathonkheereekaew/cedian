# ADR-0025: Playbooks and project verification profiles are OMP skills

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §46, §49, §77
- **Amends (consequences of):** ADR-0010

## Context

Under A1-narrow (ADR-0010) OMP owns playbooks and phase progression. The plan assumed this meant TypeScript in an OMP fork (`packages/coding-agent/src/workflow/`), which the owner wants to keep minimal (ADR-0023).

pstack (`cursor/plugins`, studied 2026-10-06) delivers 23 playbooks, principles and a per-project verification recipe entirely as skill files: markdown the agent loads, no runtime code. OMP already has skills: `skill://` is a reserved built-in URI scheme, there is a `--no-skills` flag, and skills are listed under `.omp/skills/` in §77.

pstack's `create-verification-skill` writes `verify-<app>/` with:
- fixed sections: Launch, Doctor, Drive, Evidence, Cleanup
- a feature map: one entry per user-facing feature, naming the result that proves it works

Its generator proves the skill once end to end before anyone uses it.

## Decision

1. **Playbooks are OMP skills.**
   - The five V1 playbooks (§49) are skills in `.omp/skills/`, not TypeScript and not cedian code.
   - Each tells the agent when to call `cedian_workflow_update` / `cedian_complete` (ADR-0022).
   - The number stays small (§88): no 23-playbook library.
2. **Project verification profile.**
   - Each project may have a `verify-<app>` OMP skill with the five sections and a feature map.
   - Gates may require evidence for a feature-map id ("feature `bulk-archive` proven").
   - cedian supplies the drivers the profile uses: the browser/CDP session, the shell PTY, plain HTTP, and later the simulator. cedian never runs the profile itself (§54).
3. **Ownership.** Skills are the user's OMP config (§77). cedian never writes them silently. It may offer to generate a profile through an OMP turn, which the user reviews like any other change.

## Consequences

- S2 needs no OMP fork: skills + host tools cover the OMP side of A1-narrow.
- The ROADMAP "OMP-side work" list stays at two items (native edit routing, Plan mode).
- Methodology content (principles, writing style, model routing) also belongs in OMP skills. It is never a cedian feature.
- **Superseded in part by ADR-0027:** there is no OMP-side work list any more — OMP is never forked.
