# Review pane

Review compares the worktree to the per-task baseline, not to git HEAD. A hunk you edited after the agent is STALE and is not rejected.

## Sub-features

- `review-open` opens the diff for the thread's task.
- `review-baseline` diffs against the task baseline.
- `review-stale` marks a hunk you changed after the agent as STALE.

## How to get to it (user POV)

- Finish a thread that edited files in its worktree.
- Open the review pane for that thread.
- Read the diff. Accept or leave the hunks.
- Edit a hunk yourself. The pane shows that hunk as STALE.

## Driving it with verify-cedian

Preconditions:

- Launch has created an empty `CEDIAN_DATA_DIR`.
- A thread in that directory has a task baseline and a worktree change.
- `home_cedia=absent` and `ready=yes`.

- **Open review.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive review-pane`. A passing run prints `ready=yes` and `window=started`, and the pane shows the baseline diff.
- **Proof.** `/tmp/verify-cedian/evidence/review-pane.txt` has the command and the exit code. The STALE hunk is the side effect that proves the baseline rule.

## Gotchas

- There is no thread and no worktree in the empty data directory. The drive prints `window=not-started`, `ready=no`, and exits `1`. Do not point review at the cedian git repo to force a diff.
- The STALE rule is unproven until the pane can show a hunk you edited after the agent.
