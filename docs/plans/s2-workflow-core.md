# S2 — Workflow core plan

Exit (ROADMAP S2 is the contract; restated here as observable checks):
- **(d) Blocked claim.** A recorded OMP turn runs the bug-fix playbook skill, then claims completion while the required `verify` gate is unmet. After the turn the workflow is `blocked` (or `failed` when the agent failed the phase itself, ADR-0036) and the missing gates are printed. Proven by hermetic replay (§86).
- **(a) ADR-0024 in code.** Evidence has `code_state` and `outcome ∈ pass|fail|inconclusive`. Staleness is computed purely from versions passed in as data. A later edit to a bound file makes the evidence stale, and stale evidence doesn't count for a fresh gate. `inconclusive` never passes. `cedian_complete` takes a claims ledger (`measured|inferred|guess` + evidence ids), and measurements carry `{runs, median, range, limiter, build_profile}`. `workflow.json` moves to `snapshot_version` 2.
- **(b) Verification profile (ADR-0025).** A gate can require `feature <id> proven`. Evidence from an instance with no passing Doctor since its last failed or surprising drive is `inconclusive`. A profile never run end to end (launch → doctor → drive → evidence → cleanup) is a draft and satisfies no gate.
- **(c) Playbook is an OMP skill** (bug fix: reproduce → verify) that calls `cedian_workflow_update` / `cedian_complete`. cedian never writes the user's `.omp/` (§77). Test fixtures carry their own copy.
- **(e) Fast lane.** A recorded trivial-edit turn with no floor gate completes with no `workflow.json` at all.
- CLI-typed evidence is `unattributed` (S2 exit). Today `cedian workflow evidence` attributes it to a fake `cli-turn-N` id, and that has to stop.
- **Benchmark (ADR-0026):** tasks proposed below. **Waiting for owner approval. No run before that.**

**Timebox:** 4 working sessions for U1–U10. The benchmark run is separate, after approval.
**Partial exit:** U1–U6 (engine + CLI) first, then U8–U9 (skill + blocked replay), then U7/U10 (profile, profile replay). Commit each green unit. If a unit is unfinished when the time runs out, README marks S2 `◐` and names it as the gap. Live recording is the riskiest step: if real OMP can't be recorded (auth, model behavior), that unit stays open and S2 stays `◐`. Never hand-write a fixture and call it a recorded turn.

**Owner-accepted 2026-10-07:** [ADR-0036](../decisions/0036-s2-evidence-freshness-and-turn-end-block.md): `reproduction` gates count historical evidence; a refused claim blocks at turn end and only `cedian workflow resume` unblocks; headless code state is a content hash (stand-in row H).

## Design

- **Code state (headless, row H).** `CodeState = Files(BTreeMap<path, u64>) | Tree(u64)`, as content hashes (FNV-1a, stable across processes). The CLI computes it when evidence is bound. If the bound call's args name workspace files (`read notes.txt`), it binds those files; otherwise (e.g. `bash cargo test`) it uses the tree fingerprint of workspace text files (no `.cedian/`, `.git/` or `target/`). **Born stale:** if a call that isn't read-only (anything except `read`, `grep`, `find`, `lsp`, the `cedian://` and `skill://` reads) finished after the bound call and before the report, the item is stored stale with that reason. Reporting late can't make old evidence look fresh.
- **Purity.** `Gate::evaluate(items, current: &dyn Fn(&CodeState) -> CodeState)`. Better as data: `evaluate(items, &CurrentState)`, where the channel builds `CurrentState` (path → hash, plus the tree hash) from the workdir *before* calling the engine. The engine never reads disk.
- **Outcome.** `Evidence.ok: bool` → `outcome`. On the wire `ok: true|false` stays an alias for `pass|fail`, so the recorded P5 fixture still replays. Gate rule: `require_ok` gates count only `pass`. Reproduction gates count `fail` (the bug shows). `inconclusive` counts for neither and is listed as such in the reason.
- **Claims ledger.** `cedian_complete {claims: [{text, label, evidence: [id]}]}`. A `measured` claim is *backed* only if every cited id is fresh, attributed and `pass`. Unbacked claims are flagged, never dropped. Missing claims don't block completion; the gates do that. The last completion attempt (claims + result + missing gates) is stored in `WorkflowState.last_completion` and rendered by `cedian workflow status`.
- **Turn end (ADR-0036).** `WorkflowState::end_turn()`: if `last_completion` was refused in this turn → `blocked`. `run_turn` calls it after the turn settles (prompt and shell). `cedian workflow resume` is the user's unblock.
- **Gate floor.** `GateFloor` is pure data: (kind, min risk) → gates. `WorkflowState::start(profile, &floor)` merges the floor gates in. `op=gate` lets OMP add a gate. It cannot drop, downgrade (`required: false`) or un-fresh a floor gate. The default floor is empty (fast lane). The config source is the user's `cedian.toml` (`[[workflow.floor]]`, landed with row E 2026-10-07, `docs/plans/done/row-e-cedian-toml.md`).
- **Verification profile.** `verification.rs`: parse `.omp/skills/verify-<app>/SKILL.md` (feature map = `- \`<id>\`: …` lines under `## Feature map`) — read, never written. `.cedian/verify.json` (`snapshot_version` 1) holds per profile `proven: bool` and per instance `{last_doctor_pass, last_bad_drive}` sequence numbers. `op=profile {profile, stage: launch|doctor|drive|evidence|cleanup, instance, ok, surprising?, from_tool, match?}` goes through the same resolver: unattributed stages are ignored. One attributed run of all five stages in order for one instance (doctor ok) → `proven`. `op=evidence` gains `feature` + `instance`. At bind time the outcome is forced to `inconclusive` if that instance is unhealthy (`last_bad_drive > last_doctor_pass`). A gate with `feature: Some(id)` counts only evidence with that feature, and only if the profile is proven. A draft profile → reason `profile verify-x is a draft`.

## Units (dependency order)

| # | Unit | Check (done when) | Box |
|---|---|---|---|
| U1 | `Evidence.outcome` + `CodeState`; `GatePredicate.fresh`; `Gate::evaluate(items, &CurrentState)` with stale / inconclusive rules | `cargo test -p cedian_workflow`: stale-after-edit not counted on fresh gate; stale repro still counts; inconclusive never passes; unattributed still rejected | 0.5 |
| U2 | Measurement fields + performance gate (`inconclusive` when any field is missing) | unit tests: full measurement passes, each missing field → inconclusive | 0.25 |
| U3 | Channel + CLI wiring: resolver returns code state and the born-stale reason; `CurrentState` built from workdir; `outcome` arg (+ `ok` alias); `cedian workflow evidence` always unattributed; `workflow.json` → v2 | unit: v1 file fails closed; channel test: evidence then edit → gate pending with `stale` reason; `replay_cli` P5 scenario still green | 0.5 |
| U4 | Claims ledger on `cedian_complete` + `last_completion` + status rendering | unit: backed measured, unbacked measured flagged, guess shown, stale citation → unbacked | 0.25 |
| U5 | Turn-end block (`end_turn`) in `run_turn`; `cedian workflow resume` | unit: refused-then-turn-end → blocked; resume → running, budget reset; CLI test | 0.25 |
| U6 | `GateFloor` + `op=gate` (add-only) | unit: floor gate survives, OMP cannot weaken it, OMP-added gate enforced; empty floor + no start → `complete` says nothing to check | 0.25 |
| U7 | Verification profile: parser, `verify.json` v1, `op=profile`, feature gates, instance health | unit: draft satisfies nothing; e2e run → proven; failed drive without doctor → inconclusive; doctor pass restores | 0.75 |
| U8 | Bug-fix playbook skill (`crates/cedian_cli/tests/fixtures/skills/bug-fix/SKILL.md`) + lint test: every op and tool it names exists in the channel schema | `cargo test -p cedian_cli` lint test green | 0.25 |
| U9 | **Exit replay (d):** record a live OMP turn in a temp workspace that carries the skill copy in `.omp/skills/`. The prompt only says "fix the bug in notes.txt using the bug-fix playbook". The headless denial of `bash` leaves `verify` unattributable. The model claims done → blocked | `cargo test -p cedian_cli --test replay_cli`: fixture shows OMP loaded the skill; `workflow.json` status `blocked`; output lists `verify`; no `.omp/` written by cedian outside the test's own copy | 0.75 |
| U10 | **Fast lane (e)** assertion on the existing recorded host-edit turn (no `workflow.json`), plus a profile replay (b) if recordable: a draft profile's feature gate stays unmet | replay green; the profile replay is a follow-up if it can't be recorded in the box | 0.25 |
| U11 | README S2 row (status), ROADMAP stand-in row H (no status), plan → `done/` when green | docs only | — |

## Benchmark tasks (ADR-0026) — PROPOSED, awaiting owner approval

Method: each history task starts in a fresh worktree at the commit's **parent**, with a one-paragraph task statement and no view of the commit. The predicate is checked after the run by applying the commit's own tests (hidden from the agent) plus `cargo test --workspace`. The two open-work tasks have predicates written up front. Same model in cedian and Cursor. Recorded per task: time-to-usable-result, false-done (claimed done, predicate fails), owner review time.

| # | Kind | Task (start point) | Done predicate (pass/fail) |
|---|---|---|---|
| B1 | bug fix | `resolve()` rejects `/notes.txt`-style keys even when they name an open buffer (`e98674d^`, `cedian_workspace`) | e98674d's `host.rs` tests pass: `/`-key with open buffer or workspace file resolves; a real path outside the workspace is still rejected |
| B2 | bug fix | OMP 18.6 runs host tools as `xd://<tool>` writes; the cards say "write" (`e98674d^`, `cedian_agent_ui`) | e98674d's `tool_card`/`message` tests pass: card named after the host tool; the docs read stays a read, never an edit |
| B3 | bug fix | Removing a worker runs `git branch -D` and destroys unmerged work (`6b0aac0^`, `cedian_worker`) | removing an unmerged worker returns `Err` and the branch still exists; a merged worker is removed |
| B4 | bug fix | `attach_lsp` deadlocks on the store lock (`6b0aac0^`, `cedian_workspace`) | `cargo test -p cedian_workspace` finishes under 60 s with the fake-bridge test passing |
| B5 | small feature | Router keeps a log of finished tool calls (only after `ToolEnd`, with `is_error`) (`264680f^`, `cedian_omp`) | 264680f's `event_router` tests pass |
| B6 | small feature | Headless answers OMP approval dialogs fail-closed instead of stalling to the prompt timeout (`378d623^`, `cedian_omp` + CLI) | 378d623's `headless_ui` tests pass and the `p5_worktree` replay (fixture supplied) passes |
| B7 | small feature | `workers.json` gets `snapshot_version`; a corrupt file is an error, not silently emptied (`09e88b6^`, `cedian_worker`) | 09e88b6's registry tests pass |
| B8 | refactor | `cedian workflow run` hand-parses kind/risk; reuse the serde names the channel already uses (open work, HEAD) | `cargo test --workspace` green; `cedian workflow run nope x` still errors listing every kind; no `"bug_fix" =>` arm left in `cedian_cli/src/main.rs` |
| B9 | test | Write the P7 parity test from `docs/OMP_PARITY.md` + vendored `wire.rs` (`3d66849^`) | the test passes on the ledger, fails naming `steer` when that row is renamed, and asserts parse floors (≥60 commands / ≥40 notifications / ≥10 UI requests) |
| B10 | docs-touching code | `browser.json` gets `snapshot_version` and fails closed on a mismatch, with the reset command in the error; README status line updated (`09e88b6^`, `cedian_cli` browser store) | 09e88b6's `browser_store` tests pass; a v0 `browser.json` gives an error naming the reset command; README mentions the versioned store |

Owner (2026-10-07): B10 was row E; swapped for P3's `browser.json` part, since row E is real work done before P8. B6 is kept as the one medium task. **The list still needs the owner's final approval before any run.**

## Findings (2026-10-07)

- **Kind-to-tool consistency was missing** (ADR-0031 had handed it to this slice): a `read` reported as `kind: test` would have passed `verify`. Evidence kind now follows the bound tool (exec → command/test, read-only → file, browser → browser/screenshot). The bug-fix `reproduce` gate accepts `file`, because reading wrong output reproduces a bug. The live recording hit exactly this: the model's verify evidence was a `read` labelled `test`.
- **The P5 replay already shows the S2 exit** under ADR-0036. Its recorded turn ends on a refused `cedian_complete`, so the workflow is now `blocked` (assertion updated).
- **Live S2 recording, OMP 18.6.1.** The model read `skill://bug-fix` from the workspace copy. It forgot `kind` on its first `start` (the error message told it). It tried `bash sh check.sh` and `eval`, and headless denied both. It reported the repro from a `read`, which was born stale because the denied `bash` came after it. Not harmful, since `reproduce` isn't fresh, but denied calls can't mutate anything: a later refinement could skip calls OMP never ran. It fixed the line via `cedian_apply_edit` (its first try used a wrong `expected_version`), then reported `verify` from a `read` (now `file`, not counted). It claimed `measured` citing e1+e2, and the claim was flagged (e1 stale). It then reported `inconclusive` twice (one unattributed: no finished `bash`), marked the verify phase failed, and ended. The turn end turned `failed` into `blocked`. Owner decided (ADR-0036): an agent-failed phase stays `failed`; the replay now asserts `failed` and the P5 replay covers `blocked`.
- Gate reasons used to say "has 0" or "rejects unattributed" when the real cause was the wrong kind of evidence. They now list `N stale, N inconclusive, N unattributed, N failing, N wrong kind`.
- `cedian browser shot --attach` was attributing headless captures to a fake `cli-browser-n` id. It's now unattributed `[headless-capture]`, as row D already required.

## Open after this pass

- Verification profile through an OMP turn: every profile stage is a `bash`-driven call in practice, and headless denies `bash`. Recording one needs `policy = "omp"` (P8) or a profile whose drive is read-only. Follow-up; S2 stays `◐` on (b).
- ~~Gate floor from `cedian.toml` (row E).~~ Done 2026-10-07: `replay_row_e_floor_from_cedian_toml` replays the recorded S2 turn with a floor rule; OMP's `op=start` workflow carries the floor gate.
- Benchmark run (after owner approval of B1–B10).
