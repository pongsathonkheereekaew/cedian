# ADR-0040: The OMP settings page mirrors OMP's config; OMP's CLI and cedian edit the same state

- **Status:** Proposed
- **Date:** 2026-10-07
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §77 (config ownership); [OMP_PARITY.md](../OMP_PARITY.md) "Launch options and config", `config_update`, `config_warnings_changed`, `open_session`; [ROADMAP.md](../ROADMAP.md) S9 (settings)
- **Follows from:** ADR-0034 (OMP feature parity), ADR-0018 (`cedian.toml` holds cedian settings only), ADR-0020 and ADR-0035 (spawn overlay and its opt-out)

## Context

cedian is meant to replace OMP's CLI as the place the owner works. That holds only if a change made in one is the same change in the other: a setting changed in the OMP TUI shows in cedian, and a setting changed in cedian is what the TUI then reads.

OMP already gives cedian what it needs, verified at the pin (`fc6c0c9`, `docs/settings.md`, `docs/config-usage.md`) and on OMP 18.7.0:
- Layers, lowest to highest: defaults ← global `~/.omp/agent/config.yml` ← project `<cwd>/.omp/config.yml` ← `--config` overlays ← runtime overrides ← setting env vars.
- `omp config get <key> --json` reads the effective value; `omp config set` writes the global file and reports when a higher layer still wins (`overriddenBy`).
- RPC hosts watch the global file, project sources and overlays, and reload on change. The agent event `config_update` reaches cedian over RPC.
- Sessions live in OMP's session store; `open_session` resumes any of them, whichever surface started it.

Two things cedian adds on purpose break a naive mirror: the spawn overlay (ADR-0020) sits above the user's config, and review, evidence and gates exist only in cedian.

## Decision

1. **OMP owns the state; cedian renders and requests changes.** Every OMP setting shown in cedian is read from OMP (`omp config get --json`, or RPC where it exists) and written through OMP (`omp config set` / `reset`, or RPC). cedian never edits OMP's YAML itself and never copies an OMP setting into `cedian.toml` (§77).
2. **Live in both directions.** On `config_update`, the settings page and every surface that shows an OMP setting re-read it. No restart and no cedian-side cache that outlives the event.
3. **Provenance is always shown.** Each setting shows the layer that supplies its effective value. When a cedian write is shadowed (`overriddenBy`), the page says by what, rather than looking saved.
4. **The spawn overlay is a visible layer, not a hidden override.** Keys the overlay pins (`tools.approvalMode`, the exec-tool prompt pins, the eval gate, `computer.enabled`) show as "set by cedian for this project", with a link to the `policy = "omp"` opt-in (ADR-0035). Editing them in the page edits the user's OMP config and says it takes effect in cedian only under that opt-in.
5. **Sessions are shared, not co-driven.** A session started in the CLI opens in cedian and the reverse, through `open_session`. One session is driven by one process at a time; cedian refuses to open a session another live process holds. Verify at S9 whether OMP exposes a lock to detect this; if it does not, cedian warns instead of refusing and the gap goes to OMP upstream (ADR-0027).
6. **cedian-only state stays in cedian.** Review decisions, evidence, gates, agent transactions and the audit log have no OMP equivalent and are not mirrored. Edits made by an OMP CLI session are not imported as agent transactions; cedian sees them as disk changes (STALE rules, §18).

## Consequences

- The S9 settings page has one exit check per decision: CLI `omp config set` → cedian shows the new value without restart; cedian write → `omp config get` returns it; an overlay-pinned key shows its badge; a session started in the CLI resumes in cedian.
- ADR-0039's "Model roles" page is one section of this page and follows these rules.
- Parity is not done when a row has a widget; it is done when the widget passes decisions 2 and 3 for that row.
- Settings that are pure TUI presentation (TUI theme, splash, TUI keybindings) are listed read-only with their value; cedian's own presentation settings stay in `cedian.toml` (ADR-0018).
