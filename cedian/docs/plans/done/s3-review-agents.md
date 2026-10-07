# S3 — Review agents plan

Exit (ROADMAP S3 is the contract; restated as observable checks). The four gate items come first, because S3 work may not start before they hold.

- **G1. Spawn profile strict, gap closed.** The default profile already passes the live precedence test (`live_spawn_profile.rs`). The ADR-0028 gap closes per ADR-0041 decision 2: a project `tools.approval.<x>: allow` for a tool the overlay does not name is pinned to `prompt` (default) or `deny` (reviewer).
- **G2. Reviewer Seatbelt profile + bypass-proof test.** `policy/reviewer.sbpl` is generated per run (§64 mechanism 2). A reviewer process that writes inside the workspace, or runs a command that is not on the allow-list, gets "Operation not permitted" from the kernel. The test runs real `sandbox-exec`, not a policy check.
- **G3. Allow-list in `cedian.toml`.** Holds since row E (`reviewer_allow_list`). S3 adds the use: the list becomes the reviewer's `bash.patterns` and its `process-exec` rules.
- **G4. Audit tuple.** Every cedian-gate decision appends `{timestamp, ordinal, tool, command/prefix, decision, scope}` to `.cedian/audit.jsonl`, `Abstain` included. A test replays the file.
- **Exit.** An OMP turn asks for a review; cedian spawns the reviewer under the reviewer profile (fresh session, the `[review] model` from `cedian.toml`, ADR-0041). Each finding arrives through `cedian_review_finding` and attaches to the hunk it names. A `blocker` keeps the `review` gate unmet until the hunk changes or the finding is dismissed with a reason (audit row). `cedian shell` runs it with `review --agent`.
- **Exit, correction ledger (ADR-0032).** Each rejected hunk, reverted turn, user edit of an agent hunk, refused completion and dismissed finding appends a row to `.cedian/corrections.jsonl`. `cedian_correction_class` refuses a class with fewer than two events from two turns. A class shows `enforced` only with evidence that its check fails on the recorded mistake and passes at head.

**Owner-accepted 2026-10-07:** [ADR-0041](../decisions/0041-reviewer-is-a-host-spawned-omp-process.md): the reviewer is a separate OMP process cedian spawns at OMP's request.

**Timebox:** 5 working sessions for U1–U10.
**Partial exit:** the gate first (U1–U4), then the reviewer and findings (U5–U8), then the correction ledger (U9). Commit each green unit. If time runs out, README marks S3 `◐` and names the gap. Live recording of two OMP processes (implementer + reviewer) is the riskiest step; if it cannot be recorded, U8 stays open and S3 stays `◐`. Never hand-write a fixture.

## Prototypes (2026-10-07, OMP 18.6.1)

- **Seatbelt with OMP.** Deny-default writes, allowing only a state directory, `/private/tmp`, `/private/var/folders`, `~/.omp/run/daemons` and `/dev` handles: `omp -p` completes a turn (`sbx-ok`). `touch` and `>>` in the workspace fail with EPERM. The first attempt passed vacuously because the workspace sat under `/private/tmp`, so the generated profile ends with an explicit `deny file-write*` on the workspace.
- **Merged `tools.approval`.** `omp config get tools.approval --json`, run in a workspace with a project allow, returns that allow. `omp --config <overlay> config get` ignores the overlay, so the overlay's precedence is the one ADR-0028 proved live for `bash`.

## Design

- **Approval pins (G1).** `spawn_profile` gains `pins_for(effective: &Map, host_tools, variant) -> Map`. The CLI reads `tools.approval` with the existing `omp config get` runner before each spawn. Every key whose value is `allow` and that is not a host tool goes into the overlay as `prompt` (default) or `deny` (reviewer). An unreadable record fails closed for the reviewer (no spawn) and prints a note for the default profile.
- **Reviewer profile (G2, G3).** `SpawnPolicy` gains `Variant::Reviewer`: `--approval-mode always-ask`, overlay denies `edit`, `write`, `ast_edit`, `eval` and `computer`, `bash.patterns` allow only the allow-list prefixes. `policy/reviewer.sbpl` is generated into the run's state directory from a template plus the allow-list resolved to absolute executables. `process-exec` is allowed for OMP's binary, `/bin/sh`, and those executables only. The workspace is readable and explicitly unwritable.
- **Audit (G4).** `.cedian/audit.jsonl` keeps the §64 envelope `{timestamp, ordinal, item}`. Tool rows stay `item.kind = "tool"` (P8). Gate rows are `item.kind = "gate"` with `{tool, command, decision: allow|deny|ask|abstain, scope: once|task|session, source}`. Today's headless decisions: each host-tool call served or refused, and each OMP approval dialog cedian refuses (`abstain` when no person can answer).
- **Review request and findings (Exit).** Host tool `cedian_review_request {focus?}` (implementer side) takes the task's review diff, spawns the reviewer with that diff as its prompt, waits for its turn, and returns the findings summary. Host tool `cedian_review_finding {path, line, count, severity: blocker|suggestion|info, message}` (reviewer side) is checked against the current diff. A finding outside every hunk is refused with the reason. `cedian_review::Severity::Required` is renamed `Blocker`, matching the exit's word.
- **Review gate.** A `review` gate counts as met when no open `blocker` attaches to a pending hunk. A finding goes stale when its hunk changes (same rule as ADR-0024 evidence). `cedian review dismiss <id> <reason>` closes one and writes an audit gate row.
- **Correction ledger.** `.cedian/corrections.jsonl` (`snapshot_version` 1) rows `{ts, turn, kind, path?, detail}`. Host tool `cedian_correction_class {name, events: [row ids], check}` refuses fewer than two events from two turns. `enforced` needs a recorded run where `check` fails at the mistake's code state and passes at head.

## Units (dependency order)

| # | Unit | Check (done when) | Box |
|---|---|---|---|
| U1 | G1 approval pins: `pins_for` + CLI read before spawn | unit: project allow for an unnamed tool → `prompt` (default) / `deny` (reviewer); host tools keep `allow`; live precedence test still green | 0.5 |
| U2 | G4 audit gate rows: host-tool calls and refused dialogs, `abstain` | unit + replay: P5 and P8 replays show gate rows; a replay reads the file back | 0.5 |
| U3 | G2+G3 reviewer profile variant + `reviewer.sbpl` generator | unit: golden argv/overlay/sbpl; allow-list → exec rules | 0.5 |
| U4 | G2 bypass-proof test (real `sandbox-exec`) | `cargo test -p cedian_omp --test reviewer_sandbox`: workspace write and a non-allow-listed exec fail with EPERM; allow-listed command runs; an OMP turn completes under the profile (live, ignored) | 0.5 |
| U5 | `cedian_review_finding` + hunk attachment + `Blocker` | unit: finding on a hunk attaches; outside every hunk refused; stale after the hunk changes | 0.5 |
| U6 | `cedian_review_request`: spawn the reviewer, run one turn, collect findings | channel test with fake OMP for both sides | 0.75 |
| U7 | Review gate + `review dismiss` + `review --agent` in the shell | unit + CLI test: blocker keeps gate unmet; dismiss writes audit row and meets it | 0.5 |
| U8 | **Exit replay:** record a live implementer turn that asks for a review; the reviewer (other model, sandboxed) reports a blocker on its hunk | `replay_cli`: finding attached, gate unmet, dismiss → met, audit rows present | 0.75 |
| U9 | Correction ledger + `cedian_correction_class` | unit: rows per event kind; class refused under two events/two turns; `enforced` only with fail-then-pass evidence | 0.5 |
| U10 | README S3 row, ROADMAP, OMP_PARITY rows for the new host tools, plan → `done/` | docs only | — |

## Exit evidence

Every gate item and exit clause, and the test that shows it (`cargo test --workspace`; replays in `cargo test -p cedian_cli --test replay_cli`):

| Item | Evidence |
|---|---|
| G1 spawn profile strict; ADR-0028 gap closed | unit `config_allow_for_an_unnamed_tool_is_pinned_to_prompt`; replay `replay_cli_user_edit_over_agent_hunk_is_stale` finds a project `allow` for `some_mcp_tool` pinned to `prompt` in the generated overlay (fails without the wiring); live `live_spawn_profile` passes on OMP 18.6.1 |
| G2 reviewer Seatbelt profile, bypass-proof | `reviewer_sandbox`: a real `sandbox-exec` refuses workspace writes and a non-allow-listed exec with EPERM, while allow-listed commands and session writes work; fails with the workspace deny removed; live case completes an OMP turn under the profile |
| G3 allow-list in the user's `cedian.toml` | `reviewer_allow_list` becomes the reviewer's `bash.patterns` allows and its `process-exec` rules (`allow_list_becomes_allow_patterns`, sandbox golden test) |
| G4 audit tuple for every cedian-gate decision, `abstain` included | `replay_p5_worktree_request_and_headless_deny` reads the file back: ordinals in order, `deny` for the refused `bash`, `allow` for the served host tool; unit cases for `abstain` and two open logs |
| An OMP turn spawns a reviewer under the reviewer profile, fresh context, different model | `replay_s3_review_agent_blocker` (recorded live): reviewer overlay `always-ask` with `edit` denied, `sandbox-exec` profile ending in the workspace deny, model glm-5.3 (implementer muse-spark-1.3) |
| Findings arrive through `cedian_review_finding` and attach to the hunk they name | same replay: the blocker is bound to `    return a - b` in `/add.py`; unit cases refuse a finding outside every hunk |
| A `blocker` keeps the gate unmet until fixed or dismissed with a recorded reason | same replay: `cedian_complete` refused with `review: blocker f1`; `review dismiss` needs a reason and writes an audit row; unit `an_open_review_blocker_refuses_completion_until_it_closes`; a changed hunk makes the finding stale |
| Triggered from `cedian shell` with `review --agent` | `replay_shell_one_runtime_two_turns_lock`: `review --agent` in the shell attaches a finding to the turn's hunk (reviewer recorded on its own) |
| ADR-0039: the reviewer's model is OMP's `review` role; cedian stores no model id | unit `the_role_picks_the_model_and_falls_back_to_default`, `review_role_defaults_to_review_and_a_model_id_is_refused`; the replays read roles from the cedian-owned roles dir |
| ADR-0039: a same-model review is `inconclusive`, never `pass` | `replay_s3_same_model_review_is_inconclusive` (recorded live: no `review` role, the reviewer answered as the implementer's muse-spark): audit `independent: false`, review evidence `inconclusive`; the independent replay records `fail` evidence for its blocker; unit `only_a_review_by_another_model_is_independent_and_can_pass`, `a_review_cedian_ran_is_evidence_for_the_review_gate` |
| Correction ledger rows | replays assert `hunk_rejected`, `user_edited_agent_hunk` (once), `turn_reverted`, `completion_refused`, `finding_dismissed` |
| `cedian_correction_class` refuses under two events from two turns; `enforced` only with fail-then-pass evidence | unit `a_one_off_is_not_a_class`, `enforced_only_when_the_check_failed_on_the_old_code_and_passes_now` |

## Findings (2026-10-07)

- **A review asked for mid-turn saw nothing** (found live): a turn's changes reached the review store only at turn end. The review tool and the blocker check now bring the running turn in first, under the row G rule.
- **Two audit logs collided on ordinals** (found live: 9 twice) while the reviewer ran inside the implementer's turn. One counter per file now; reviewer rows carry `actor: reviewer`.
- **Seatbelt prototype passed vacuously once:** the workspace sat under `/private/tmp`, which the profile allows. The generated profile denies the workspace last.

Since the move (ADR-0042) these tests run in the fork with `script/cedian-check`.

## Open after S3

In ROADMAP Follow-ups: the `cedian://corrections` URI and the `repeated` escalation (ADR-0032 decision 4); `continue_escalated` rows and the `model` field are not recorded yet; a proof cannot yet check that both evidence runs were the enforcer's command (evidence keeps no command).

