# ADR-0083: pstack reaches the agent through the spawn profile, and its settings live in cedian's storage

- **Status:** Accepted (agent's call within ADR-0079 and owner ruling 35, S10 SET-3, 2026-10-16; reported to the owner in the SET-3 PR)
- **Date:** 2026-10-16
- **Rule text:** `crates/cedian_omp/src/spawn_profile.rs` (`PstackProfile`), `crates/cedian_shell/src/pstack.rs`, `crates/cedian_shell/src/launch.rs` (`pstack_profile`), `vendor/pstack/`, `vendor/pstack-cedian/`
- **Builds on:** ADR-0079 (pstack ships with cedian), ADR-0020 (the spawn profile), ADR-0078 decision 5 (cedian's own agent storage)

## Context

ADR-0079 bundles pstack and gives it a settings page, but leaves open how the bundle reaches the agent, where the switches and the model choices are stored, and what happens when the bundle is missing. The agent finds skills through `skills.customDirectories`, takes an extra system prompt from `--append-system-prompt` (a file path or text, which replaces its `APPEND_SYSTEM.md` lookup), and finds task agents only in its config dir, a project's `.omp/agents` and extension packages.

## Decision

1. **Two folders.** `vendor/pstack/` is upstream `cursor/plugins` `pstack/` at 0.15.24 (`1e5c90a`): `skills/`, `agents/`, `LICENSE`, `README.md` and `COMMIT`, byte-identical to the upstream tree. `vendor/pstack-cedian/` is cedian's: `mandate.md` (a template), `omp-tools.md`, the panel agents and `pin.json` (version, commit, author, license). The app bundle carries them as `Contents/Resources/pstack{,-cedian}`.
2. **The spawn profile carries it.** With pstack on, a person's session gets `skills.customDirectories: [<bundle>/pstack/skills]` in the overlay and, unless the mandate is off, the mandate rendered with this machine's paths, written beside the overlay as `cedian-pstack.md` and passed with `--append-system-prompt`. A reviewer's profile with pstack is refused at plan time, so a reviewer never gets the skills or the mandate.
3. **Switches in `cedian.toml`.** `pstack` and `pstack_mandate`, both default `true`; per project `[projects."<path>"] pstack`, read only from the user's file, with the ADR-0035 key checks. A file inside the workspace never switches it. The Methodology page writes the two global keys through `toml_edit`, keeping the rest of the file, and never writes a file that would not parse.
4. **The override sheet.** `<agent config dir>/pstack-models.md` (`~/.config/cedian/agent/pstack-models.md` by default), in the shape `/setup-pstack` writes: one `<role>: <model or @role> [@effort]` line per role that is not on the agent's default, and `default effort: <level|session>`. Methodology replaces it in one rename; the mandate names its path, and `omp-tools.md` says how to read it.
5. **A missing bundle refuses the spawn** while pstack is on, naming the switch that turns it off. A debug build also looks in the source tree it was built from, as `CEDIAN_OMP_BINARY` is honored only in debug builds.

## Consequences

- While pstack is on, the overlay's `skills.customDirectories` replaces the person's own list (their skills folder in the agent dir still loads).
- `--append-system-prompt` turns off the agent's own `APPEND_SYSTEM.md` lookup, so while the mandate is on cedian appends the text itself, after the mandate: the project's file (the first of `.omp`, `.claude`, `.codex`, `.gemini` under the workspace), then the agent dir's. Without the mandate the agent reads only one of them, as before.
- The panel agents (`panel-slow`, `panel-plan`, `panel-default`, `poteto-agent`, `comment-sicko`) ship in `vendor/pstack-cedian/agents/` but the agent does not load them yet: no launch option adds a task-agent folder, and proving the extension-package route needs a live turn. `omp-tools.md` tells the model to use agent `task` when they are not listed. Wiring them is a follow-up.
- The update script ADR-0079 decision 5 describes, the panel model lists and the version check are U9.
