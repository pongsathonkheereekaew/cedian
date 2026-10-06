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
