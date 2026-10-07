# Row E — settings move to `cedian.toml`

ROADMAP stand-in row E ends here. The headless settings file stops being `<workdir>/cedian.json` and becomes the user's `cedian.toml`, the one settings file of ADR-0018. The gate floor (ADR-0010, ADR-0026) moves from code into that file. P8 (ADR-0035) then adds `[projects."<path>"]` to the same file without reshaping anything.

**Exit (observable checks):**
- `cedian.toml` with a `schema` key is the only settings file. A missing `schema`, a wrong `schema`, an unknown key at any depth, or a bad floor gate fails the command (exit 2) with the reason.
- A leftover `<workdir>/cedian.json` is refused with an error that names the exact move (the file found, the user `cedian.toml` path, `schema = 1`). A `<workdir>/cedian.toml` is refused the same way, citing §77, unless it is the user file itself.
- Permissions behave as before (strict-wins, ADR-0012): the same `[permissions]` tiers map to the same spawn policy and host-tool set.
- `[[workflow.floor]]` rules (`kind`, `min_risk`, `gates`) load into `cedian_workflow::GateFloor` and reach `WorkflowChannel::with_policy`. No floor means an empty floor (fast lane).
- `grep -rn cedian.json` (outside `docs/archive/` and this plan) finds only the migration message and its test.

**Timebox:** 1 working session. **Partial exit:** E1–E3 (parse, location, CLI wiring) are the row; E4 (floor through an OMP turn) and E5 (docs) follow. Commit each green unit. If time runs out, README names the open unit and P8 waits for E3 only.

## Design (architect, 2026-10-07)

Arena of three runners (opus, opus, sonnet; fable had no credits) plus a sonnet cross-judge. All three put the file at user level and refuse workspace files. The judge preferred candidate 2's resolve-once shape; candidate 1's smaller surface won the type names. Synthesis:

- **Location.** `$CEDIAN_CONFIG` if set, else `$XDG_CONFIG_HOME/cedian/cedian.toml`, else `~/.config/cedian/cedian.toml`. An explicitly set `CEDIAN_CONFIG` that names a missing file is an error (a typo must not silently mean defaults). The implicit path missing means defaults. Nothing in a workspace is ever read for content: ADR-0035's "a repository cannot opt itself in" holds by construction, for every key, not just P8's.
- **Pure core, thin shell.** `cedian_shell::parse_settings(&str) -> Result<Settings, SettingsError>` is pure (TOML → private wire structs with `deny_unknown_fields` at every level → schema check → floor gates through `GateSpec::build`). `cedian_shell::resolve_settings(workdir) -> Result<Settings, SettingsError>` is the only function that reads env and disk (locate, tripwires, read, parse).
- **Load once.** `main.rs` resolves once after canonicalizing the workdir and passes `&Settings` to the prompt gate, `spawn` and `host_tools`. The three re-reads of `load_workdir_settings` go away, so one command can't run two policies.
- **One way to build a floor-style gate.** `cedian_workflow::GateSpec {id, gate_kind, evidence_kinds, min_items, feature}` is the `op=gate` shape. `GateSpec::build()` is the single place that sets `required: true` and `require_ok`/`fresh = kind != reproduction` and calls `Gate::register`. `channel.rs` `op=gate` and the settings loader both call it. `GateFloor`/`FloorRule` drop `Deserialize` (nothing deserializes them; the file goes through `FloorRuleSpec`). `Gate` keeps `Deserialize` because `workflow.json` persists playbook gates (candidate 2 proposed dropping it; that would break the store).
- **Schema.** `SETTINGS_SCHEMA` stays 1 and becomes required. The TOML file is new, the shape is unchanged, and the floor table is additive. P3's rule holds: mismatch fails closed.
- **`update_channel`** becomes an enum (`stable|beta`), so a bad channel is a parse error, not a separate check.
- **Hermetic tests.** Every replay spawn sets `CEDIAN_CONFIG` to a file inside its tempdir through one helper. No debug-only guard (judged too clever; the helper is the one path).

Rejected: a per-workspace `cedian.toml` (P8 would need a second file, breaking ADR-0018); a layered user + workspace merge (the floor and `[projects]` have no obvious "stricter", so every key would need a merge rule); `CEDIAN_CONFIG=none` (a magic string; a missing explicit file is an error instead).

Accepted tradeoff: a repository can no longer ship its own permission tiers. Per-project tightening can come back as keys under `[projects."<path>"]` in the user file.

## Units (dependency order)

| # | Unit | Check (done when) |
|---|---|---|
| E1 | `GateSpec` + `FloorRuleSpec` + `GateFloor::from_specs` in `cedian_workflow`; `op=gate` calls `GateSpec::build`; `GateFloor`/`FloorRule` drop `Deserialize` | `cargo test -p cedian_workflow`: existing `op=gate` tests unchanged and green; new test: a spec builds the same `Gate` `op=gate` builds; a non-fresh non-reproduction spec is impossible (fields absent) |
| E2 | `cedian_shell` settings: TOML parse (`toml` crate), private wire types, `deny_unknown_fields`, required `schema`, `UpdateChannel` enum, floor, `default_settings_toml` emits TOML; `resolve_settings` with location + tripwires | `cargo test -p cedian_shell`: missing schema, schema 2, unknown nested key (`[permissions] dangerus`), bad floor gate, `cedian.json` tripwire message, workspace `cedian.toml` tripwire, explicit missing `CEDIAN_CONFIG` → error, implicit missing → defaults, default document round-trips |
| E3 | CLI: resolve once, pass `&Settings` to prompt gate / `spawn` / `host_tools`; floor from settings into `with_policy`; replay harness sets `CEDIAN_CONFIG` | `cargo test -p cedian_cli` (unit + every `replay_cli` scenario) green; new CLI test: a `<workdir>/cedian.json` makes `cedian review` exit 2 naming the move |
| E4 | Floor reaches the channel: hermetic check that a floor from `cedian.toml` adds a required gate an OMP `op=start` workflow cannot drop | channel-level test through `resolve_settings` → `with_policy`, or a CLI test with `cedian workflow` verbs; replaying an existing recorded turn with a floor set shows the floor gate in `workflow.json` |
| E5 | Docs: README Build/test (where settings live), every `cedian.json` reference, `floor.rs` and settings module docs, README slice rows that row E affects | `grep -rn cedian.json` shows only the migration message and its test; README S2 row drops "gate-floor config waits for row E" |

## Findings

(filled in as units land)
