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
- **Resolved per workspace, once.** `cedian_shell::Settings` gains `policy: Policy` (`Cedian` default, `Omp`) resolved for the canonical workdir, plus `notes: Vec<String>` the CLI prints.
- **Unattended runs cannot opt in.** `Settings::policy_for(run: RunKind)` returns `Cedian` for `RunKind::Unattended` (reviewers, automations), whatever the key says. `spawn_policy` takes the `RunKind`. No reviewer or automation exists yet, so a unit test pins the rule for when S3 and S7 add one.
- **Spawn profile variant.** `SpawnPolicy.approvals: Approvals` replaces `approval_mode`:
  - `Approvals::Cedian { mode: ApprovalMode, bash_patterns }` is today's profile: `--approval-mode`, `tools.approvalMode`, the `EXEC_TOOLS` prompt pins, the eval gate, `bash.patterns`, `computer.enabled: false`.
  - `Approvals::Omp` drops `--approval-mode` from argv and leaves `tools.approvalMode`, the exec prompt pins, the eval gate, `bash` and `computer` out of the overlay.
  - Both keep the host-tool allows (ADR-0028 decision 2), the scrubbed env, the `--config` overlay file, and every cedian `Deny` from `tool_policies` (strict-wins: a cedian Deny still lands in OMP's resolver, where tool-Deny is absolute).
  - `ApprovalMode` still cannot represent yolo. Under `Omp`, cedian never names a mode at all; OMP reads the user's.
- **Badge.** In an opted-in workspace the CLI prints, before the turn: `◆ OMP policy — approvals and computer from your OMP config (approvalMode: <v>, computer: <on|off|unknown>)`. The values come from `omp config get <key> --json` run in the workspace cwd through the spawn-profile module (same binary, scrubbed env), so cedian never re-implements OMP's config merge. A failure prints `unknown`.
- **Audit.** Every `tool_execution_start`/`tool_execution_end` the router sees during a turn appends one line to `.cedian/audit.jsonl`: `{timestamp, ordinal, item: {tool, tool_call_id, event: start|end, is_error, decision_source}}`, the §64 envelope. `decision_source` is `omp` under the opt-in and `cedian` under the default profile. Append-only; the ordinal continues from the file's line count.
- **Card label.** Under the opt-in, the tool card line of an OMP tool (not a cedian host tool) that ran gets ` · approved by OMP`. Headless refuses every dialog, so a call that ran was not approved by a person.

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

## Findings

(filled in as units land)
