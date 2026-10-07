# P8 — `policy = "omp"`: a project opts in to the user's own OMP policy

ADR-0035 adds one per-project key to the user's `cedian.toml`. With `[projects."<path>"] policy = "omp"`, OMP runs in that workspace with the user's own approval mode and `computer` setting, instead of the strict default profile (ADR-0020). Everything else cedian enforces stays: host-tool gating, the scrubbed env, native dialogs, workflow gates. The opt-in is always visible (badge) and always audited (`.cedian/audit.jsonl`, `decision_source: omp`). Reviewers and automations never get it.

This is the one change that loosens the spawn profile. The default profile, which rejects yolo, is unchanged and its tests stay as they are.

**Exit (ROADMAP P8):**
- **Live:** an opted-in temp project whose `.omp/config.yml` says yolo runs an exec-tier call (`bash`) with no approval prompt, and the call is in `.cedian/audit.jsonl` with `decision_source: omp`. Recorded through `cedian prompt` against real OMP 18.6.1.
- **Hermetic:** the same recorded turn replays in `cargo test -p cedian_cli --test replay_cli` and asserts the same facts. The replay also asserts that the same project with no opt-in uses the default profile (argv has `--approval-mode write`, overlay has `computer.enabled: false`).
- Badge, `approved by OMP` card label and audit rows are headless here (CLI output and a file); the GPUI surfaces are S9.

**Timebox:** 1.5 working sessions. **Partial exit:** U1–U3 and U5–U6 are the exit (opt-in parsed, profile variant, unattended runs pinned to the default, audit, live + replay). U4 (badge with OMP's effective `computer` value) can land after; until then the badge prints the opt-in without the `computer` state and README names the gap. Live recording is the riskiest step: if real OMP can't be recorded, U6 stays open and P8 stays `◐`. Never hand-write the fixture.

## Design

- **Where the key lives.** `[projects."<abs path>"] policy = "omp" | "cedian"` in the user's `cedian.toml` (row E made that the only settings file, so a repository cannot opt itself in). The key is canonicalized at load. A key that is relative, or names a directory that does not exist, gives a note ("project key … ignored: …; default profile") and the default profile. A `cedian.toml` that fails its schema check refuses the command (row E), which is stricter than ADR-0035 decision 6's "default profile applies".
- **Resolved per run, never stored.** `Settings::policy_for(workdir, RunKind) -> PolicyChoice { policy, notes }`. There is no public resolved field, so every caller names the run kind. The CLI prints the notes.
- **Unattended runs cannot opt in.** `RunKind::Unattended` (reviewers, automations) always gets `Cedian`, whatever the key says. No reviewer or automation exists yet, so a unit test pins the rule for when S3 and S7 add one.
- **Keys are exact and fail closed.** A key must be absolute and already canonical as written (no symlink in it, so nothing writable can repoint it). Two keys that match the same workspace with different values give `Cedian` and a note. The opt-in is refused (note, `Cedian`) when the user's `cedian.toml` itself sits inside the workspace.
- **Spawn profile variant.** `SpawnPolicy.approvals: Approvals` replaces `approval_mode`:
  - `Approvals::Cedian { mode: ApprovalMode, bash_patterns }` is today's profile: `--approval-mode`, `tools.approvalMode`, the `EXEC_TOOLS` prompt pins, the eval gate, `bash.patterns`, `computer.enabled: false`.
  - `Approvals::Omp` drops `--approval-mode` from argv and leaves `tools.approvalMode`, the exec prompt pins, the eval gate, `bash` and `computer` out of the overlay.
  - Both keep the host-tool allows (ADR-0028 decision 2), the scrubbed env, the `--config` overlay file, every cedian `Deny` in `tool_policies`, and every `Deny` rule in `bash_patterns` (with `allowCompoundCommands: false` when there is one). Strict-wins: a cedian Deny still lands in OMP's resolver, where tool-Deny is absolute. U6 proves that live (opted-in, project yolo, `project_write = "deny"`: OMP refuses `write`).
  - cedian `Ask` tiers (`project_write = "ask"`, `dangerous = "ask"`) do not apply under `Omp`: that is what the opt-in hands over. The CLI prints a note when the user's `cedian.toml` has an `ask` tier in an opted-in workspace.
  - The dedupe key carries the variant, so a runtime spawned under one profile is never reused for the other.
  - `ApprovalMode` still cannot represent yolo. Under `Omp`, cedian never names a mode at all; OMP reads the user's.
- **Badge.** In an opted-in workspace the CLI prints, before the turn: `◆ OMP policy — approvals and computer from your OMP config (approvalMode: <v>, computer: <on|off>)`. The values come from `omp config get <key> --json` run through the spawn-profile module (same binary, scrubbed env, 10 s timeout), so cedian never re-implements OMP's config merge. It runs twice: in the workspace (the effective value) and in an empty temp dir (the global value). When they differ the badge says `from the project's .omp/config.yml`. Owner decision 2026-10-07: under the opt-in the project layer may set these (ADR-0035 as written); the badge names it. A failure prints `unknown — assume on`.
- **Audit.** Every `tool_execution_start`/`tool_execution_end` the router sees during a turn appends one line to `.cedian/audit.jsonl`: `{timestamp, ordinal, item: {tool, tool_call_id, event: start|end, is_error, decision_source}}`, the §64 envelope. `decision_source` is `omp` under the opt-in and `cedian` under the default profile. Append-only; the ordinal continues from the file's line count.
- **Card label.** Under the opt-in, the card of an exec-tier call (`EXEC_TOOLS`: a call the default profile would have prompted for) that ran gets ` · approved by OMP`. Headless refuses every dialog, so such a call was not approved by a person. At S9, where dialogs can be answered, the label and the audit row must come from the dialog record per `tool_call_id`, not from the mode.
- **Accepted risk, recorded.** Under yolo the agent can edit `.cedian/audit.jsonl`, the user's `cedian.toml` and the global OMP config. ADR-0035 accepts this (badge, audit, per-project scope are the mitigations); Seatbelt (S3) is what makes `.cedian/` unwritable. The audit append failing fails the turn.

Not done here: Seatbelt, the S3 audit tuple for cedian-gate decisions (this file gains those rows in S3), GPUI badge and settings UI (S9).

## Units (dependency order)

| # | Unit | Check (done when) |
|---|---|---|
| U1 | `[projects]` in `cedian.toml`: parse, canonicalize keys, resolve `Settings.policy` for the workdir, notes; `Settings::policy_for(RunKind)` | `cargo test -p cedian_shell`: opted-in path → `Omp`; other path → `Cedian`; `/tmp` vs `/private/tmp` key matches the canonical workdir; relative or missing key → note + `Cedian`; `policy = "yolo"` → parse error; `Unattended` → `Cedian` even when opted in |
| U2 | `Approvals` in `SpawnPolicy`; argv and overlay per variant | `cargo test -p cedian_omp`: golden argv/overlay for `Cedian` unchanged; `Omp` argv has no `--approval-mode`, overlay has no `approvalMode`/exec pins/`bash`/`computer`, keeps host allows and cedian Denies; `yolo` still unparseable |
| U3 | CLI: `spawn_policy(settings, RunKind::Interactive)`; notes printed | `cargo test -p cedian_cli` unit: opted-in settings → `Approvals::Omp`; default → `Cedian` with today's fields |
| U4 | Badge: `omp config get` through the spawn-profile module; printed in opted-in prompt/shell | unit test on the JSON parse; replay prints the badge (the fake answers `config get`) |
| U5 | Audit rows + card label | unit test on the audit envelope and ordinal; replay asserts rows and label |
| U6 | **Exit:** record `CEDIAN_P2_RECORD=p8` (opted-in temp project, project `.omp/config.yml` yolo, prompt asks for `bash touch probe`), then replay | fixture shows a `bash` tool start/end and no `extension_ui_request`; replay: probe exists, no `refused` line, audit has `bash` with `decision_source: omp`, card says `approved by OMP`; same workspace without the key → default argv and overlay |
| U7 | Docs: OMP_PARITY rows (approval modes, `computer`, `omp config get`), README P8 row | P7 parity test green; README row names what is live and what is headless |

## Interrogate verdict (2026-10-07)

Reviewers: sonnet (9 findings) and opus (9 findings); the third seat (fable) had no credits and the first round died on a session limit.

- **Act on.** Both flagged that the project's `.omp/config.yml` decides yolo and `computer` under the opt-in. The owner kept ADR-0035 as written; the badge names the project layer. Opus: keep bash Deny patterns under `Omp`; prove tool-Deny holds under yolo live; refuse non-canonical keys (symlink repointing); conflicting keys fail closed; refuse the opt-in when the user file is in the workspace. Sonnet: put the variant in the dedupe key; label only calls that would have prompted; `unknown` means assume on; a timeout on `omp config get`. Both: the audit append fails the turn on error.
- **Consider.** Sonnet: `decision_source` describes the mode, not who decided. True once dialogs can be answered (S9); recorded above as an S9 rule.
- **Noted.** The agent can tamper with the audit log and the user's config under yolo: ADR-0035's accepted risk until Seatbelt. Ordinal races: a mutating command can't run while `cedian shell` holds the workspace, so one writer at a time.
- **Dismissed.** Sonnet: the row E floor can create `Prompt` tool policies. It can't; the floor is workflow gates, not tool policies. Sonnet: `dedupe_key` reuse lets an unattended run attach to an opted-in child. Nothing reuses runtimes by that key today; the variant goes in the key anyway.

## Findings

All units landed 2026-10-07: U1 `f3dd28e` + `6f32e26` (fail closed on doubt), U2–U3 `a33a8ca`, U5 `4125312`, U6 `511bd61`, U4 `82931f5`, U7 with this file.

- **OMP enforces a cedian Deny under yolo.** The recorded `p8deny` turn: `tool_execution_start write`, then `tool_execution_end` with `Tool "write" is blocked by user policy.` Strict-wins holds through OMP's resolver in the opt-in profile.
- **A blocked call still starts.** OMP emits `tool_execution_start` before its policy check, so "the call started" does not mean "the call was approved". The `approved by OMP` label needs a completed exec-tier call; the audit row for a blocked call ends with `is_error: true`. S9 should take approval from the dialog record per `tool_call_id`.
- **OMP sends fire-and-forget UI requests in every turn** (`setWidget`, key `autoresearch`). "No dialog" means no `select|confirm|input|editor|ask` request, not no `extension_ui_request`.
- **The live recording used the model OMP picked** (`opencode-go/muse-spark-1.3-contributor`), not a pinned one; the fixture records it.
- **Badge, live.** `cedian state` in an opted-in temp project against real OMP printed `approvalMode: yolo; computer: on`. Both values equal the global config on this machine, so the badge does not name the project layer there; the replay covers the case where they differ.
- **The `ask` tier note prints in every opted-in workspace** with default permissions, because `dangerous = "ask"` is the default. It is true and short; it stays.
- **Order changed:** U4 (badge) landed after U6, as the partial exit allowed.

## Exit evidence

README status row as of 2026-10-07, moved here so README keeps only the status:

✅ 2026-10-07 (live recording + hermetic replay, `replay_p8_omp_policy`): a project opted in with `[projects."<path>"] policy = "omp"` in the user's `cedian.toml`, whose `.omp/config.yml` says yolo, runs `bash` with no approval dialog; the card says `approved by OMP`, `.cedian/audit.jsonl` has its rows with `decision_source: omp`, and the badge names the project layer. `replay_p8deny_omp_policy`: under the same opt-in, `project_write = "deny"` still blocks OMP's `write` (strict-wins). Keys fail closed (canonical only, conflicts, file inside the workspace); unattended runs always get the default profile (unit test; no reviewer or automation exists yet). Badge, label and audit are headless here; GPUI at S9
