# Decisions (ADRs)

One file per decision. Never edit an accepted ADR's decision — write a new ADR and mark the old one `Superseded by ADR-xxxx`.

| ADR | Decision |
|---|---|
| [0001](0001-environment-vs-intelligence.md) | cedian is the environment, OMP is the intelligence |
| [0002](0002-omp-child-process-rpc-v2.md) | OMP runs as a child process over RPC v2 |
| [0003](0003-bundled-omp-pin-and-trust.md) | Bundled OMP is pinned and verified as a trust boundary |
| [0004](0004-integrate-via-host-tools-and-uris.md) | Integrate through host tools and `cedian://` URIs — OMP has no backend seam |
| [0005](0005-agent-sync-v1-unsaved-buffers.md) | Unsaved buffers: Agent Sync V1 (with an expiry) |
| [0006](0006-review-baseline-provenance-precedence.md) | Review: task baseline, cedian-owned provenance, strict hunk precedence |
| [0007](0007-shared-browser-and-evidence-frames.md) | One shared browser, user input preempts, evidence bound to frames |
| [0008](0008-computer-tool-cua-driver-only.md) | `computer` goes through the CUA `cua-driver` contract only |
| [0009](0009-worktree-mechanism-vs-policy.md) | Worktrees: cedian owns the mechanism, OMP owns the policy |
| [0010](0010-gate-checker-a1-narrow.md) | Workflow split A1-narrow: OMP drives, cedian checks |
| [0011](0011-reviewers-read-only-sandbox.md) | Reviewer agents are read-only, enforced by the OS |
| [0012](0012-permissions-strict-wins-at-cedian-gate.md) | Permissions: OMP configured strict, strict-wins at the cedian gate |
| [0013](0013-ask-is-a-lease.md) | `ask` is a lease, not a lock |
| [0014](0014-agent-modes.md) | Agent modes: Normal + Goal; Plan only via OMP addition; no Deep |
| [0015](0015-two-shells-one-policy.md) | Two shells, one policy |
| [0016](0016-crash-persistence-resume.md) | Crash, persistence and resume |
| [0017](0017-slices-exit-rule-stand-ins.md) | Plan by vertical slices, with an exit-criterion rule and registered stand-ins |
| [0018](0018-settings-in-one-toml.md) | Settings and policy live in one TOML file |
| [0019](0019-scope-cuts.md) | Scope cuts: PR v1, local-only automations, out-of-scope list, daemon track |
| [0020](0020-omp-spawn-profile.md) | OMP strictness via a cedian spawn profile (argv + config overlay), not RPC |
| [0021](0021-headless-host-process.md) | Headless work runs in one long-lived `cedian shell` process |
| [0022](0022-host-tool-first.md) | Host-tool-first: OMP reports to cedian through host tools |
| [0023](0023-product-scope.md) | Product scope — what cedian is |
| [0024](0024-evidence-bound-to-code-state.md) | Evidence is bound to code state, has three outcomes, and claims must cite it |
| [0025](0025-playbooks-are-omp-skills.md) | Playbooks and project verification profiles are OMP skills |
| [0026](0026-fast-lane.md) | Fast lane — proportional rigor, inline edit, revert turn, measured speed |
| [0027](0027-zero-omp-fork.md) | Zero OMP fork — cedian adapts to OMP, only Zed is forked |
| [0028](0028-spawn-profile-approval-findings.md) | Spawn profile approval findings: exec floor, host-tool allows, precedence test on an exec tool |
| [0029](0029-agent-edit-import-via-reload.md) | Agent-edit import via `Buffer::reload`; attribution is a cedian map, not `ReplicaId::AGENT` |
| [0030](0030-fork-layout-and-build-profile.md) | Zed fork layout (crates move into the fork at S9) and build profile |
| [0031](0031-evidence-cites-tool-not-call-id.md) | Evidence names the tool; cedian resolves the call id from the router log |
| [0032](0032-correction-ledger.md) | Correction ledger: recorded corrections become enforced rules |
| [0033](0033-worker-brief-contract.md) | A worktree request carries a checked brief; worker liveness is side effects |
| [0034](0034-identity-and-omp-parity.md) | Identity: a Cursor-style IDE for OMP, with full OMP feature parity |
| [0035](0035-omp-native-approval-opt-in.md) | Per-project opt-in to OMP's own policy (approval mode and `computer`) |
| [0036](0036-s2-evidence-freshness-and-turn-end-block.md) | S2 readings of ADR-0024: historical reproduction evidence, turn-end block, headless code state |
| [0037](0037-benchmark-without-cursor.md) | The speed benchmark measures cedian on its own, against fixed budgets |
| [0038](0038-s4-s5-close-inside-s9.md) | After S3 comes S9; S4 and S5 close inside S9 |
| [0039](0039-model-roles-and-independent-review.md) | Model roles are OMP's `modelRoles`; an independent review needs a different model |
| [0040](0040-omp-settings-mirror-omp-config.md) | The OMP settings page mirrors OMP's config; OMP's CLI and cedian edit the same state |
| [0041](0041-reviewer-is-a-host-spawned-omp-process.md) | A reviewer is a separate OMP process that cedian spawns at OMP's request |
| [0042](0042-all-code-moves-into-the-fork.md) | All cedian code moves into the fork; this repo keeps the docs |
| [0043](0043-reviewer-sandbox-private-state-no-credentials.md) | The reviewer writes only its own run directory, cannot read credentials, and gets a fixed tool set |
| [0044](0044-cedian-state-outside-the-workspace.md) | cedian's per-workspace state lives outside the workspace |
| [0045](0045-omp-settings-live-by-watching-config-sources.md) | The OMP settings page stays live by watching OMP's config files; sources are derived through OMP |
| [0046](0046-one-driver-per-session-by-file-holders.md) | The app refuses a session another process drives, detected by who holds OMP's session files |
| [0047](0047-bash-writes-are-agent-edits.md) | Files an OMP `bash` call changes are agent edits |
| [0048](0048-one-lsp-zeds-staged.md) | The app runs one language server, Zed's; OMP's `lsp` tool is turned off only once Zed covers it |
| [0049](0049-one-chromium-owned-by-the-app.md) | The app owns one Chromium per workspace; OMP attaches to it over CDP |
