# ADR-0035: Per-project opt-in to OMP's own policy (approval mode and `computer`)

- **Status:** Accepted (owner, 2026-10-07)
- **Date:** 2026-10-07
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §9 (`computer` row), §38, §64, §77; [ROADMAP.md](../ROADMAP.md) P8
- **Supersedes in part:** ADR-0020 decision 2 ("`yolo` is never passed") and decision 3 (`computer.enabled: false` always); ADR-0012 ("the runtime configures OMP strict at spawn", now the default, not the only mode); ADR-0008 ("hard-disabled", now the default)
- **Follows from:** ADR-0034 (OMP feature parity)

## Context

The spawn profile (ADR-0020, ADR-0028) always passes `--approval-mode write` and a `--config` overlay that pins exec-tier tools to `prompt` and sets `computer.enabled: false`. That overrides the user's own OMP config. A user who runs OMP with `tools.approvalMode: yolo` and `computer.enabled: true` (as the dev machine does) loses both inside cedian. Under ADR-0034 that is a cut OMP feature.

The defaults exist for good reasons: OMP's resolver is not strict-wins (§64), and `computer` drives the real desktop (ADR-0008). The owner chose to keep them as defaults and add one explicit, per-project opt-in that hands both to OMP.

A split into two keys (approval and `computer` separately) was offered so that yolo could not turn on desktop control by itself. The owner chose one key: "use my OMP config" should mean all of it.

## Decision

1. **One key, per project, in `cedian.toml` only.**
   ```toml
   [projects."/Users/pond/src/app"]
   policy = "omp"      # default "cedian"
   ```
   - The project key is the canonicalized absolute workspace path. There are no globs and no global switch.
   - The opt-in is read only from the user's `cedian.toml`. A repository cannot opt itself in: nothing in the workspace (`.omp/`, `.cedian/`, files) can set it (§77).
2. **`policy = "omp"` hands approvals and `computer` to the user's OMP config.**
   - The spawn profile omits `--approval-mode`.
   - The overlay leaves out `tools.approvalMode`, the `EXEC_TOOLS` prompt pins, the eval gate and `computer.enabled`.
   - OMP then resolves approvals from the user's own config, yolo included. OMP's `computer` prelude runs exactly when the user's OMP config enables it.
   - The CUA-driver route (ADR-0008) remains the default target for `computer` under `policy = "cedian"` once it lands.
3. **Unchanged in this mode:**
   - host-tool allows (ADR-0028 decision 2)
   - the scrubbed environment (ADR-0015)
   - `set_ask_dialog(true)`: OMP's own prompts still render as native dialogs
   - the cedian gate at its own boundary (host-tool dispatch, protected metadata paths, Seatbelt once it lands): cedian can still deny (ADR-0012)
   - workflow gates and evidence (ADR-0010, ADR-0024)
4. **Always visible, always audited.** In an opted-in workspace:
   - the agent panel shows a persistent badge, "OMP policy", and names whether OMP's config has `computer` on
   - tool cards for calls OMP auto-approved are labelled "approved by OMP"
   - every `tool_execution_start/end` from the router log is appended to `.cedian/audit.jsonl` with `decision_source: omp`
5. **Never for unattended or review runs.** Reviewer subagents (ADR-0011) and automations (S7) always use the cedian profile, whatever the project key says. Unattended runs have no one to answer a prompt and no one to notice a mistake.
6. **Fail closed on doubt.** If the project key does not canonicalize, or `cedian.toml` fails its schema check, the default profile applies and the UI says why.

## Consequences

- OMP's approval modes and `computer` move from "cut" to `gated ADR-0035` in the parity ledger (ADR-0034).
- The ADR-0020/0028 precedence test still guards the default profile. P8 adds a second live test: with `policy = "omp"` and a project yolo config, an exec-tier call runs without a prompt and lands in the audit log with `decision_source: omp`.
- The ADR-0028 known gap (project `tools.approval` allows for tools the overlay does not name) only matters in the default profile. In the opt-in profile the user's config applies by design.
- Risk accepted by the owner: in an opted-in project, a prompt-injected agent can do whatever the user's OMP config allows, including desktop control when `computer` is on. The badge, the audit log and the per-project scope are the mitigations.
