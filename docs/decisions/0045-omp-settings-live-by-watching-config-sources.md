# ADR-0045: The OMP settings page stays live by watching OMP's config files; sources are derived through OMP

- **Status:** Accepted (owner, 2026-10-08, in chat: "เฝ้าไฟล์ config" and "ทุกคีย์ + แก้แบบง่าย")
- **Date:** 2026-10-08
- **Rule text:** `docs/plans/s9-real-app.md` U3a
- **Refines:** ADR-0040 decisions 2 and 3

## Context

ADR-0040 decision 2 has the settings page re-read on `config_update`. Probes on OMP 18.6.1 (the pin) on 2026-10-07 show that cannot work:

- `config_update` carries only `model` and `thinkingLevel` (`ConfigUpdateEvent` in the vendored `wire.rs`).
- An OMP RPC process emitted no frame of any kind when its project `.omp/config.yml` changed twice.

ADR-0040 decision 3 asks for the layer behind every value. OMP does not report it:

- `omp config list --json` gives 491 keys with their effective values, types and descriptions, but no layer.
- `omp config get` gives the effective value only.
- `omp config set` writes the global file and reports `overriddenBy` (for example `"project"`) when a higher layer wins.
- A dotted record entry such as `modelRoles.review` is an unknown setting. The whole record can be set as JSON.

OMP reads its state from `PI_CODING_AGENT_DIR`, or `OMP_PROFILE`'s profile, or `~/.omp/agent`. cedian's scrubbed env dropped both variables, so a user who runs OMP under a profile would see a different config in cedian than in the CLI.

## Decision

1. **Live by watching what OMP reads.** cedian watches, read-only, the global `config.yml` in the directory `omp config path` reports, and the workspace's `.omp/config.yml`. On any change, and on `config_update`, it re-reads every value through `omp config list --json`. It never parses OMP's YAML.
2. **Sources are derived through OMP.** For each key cedian compares three reads:
   - in the workspace (effective);
   - in an empty directory (global and defaults);
   - with an empty agent directory (defaults only).
   The layer is `project` when the first two differ, `global` when the last two differ, and `default` otherwise. Keys the spawn overlay pins show `set by cedian` (ADR-0040 decision 4). A value from a setting env var or another `--config` overlay shows as `project` or `global`, whichever read first carries it; the page says layers are derived.
3. **Writes go through `omp config set` and `reset`.** A shadowed write shows `overriddenBy`. Booleans, enums, numbers and strings are editable in place. `modelRoles` has its own section, where one role is edited by reading the record, changing that entry and writing the whole record back. Other records and arrays are read-only for now.
4. **cedian honours OMP's own state selection.** `PI_CODING_AGENT_DIR` and `OMP_PROFILE` pass through the spawn profile's env allow-list. They name directories, not credentials.
5. **The missing reload event goes upstream.** An RPC notification when OMP reloads its config is an OMP request (ADR-0027). When it exists, it replaces the file watch.

## Consequences

- The page costs three `omp config list` runs on open and one per change. The defaults read runs once per OMP binary.
- A key changed by an environment variable, with no file change, does not refresh until the next file change or reopen.
- `OMP_PARITY.md` gains rows for `omp config list/set/reset/path` and for the two env vars.
