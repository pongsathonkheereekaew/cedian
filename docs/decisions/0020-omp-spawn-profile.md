# ADR-0020: OMP strictness is set by a cedian spawn profile (argv + config overlay), not RPC

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §9 (`computer` row), §38, §63, §64, §71, §77
- **Amends (how, not what):** ADR-0008, ADR-0012

## Context

ADR-0012 and §63 say the runtime "MUST set `approvalMode` + `approval.*` + `bash.patterns` at spawn", and ADR-0008 says `computer` stays hard-disabled. An audit on 2026-10-06 found:

- The vendored RPC client (OMP 18.6.1) has 66 commands and **none** sets approval policy or disables a tool. Only `set_ask_dialog` is RPC.
- cedian currently spawns `omp --mode rpc-ui --session-dir … --cwd …` with nothing else, so it inherits whatever the user's config says. On the dev machine that is `tools.approvalMode = "yolo"` and `computer.enabled = true`. **Both hard rules are violated on every turn today.**
- `computer` is not a separate tool. It is a scriptable host-desktop prelude inside the `eval` tool, switched by `computer.enabled`, so a `--tools` allow-list alone cannot remove it.
- The OMP CLI does expose `--approval-mode=always-ask|write|yolo`, `--config=<overlay.yml>` (repeatable, "extra config.yml-style overlay for this run"), `--tools=<list>`, `--no-lsp`, and `--profile`.
- The spawn argv is built in two places (`runtime.rs` and `respawn.rs`).

## Decision

1. **One builder.** A single `SpawnProfile` in `cedian_omp` produces the argv for spawn and respawn alike. It goes through the existing respawn validation (absolute paths, bounded args, no control characters). The dedupe key stays `{binary_path, session_dir, cwd}`.
2. **Argv:** `omp --mode rpc-ui --session-dir <dir> --cwd <ws> --approval-mode <write|always-ask> --config <cedian overlay>`. The default is `write` (ADR-0023): read and in-workspace edits run, everything else follows the overlay policy. `yolo` is never passed. `--auto-approve` is never passed.
3. **Overlay.** At spawn, cedian generates the overlay from `cedian.toml` (ADR-0018) into a cedian-owned path inside the session dir. It never writes the user's `.omp/` (§77). Minimum content:
   - `computer.enabled: false` until ADR-0008's atomic landing
   - `tools.approvalMode` (same value as the flag)
   - `tools.approval.*` per-tool policy
   - `bash.patterns` from the policy file
   - `bash.allowCompoundCommands: false`
   - **the eval gate:** `tools.approval.eval` set to prompt. `eval` runs Python/JS, which is as powerful as `bash` but is not covered by `bash.patterns`, so it is never auto-approved. Reviewer and automation profiles deny it outright.
4. **Environment.** The child gets the ADR-0015 scrubbed environment: no `SSH_AUTH_SOCK`, no cloud or forge credential variables (`AWS_*`, `GH_TOKEN`, `GITHUB_TOKEN`, …), allow-list rather than deny-list. OMP's own provider auth must come from its auth store (`omp login`, its profile directory), not from inherited environment.
5. **Still RPC:** `set_ask_dialog(true)` right after the handshake, as today.
6. **Fail closed.** If the overlay cannot be written or the profile fails validation, the runtime does not start. It never falls back to a bare spawn.
7. **Not a sandbox.** These are OMP-layer controls. OS enforcement stays ADR-0012 mechanism 2 (Seatbelt). The overlay only stops OMP from being permissive by default.

## Verification (must pass before any ✅ that relies on it)

- **Precedence test (live lane).** The project `.omp/config.yml` sets `computer.enabled: true` and `tools.approvalMode: yolo`, and cedian spawns with its profile. Assert that a write-class tool raises an approval prompt instead of auto-running, and that the `eval` computer prelude is unavailable. *Overlay precedence over project/user config is NOT yet verified upstream.* If the overlay loses, the fallback is to run under an isolated `--profile`, and this ADR is amended.
- **Auth test (live lane).** With the scrubbed environment, a prompt still reaches the provider. If OMP's auth needs an environment variable, that single variable is allow-listed by name and recorded here.
- **Unit test.** The builder output is exact (golden argv), the environment allow-list is exact, and a profile with `yolo` or a missing overlay is rejected.

## Consequences

- Fixes the two active violations with a small, local change in `cedian_omp` + the CLI.
- `--no-lsp` is NOT used now: it would remove OMP's `lsp` tool, which the model expects (§21). Revisit at S9 when Zed's LSP backs `cedian://` (stand-in row B).
- Every new strictness knob goes into the overlay generator, never into ad-hoc spawn flags.
- **Superseded in part by ADR-0035:** decisions 2–3 describe the default profile. A project can opt in to OMP's own approval mode (`approval = "omp"`) and to OMP's `computer` (`computer = "omp"`) in `cedian.toml`.
