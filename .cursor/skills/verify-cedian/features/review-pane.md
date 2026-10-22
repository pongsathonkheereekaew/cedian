# Review pane

Review compares the worktree to the per-task baseline, not to git HEAD. A hunk you edited after the agent is STALE and is not rejected. That behavior is phase B in `docs/ARCHITECTURE.md`. No crate in this checkout draws a review pane or reads a snapshot.

## Sub-features

- `review-open` would open the diff for the thread's task.
- `review-baseline` would diff against the task baseline.
- `review-stale` would mark a hunk you changed after the agent as STALE.

## How to get to it (user POV)

- Finish a thread that edited files in its worktree. This checkout cannot. The window only replays a fixture.
- Open the review pane for that thread. There is no such pane in the window.
- The phase A window stays on the agent, the editor, and the shell.

## Driving it with verify-cedian

Preconditions:

- Launch has created an empty `CEDIAN_DATA_DIR` and, when the binary exists, a live window.
- Doctor prints `home_cedia=absent`. `ready=yes` means the window is up. It does not mean review exists.

- **Open review.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive review-pane`. A passing run would print `ready=yes` and show a baseline diff. This checkout prints `review_pane=absent`, `baseline=absent`, and `exit=1`.
- **Proof.** `/tmp/verify-cedian/evidence/review-pane.txt` has the command and the exit code. There is no STALE hunk to show.

## Gotchas

- The empty data directory has no thread and no worktree. Do not point review at the cedian git repo to force a diff.
- `window=started` on this drive means the phase A window is up. It does not mean the review pane opened.
- The STALE rule is unproven until a pane can show a hunk you edited after the agent.
