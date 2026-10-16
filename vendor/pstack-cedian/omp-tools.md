# pstack in cedian: mapping from Cursor

cedian bundles pstack from Lauren Tan's upstream (`cursor/plugins`, folder `pstack/`), written for Cursor. Here the skills are the same files; this file names the OMP equivalent of each Cursor tool, agent type, model, path and command they use. Read the row before following a skill step that names one.

## Tools

| Cursor | OMP |
|---|---|
| a skill, `/name` | `read skill://<name>`, where `<name>` is the skill's `name:` field; interactively also `/skill:<name>`. Two differ from their folder: `poteto-mode` is `skill://Poteto Mode`, `make-bot-ui` is `skill://Make Bot UI` |
| a skill with `disable-model-invocation: true` (every pstack skill except `setup-pstack`) | OMP honors the field: the skill loads and resolves through `skill://`, but is left out of the model's advertised skill list, so entering it means `read skill://<name>` or an interactive `/skill:<name>`; Cursor's bare `/name` form does not exist. cedian's pstack instructions are what tell the model to read `skill://Poteto Mode` by name |
| `Read`, `Write`, `StrReplace`, `Grep`, `Glob`, `Shell` | `read`, `write`, `edit`, `grep`, `glob`, `bash` |
| `Task` (one subagent) | `task` with one item in `tasks[]` |
| several `Task` calls with `run_in_background: true` | one `task` call with several items; IDs return at once and results arrive on their own; call `wait` only when blocked with nothing else to do |
| `subagent_type: "poteto-agent"` | a `task` item with `agent: "poteto-agent"` when the task tool lists it; otherwise agent `task`, told to read `skill://Poteto Mode` and pstack's `agents/poteto-agent.md` first |
| `subagent_type: "comment-sicko"` | a `task` item with `agent: "comment-sicko"` when listed; otherwise agent `task`, told to read pstack's `agents/comment-sicko.md` first |
| `subagent_type: "generalPurpose"` or none | agent `task` |
| `readonly: true` | for search and exploration only: agent `scout`, which runs on `@smol`. A design runner, arena or architect candidate, judge or panel member is never a `scout`: use a `panel-*` agent from the Models section |
| `readonly: false` (agent mode) | agent `task` |
| resume or message a running subagent | `write agent://<id>` |
| `AskQuestion` | the `ask` tool when offered; otherwise ask in plain text and stop |
| todolist (`TodoWrite`) | `todo` |
| `/loop`, `/loop <interval>`, `/loop until X` (autonomous-run, autopilot, multi-phase-plan audit tick) | none yet. Under cedian the host tool `cedian_loop` (cedian ADR-0072, slice S7) arms a loop that wakes this session, with no confirmation; use it when the session lists it. Until then keep the run in this turn: an event to watch gets a background `task` running `watch-pr` and `wait`, and say that the heartbeat and later ticks need a loop |
| Bugbot (Cursor's PR review bot) | none. Treat other review bots' comments the way the Bugbot reference says; under cedian, review findings come from cedian's reviewer |

`<active>` is pstack's folder, named in cedian's pstack instructions (the folder above `skills/`).

## Models

Skills name Cursor model slugs. OMP 18.10 has no per-item `model` on `task` (removed after 18.5): a subagent runs on its agent's role. So pick the agent whose role fits; the user's `modelRoles` decide what each role runs. `task` (the general agent) and `poteto-agent` run on `@default`, `scout` on `@smol`, `reviewer` on `@slow`, and `panel-slow`, `panel-plan`, `panel-default` (full tools, in cedian's pstack folder `agents/`) on the role they name, when the task tool lists them; when it does not, use agent `task` and say the panel ran on one role.

| Cursor slug or role in a skill | OMP role, and the agent that runs on it |
|---|---|
| strongest judgment, hardest changes, prose and judgment (`claude-opus-*`) | `@slow`: agent `panel-slow`, or `reviewer` read-only |
| code (`grok-*-fast`), most delegates | `@default`: agent `task` or `poteto-agent` |
| trivial mechanical edits, fast code model | `@smol`: agent `scout` (read-only) |
| planning | `@plan`: agent `panel-plan` |
| `inherit-parent`, `auto`, no model | agent `task` |
| a reasoning level for one spawn (`-low` … `-max` on a slug, "raise it one step", "lower it two steps") | the `task` item's `effort`: `lo` for low and below, `med` for medium, `hi` for high and above. Without it OMP picks the level from `solutionSpace`. `task.maxEffort` caps it |
| fast mode (`-fast` on a slug) | no per-spawn switch; keep the role and say so when the skill asks for fast |

A diverse panel (`arena`, `architect`, `interrogate`, `how`, `reflect`, `why`) gives each member a different agent: `panel-slow`, `panel-plan`, `panel-default` in that order, then repeat; a cross-judge is `panel-slow`. When the roles resolve to the same model, say in the verdict that diversity was reduced. Under cedian, an independent review is cedian's reviewer on the `review` role (cedian ADR-0039), not a panel member.

`/setup-pstack` and `~/.cursor/rules/pstack-models.mdc` do not apply in cedian. The person picks pstack's role models in cedian's Settings > Methodology, which writes the override sheet named in cedian's pstack instructions, in the shape `/setup-pstack` writes. Where a skill reads a role line from its sheet, read that file: a line's value is a model (`provider/model`) or an agent role (`@slow`), optionally with an effort (`@high`). An agent role picks the `panel-*` agent on that role; a model picks the agent whose role runs that model, and when none does, say so and use the nearest role. A role with no line uses the table defaults above. `default effort` applies to a role line without one; `session` keeps the session's level.

## Paths

| Cursor | OMP |
|---|---|
| `.cursor/rules/`, the instructions file | `AGENTS.md` in the project and its parents; user level the agent's config folder (`~/.config/cedian/agent/`) |
| project skills `.cursor/skills/<name>/` (for example `verify`) | `.omp/skills/<name>/`; a project verification skill is `.omp/skills/verify-<app>/` (cedian ADR-0025) |
| user skills `~/.cursor/skills/`, plugin skills `~/.cursor/plugins/` | `~/.config/cedian/agent/skills/`; pstack's own skills come from cedian's bundle |
| transcripts `~/.cursor/projects/<slug>/agent-transcripts/` | the agent's sessions `~/.local/state/cedian/agent/sessions/<dir>/<iso>_<id>.jsonl`, where `<dir>` is the cwd relative to home with each `/` as `-` (for `~/cedian`, `-cedian`). Read only this workspace's directory |
| `.cursor/worktrees/` | under cedian, worktrees come from `cedian_worktree_request` with the worker brief (cedian ADR-0033, ADR-0050); OMP's own task isolation is off |

## Scripts

Scripts live under `<active>/skills/<skill>/scripts/` and run through `bash` with `node` or `bun` (`watch-pr` installs its own packages on first run).

- `poteto-mode/scripts/worktree-audit.sh` works; its `LAST_CHAT` column reads Cursor transcripts only, so it stays `-` for OMP sessions.
- `poteto-mode/scripts/watch-pr/watch-pr`, `orch/orch.ts`, `check-plan.mjs` and `show-me-your-work/scripts/log.sh` work as written.
- `check-plan.mjs` checks pstack's own plan skeleton; a project whose plans use another shape (cedian's `docs/plans/`) fails it by design.
