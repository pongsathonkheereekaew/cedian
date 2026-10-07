# P6 — Revert turn + headless inline edit plan

Exit (ROADMAP P6, ADR-0026 decisions 2–3, made observable):
- **Revert turn:** after OMP turns change files, `revert-turn <n|last>` puts back every change turn `n` made in one action. Regions changed after that turn (by the user or a later turn) are skipped and listed as `STALE`, never overwritten. The revert is itself a turn, so reverting it redoes the original.
- **Inline edit:** in `cedian shell`, `edit <path> <start>-<end> <instruction>` runs one OMP turn with those lines as the explicit target; the result is an ordinary turn (reviewable, revertable). Changes outside the range are reported.
- Proven by a hermetic replay of recorded OMP turns in one shell session (§86). No OMP change.

**Timebox:** 2 working sessions. **Partial exit:** revert (pure + CLI) first, inline edit second; commit what is green, name the rest in README.

## Design

- **Turn log, not tool calls.** Revert must cover every file a turn changed, including `UNATTRIBUTED` ones that have no `AgentEdit`. So the review store gains `turns: Vec<TurnRecord>`: `{n, kind: prompt|edit|revert{of}, label, files: [{file, before, after}]}`, written by `run_turn` from the same pre/post disk snapshots it already takes. `review.json` → `snapshot_version` 2 (P3: older files fail closed).
- **Per-file revert is a 3-way line merge** (`cedian_review::revert`): hunks of `diff(before, after)` (what the turn did) are put back only where `diff(after, current)` (what happened since) does not touch them; touched hunks are `STALE`. Pure function, unit-tested.
- **Review stays coherent.** The revert writes disk, records a `revert` turn, and records an `AgentEdit` per file (`tool_call_id = revert-<n>`) so the tracker's "latest agent text" moves with it and reverted regions don't come back as `STALE`.
- **Inline-edit model.** ADR-0026 asks for OMP's `smol` role, but RPC has no role switch (only `set_model provider/id`) and the owner's OMP config sets no `smol`. Default: the session model; `--model provider/id` overrides for that one turn (then restored). Recorded as a finding.

## Tasks

| # | Task | Done when |
|---|---|---|
| T1 | `cedian_review::revert::revert_file(before, after, current)` | unit tests: clean revert, shifted by a later edit elsewhere, overlapping user edit → STALE, new file, deleted file |
| T2 | Turn log in the review store (v2) + `run_turn` writes it | `cargo test` green; store roundtrip test |
| T3 | `cedian turns` (read-only) + `cedian revert-turn <n|last>` | unit/CLI tests; revert of a revert redoes |
| T4 | Shell `edit <path> <start>-<end> <instruction> [--model p/id]` | out-of-range changes reported; `kind: edit` turn recorded |
| T5 | Recorded shell session → hermetic replay: inline edit turn + prompt turn, user edit on disk, `revert-turn` skips STALE, revert of the revert redoes | `cargo test -p cedian_cli --test replay_cli` green |

## Findings (2026-10-07 recordings)

- Asked to "use your edit tool", the model first picked `cedian_apply_edit` and got the byte offsets wrong (`alBETApha`); the out-of-range warning caught it. The inline-edit prompt now says to use OMP's own edit tool, not `cedian_apply_edit`, and the warning names `revert-turn n`.
- Ambient context rendered buffer keys as `/notes.txt`; OMP read that as an absolute path. Ambient now renders workspace-relative paths (`notes.txt`, `selection: notes.txt bytes a-b`).
- Latent P4 bug: `cedian shell` kept host buffers across turns and `open` never re-read disk, so turn 2's pre-turn snapshot (provenance + turn log) still held turn 1's start text. `BufferStore::reload` now takes the disk text before every turn (dirty buffers kept and reported).
- The P2 gap "OMP-native disk edits are not replayed" blocked this replay (the model used OMP's `edit`). fake-omp now records each tool's file writes as `fs` frames (snapshot at `tool_execution_start`, diff at `tool_execution_end`) and applies them on replay.
- No RPC model-role switch exists, and the owner's OMP config sets no `smol`: inline edit uses the session model unless `--model provider/id`.

## Outcome

All tasks green; `p6_revert.jsonl` replays the whole exit in one shell session.

## Exit evidence

README status row as of 2026-10-07, moved here so README keeps only the status:

✅ 2026-10-07 (hermetic replay of a recorded `cedian shell` session): `edit notes.txt 2-2 …` changes only its target line; `revert-turn 2` puts a prompt turn back but keeps the line the user rewrote since (STALE, listed); reverting the revert redoes it; `turns` lists them — finding: OMP has no RPC model-role switch, so inline edit uses the session model unless `--model provider/id` (ADR-0026 asked for `smol`)
