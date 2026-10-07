# ADR-0012: Permissions: OMP configured strict, strict-wins at the cedian gate

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §63 (upstream fact), §64

## Context

OMP approvals default to YOLO; OMP's own resolver order is not "strict-wins" and cedian can't reorder it. Codex (`openai/codex`) was studied in depth as prior art for policy + sandboxing.

## Decision

- The runtime configures OMP strict at spawn (`approvalMode`, `approval.*`, `bash.patterns`, `eval` gate, `set_ask_dialog(true)`).
- cedian re-evaluates every granted action at its own boundary (host-tool dispatch + sandbox); `Deny > Ask > Allow`; cedian can still deny what OMP allowed, never force what OMP denied.
- Fail-closed paths (no-UI, provider safety checks) are normal control flow.
- Adopted Codex mechanisms: three-valued prefix rules, OS-enforced Seatbelt under the policy, protected metadata paths, audit tuple (cedian extension inside Codex's JSONL envelope).

## Consequences

Linux replication is out of scope (macOS-only). Open item: the runtime does not set `approvalMode` yet — S3 gate 1.

- **Amended by ADR-0020 (how):** RPC cannot set approval policy; strictness is applied through the spawn profile (`--approval-mode` + generated `--config` overlay).
- **Superseded in part by ADR-0035:** configuring OMP strict at spawn is the default, not the only mode; strict-wins at the cedian gate is unchanged in every mode.

## Original research notes (verbatim from the 2026-10-06 plan)

> **Grill R2 — Codex lessons (`openai/codex`, Rust, 128k⭐, studied 2026-10-05; deep-dived 2026-10-06).** Adopt four mechanisms, adapted to the cedian ownership split. Corrections from the deep-dive are inline:
>
> 1. **Three-valued decisions, not boolean — with Codex's exact semantics.** Codex `execpolicy` (`codex-rs/execpolicy`) has THREE builtins: `prefix_rule(pattern, decision?, justification?, match?, not_match?)`, `network_rule(host, protocol, decision, justification?)`, `host_executable(name, paths)`. `decision` is `Allow|Prompt|Forbidden` in `prefix_rule` ONLY — `deny` is valid SOLELY as a `network_rule` alias for `forbidden`; `prefix_rule(decision=deny)` FAILS to parse. `match`/`not_match` are PER-RULE unit tests evaluated against an EPHEMERAL single-rule policy (basename resolution forced on), not policy-wide integration tests; violations report file+line/col (`ExampleDidNotMatch`/`ExampleDidMatch`). Evaluation: exact-first-token lookup → matchedRules lists EVERY matching rule → effective decision = STRICTEST across all matches → NO match yields decision OMITTED (not allow). cedian adopts: same three builtins, same per-rule validation, same strictest-wins, same no-match-omitted. Canonical spelling stays `Allow|Ask|Deny` (Codex `prompt|forbidden` ≡ `Ask|Deny`).
> 2. **OS-enforced sandbox under the policy, not policy alone — starter profile identified.** Codex layers `codex-rs/sandboxing` per-OS backends (macOS Seatbelt `.sbpl`, Linux Landlock/bwrap/namespaces, Windows restricted tokens) + `linux-sandbox` launcher + `windows-sandbox-rs` — so a `Deny` verdict is enforced by the kernel even if the agent bypasses the policy check. cedian mirrors this: the CUA `cua-driver` contract (R2-6a) is the enforcement floor for `computer`; filesystem/network/process verdicts in `permissions.rs` must map to an OS mechanism (Seatbelt profile on macOS first), never live as TOML-only intent. Adaptation seed: `codex-rs/sandboxing/src/seatbelt_base_policy.sbpl` (~3.9KB, deny-default + process-exec/fork + sysctl/mach/pty allows) composed per `seatbelt.rs` (BASE + generated read/write roots + unlink-deny anchors + network fragment + `sandbox-exec -p PARAMS`); hardenings to copy verbatim: canonicalize-then-compare paths, write-exclusions deny BOTH literal and subpath, proxy-unparseable ⇒ fail-closed empty network policy, execute ONLY `/usr/bin/sandbox-exec`. Linux replication is thousands of lines (`linux_run_main.rs` ~1.5k + `bwrap.rs` ~120KB) — macOS-only cedian has no such obligation.
> 3. **Protected metadata paths.** Codex `codex-protocol::permissions::PROTECTED_METADATA_PATH_NAMES` (imported by `seatbelt.rs`, NOT a local list) hard-blocks agent writes to `.git/.codex/.agents/.aws` even under writable roots. cedian adopts the same list (plus `.cedian/`, OMP session dir): metadata writes are `Deny`, not `Ask` — no prompt fatigue bypass. (Canonical spelling `Deny`; `forbidden` here cites the Codex symbol only.)
> 4. **Escalation as a first-class session — audit envelope clarified.** Codex `shell-escalation` (`EscalateServer`, `EscalationSession`, `PreparedExec`, `ResolvedPermissionProfile`) re-resolves permissions per exec; sessions persist as JSONL rollout lines `{timestamp: RFC3339, ordinal?, ...item}` under `$CODEX_HOME/sessions/` (`codex-rs/rollout/src/recorder.rs`, replayable via jq/fx). CAVEAT: no dedicated per-tool `(tool_call, approval_decision, policy_outcome)` tuple was confirmed upstream — cedian's `{tool, command/prefix, decision, scope (once|task|session), timestamp}` tuple is a cedian EXTENSION inside the Codex `{timestamp, ordinal, item}` envelope, not parity. `decision: Allow | Ask | Deny | Abstain` (canonical). `Abstain` is system-generated (lease expiry / dead runtime, §63): deny-for-execution, blocked-for-gates (§54), never writable in policy.
