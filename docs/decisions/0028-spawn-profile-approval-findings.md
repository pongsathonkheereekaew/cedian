# ADR-0028: Spawn profile approval findings — exec floor, host-tool allows, precedence test on an exec tool

- **Status:** Accepted
- **Date:** 2026-10-07
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §63, §64; [ROADMAP.md](../ROADMAP.md) S3 gate item 1
- **Amends (how, not what):** ADR-0020

## Context

Building P1 (ADR-0020) and reading the pinned OMP source (`packages/coding-agent/src/config/settings.ts`, `tools/approval.ts`, `modes/rpc/host-tools.ts`), then running the live precedence test against `omp 18.6.3`, turned up four facts ADR-0020 did not have:

1. **Layering.** OMP merges settings as global < project < `--config` overlay < runtime overrides (`--approval-mode` is a runtime override). Scalars and arrays in a higher layer replace lower ones. **Records deep-merge**, and `tools.approval` is a record. A project `.omp/config.yml` with `tools.approval.bash: allow` therefore survives an overlay that does not name `bash`.
2. **Resolution order.** For one call, OMP checks the tool's own decision first (a deny, an override, or an explicit allow such as a matching `bash.patterns` rule). Next comes the user policy `tools.approval.<tool>`, and only then the mode tier. A user policy outranks the mode, so pinning a tool to `prompt` also prompts its read-tier calls.
3. **Host tools have no tier.** The RPC `HostToolDefinition` has no approval field. OMP treats every host tool as `exec`, and `write` mode prompts for it. Nothing in headless cedian answers that prompt, so the turn stalls. Before P1 this was hidden because the dev machine runs `tools.approvalMode: yolo`. OMP 18.6 also mounts host tools as `xd://<tool>` devices: the model calls them through `read` (docs) and `write` (call).
4. **`write` mode auto-runs write-tier tools by design** (ADR-0023 default). ADR-0020's precedence test says "a write-class tool raises an approval prompt", which is true under `always-ask` but not under the default profile.

## Decision

1. **Exec floor.** The overlay pins each pure exec-tier OMP tool to `prompt` by name: `bash`, `eval`, `browser`, `task`, `vibe_spawn`, `vibe_send` (`cedian_omp::spawn_profile::EXEC_TOOLS`). This defeats a project-level `allow` (fact 1). Tools whose tier depends on their arguments (`gh`, `debug`, `lsp`, …) are left to the mode, because pinning them would prompt their read calls (fact 2). `bash` pinned to `prompt` still honours `bash.patterns` allows, since a pattern decision is a tool decision and resolves first.
2. **Host-tool allows.** The overlay sets `tools.approval.<name>: allow` for exactly the host tools cedian registers (`SpawnPolicy::host_tools`). cedian gates its own host tools at its own gate (ADR-0012), so OMP's exec-tier prompt would be a second, unanswerable gate. A host-tool name may not shadow an `EXEC_TOOLS` name; the profile rejects it. A cedian `tool_policies` entry still overrides (a `deny` stays a `deny`).
3. **Precedence test uses an exec-tier tool.** ADR-0020's verification reads, for the default `write` profile: a project config saying yolo + `computer.enabled: true` + `tools.approval.bash: allow` still yields an approval request for `bash`, without the command running, and no computer prelude. The computer check runs a control (a plain `omp -p` in the same workspace) that must see the prelude, so the check cannot pass vacuously. Write-tier prompting is tested only under `always-ask`.
4. **Auth.** Verified 2026-10-07 on `omp 18.6.3`: with the ADR-0015 allow-listed environment, a prompt reaches the provider. No extra variable is allow-listed.

## Known gap (open)

A project `tools.approval.<tool>: allow` for a tool cedian does not name in the overlay (MCP and extension tools, future OMP tools) still applies, because the overlay cannot delete keys from a deep-merged record. An isolated `--profile` (ADR-0020's fallback) does not help: it isolates global state, not the project's `.omp/config.yml`. This gap must be closed or explicitly accepted before S3 gate item 1 counts. Options: enumerate OMP's tool list at handshake and pin every exec-tier name; or ask upstream for an overlay key that resets `tools.approval`. An upstream request goes through a PR, never a fork (ADR-0027).

## Consequences

- `write`-mode turns run host tools without a stall, and bash runs only through an explicit `bash.patterns` allow or an answered prompt. The live `tool_cards` test now allows its `wc -l` probe by pattern.
- Tool cards name `xd://<tool>` writes after the host tool, and render the matching `read` as "Read host tool docs" (never counted as an edit).
- New exec-tier OMP tools must be added to `EXEC_TOOLS` when the OMP pin moves.
- Observed 2026-10-07 (live, `omp 18.6.3`): with a bare instruction to call `cedian_apply_edit`, the model refused because the `xd://` device was announced by a "dynamic-device notice" it treats as untrusted. A retry that names the tool as cedian's own and lets the model read the device docs first succeeded. Host-tool-first (ADR-0022) depends on the model trusting these devices, so P5 should state host tools' provenance in the tool description (or through an OMP skill, ADR-0025), and not rely on the prompt.
- **Refined by ADR-0039:** the known gap closes by pinning every unnamed `allow` read from `omp config get tools.approval` before spawn.
