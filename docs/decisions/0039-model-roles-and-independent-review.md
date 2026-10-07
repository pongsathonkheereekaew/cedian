# ADR-0039: Model roles are OMP's `modelRoles`; an independent review needs a different model

- **Status:** Accepted (owner, 2026-10-07)
- **Date:** 2026-10-07
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §57–§60 (reviewers), §64 (gate floor), §77 (config ownership); [OMP_PARITY.md](../OMP_PARITY.md) "Launch options and config"; [ROADMAP.md](../ROADMAP.md) S3, S9 (settings)
- **Tightens:** ADR-0011 consequence "where OMP routing allows, a different model from the implementer"
- **Follows from:** ADR-0034 (OMP feature parity), ADR-0040 (settings mirror OMP config), ADR-0024 (evidence outcomes), ADR-0025 (pstack playbooks as OMP skills)

## Context

pstack splits work by model strength and reviews with more than one model, because a model is weakest at catching its own mistakes. cedian today has one model picker per task (S9) and a reviewer rule that only asks for a different model "where OMP routing allows".

OMP already has the whole mechanism, verified at the pin (`fc6c0c9`) and on the dev machine (OMP 18.7.0):
- `modelRoles` is a record setting: role name → model selector, with an optional thinking suffix (`openai/gpt-5.2:high`). Built-in roles include `default`, `smol`, `slow`, `plan`, `advisor`; any other name (`review`, `fast`) is a custom role (`docs/task-agent-discovery.md`).
- Agents and task spawns refer to a role as `@review`; a per-spawn `model` has the highest precedence (`docs/tools/task.md`).
- Roles live in `~/.omp/agent/config.yml`, or in `<cwd>/.omp/config.yml` when `modelRoleStorage: project`. `omp config get modelRoles --json` reads the effective record; `--model`, `--smol`, `--slow`, `--plan` set them per process (`docs/settings.md`).
- The RPC surface has `set_model`, `cycle_model`, `get_available_models` (`wire.rs`), already parity rows.

So cedian needs no role system of its own. It needs a surface for OMP's, and a rule that makes review independent.

## Decision

1. **One home for the mapping: OMP's `modelRoles`.** cedian never stores a model id. `cedian.toml` names roles only.
2. **Settings page "Model roles" (S9),** a section of the OMP settings page and bound by its rules (ADR-0040: read and write through OMP, live on `config_update`, provenance shown). It lists every effective role with its model and thinking level, and the catalog from `get_available_models`.
   - Global scope by default; project scope only where the user's `modelRoleStorage` already says `project`.
   - Verify at implementation: that `omp config set` accepts a single `modelRoles` entry. If it does not, the page is read-only plus "open in OMP `/model` Roles" until an upstream PR lands (ADR-0027: no fork).
3. **Reviewers run on a role, resolved from user config only.** A reviewer runtime is spawned through the spawn profile (ADR-0020) with `--model` set to the concrete selector of its role.
   - cedian resolves that selector with `omp config get modelRoles --json` run from a cedian-owned directory, so a workspace's `.omp/config.yml` cannot choose who reviews it (same reason as ADR-0035: a repository cannot loosen its own checks).
   - Default reviewer role: `review`. If unset, the reviewer falls back to `default` and decision 4 applies.
4. **Independence is measured, not assumed.** A review counts as independent only when the reviewer's resolved model identity differs from every model that edited the task (from `model_changed` events and the spawn record).
   - A gate that requires an independent review gets `inconclusive` (ADR-0024) from a same-model review, never `pass`.
   - The finding card and `.cedian/audit.jsonl` record both model identities.
5. **Panels by role list, in the gate floor.** `[[workflow.floor]]` may set `review_roles = ["review", "review-alt"]` (role names, never model ids). Each listed role runs one reviewer, capped by `maxReviewers` (§59). Roles that resolve to the same model count once.

## Consequences

- OMP_PARITY gains a row for `modelRoles` / `modelRoleStorage` / `--smol` `--slow` `--plan`, surfaced by the settings page (planned S9).
- S3 exit gains: a reviewer on the same model as the implementer yields `inconclusive` (hermetic replay).
- If OMP adds a provider or role, cedian shows it with no change.
- A user with one model configured still gets reviews; they just never satisfy an independence requirement. The settings page says why.
- Open: whether the implementer should also be role-driven per playbook (pstack's "code to model A, judgment to model B"). OMP agent frontmatter (`model: "@role"`) already does this inside OMP skills (ADR-0025), so cedian adds nothing until a need shows up.
