# Composer

The composer is the field under the thread. Enter sends. Shift-Enter adds a line. The row under it shows where the thread runs, including the worktree toggle. The default layout draws this row. Replay hides it.

## Sub-features

- `composer-open` shows the field and the worktree toggle on the live window.
- `composer-send` is the prompt the agent drive sends through the same daemon request Enter uses. This drive does not press Enter.

## How to get to it (user POV)

- Launch cedian with the helper.
- Look under the agent column. The row names the worktree toggle, and the hint names Enter.

## Driving it with verify-cedian

Preconditions:

- Launch has a live window on the default layout.
- Doctor prints `ready=yes` and `home_cedia=absent`.
- Run this before the editor and terminal keys.

- **Read the row.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive composer`. A passing run prints `ocr_worktree=yes`, `ocr_enter=yes`, and exits `0`.
- **Proof.** `/tmp/verify-cedian/evidence/composer.txt` has the record. `/tmp/verify-cedian/evidence/composer.png` is the screenshot.

## Gotchas

- `ocr_enter=yes` is the hint text. It is not a sent message. `agent-pane` sends the prompt.
- The model chip is on this row. This drive does not change it, except that a pinned cedia prompt sets `model` to `faux`.
