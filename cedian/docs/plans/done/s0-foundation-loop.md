# S0 — Foundation loop (exit record)

S0 had no plan file; it was built under the old monolithic plan. This file holds its exit evidence, so README keeps only the status.

## Exit evidence

Each item of the ROADMAP S0 exit, and the hermetic replay (`cargo test -p cedian_cli --test replay_cli`) that shows it:

| Exit item | Replay |
|---|---|
| `prompt → cards → edit → review → accept/reject` in the CLI | `replay_cli_host_edit_review_reject`; `replay_cli_user_edit_over_agent_hunk_is_stale` (accept) |
| Streamed text reaches the thread, not only `prompt_result` | `replay_cli_host_edit_review_reject`: one-shot output prints the thread's text (since 2026-10-07); with text deltas dropped from the thread, the replay fails |
| Review shows only task-attributed hunks | `replay_cli_host_edit_review_reject` ("one attributed hunk") |
| A user edit after the agent shows `STALE` | `replay_cli_user_edit_over_agent_hunk_is_stale`: review says `stale`, reject is refused, the user's line survives; with stale detection off, the replay fails |
| Accept/reject persist across invocations | both replays above: a later `cedian review` sees the resolution |
| No OMP disk edit is overwritten by a buffer write-back (row G) | `replay_p6_inline_edit_revert_turn`: turn 2's OMP-native `edit` lands on disk while the shell holds the buffer, and the file matches OMP's edit |
| A hermetic fake-omp replay covers router → thread | every replay above (P2) |

README status row as of 2026-10-07, before this record:

prompt → cards → edit → review → accept/reject in CLI; streaming text, task attribution + STALE, persisted resolutions fixed 2026-10-06; P1 spawn profile live-verified 2026-10-07 (project yolo/computer-on loses to the overlay); P2 hermetic replay 2026-10-07: `cedian prompt` → card → host-tool edit → `review` → `reject` from a recorded OMP turn ; OMP-native disk edits replay too since P6 (fake-omp records each tool's file writes as `fs` frames); `cedian shell` re-reads buffers from disk before every turn (stale buffers had skewed pre-turn snapshots); P4 `cedian shell` 2026-10-07: one runtime serves two turns + `.cedian/shell.lock` blocks mutating one-shots (hermetic replay), steer/abort mid-turn live-verified; headless CLI/shell answer OMP approval dialogs fail-closed (Deny at once, refusal printed) instead of stalling until the prompt timeout
