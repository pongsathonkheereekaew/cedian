# Review pane

Review diffs the thread's first shadow-git snapshot to its last, not git HEAD. The header is `เทียบกับ snapshot แรกของ thread ไม่ใช่ git HEAD`. A hunk whose new lines are no longer on disk is STALE and cannot be rejected. The default layout puts Review in front of Session. With no open thread the pane says `ยังไม่ได้เปิด thread`. With a thread and no file changes it says `thread นี้ยังไม่ได้แก้ไฟล์ไหน`.

## Sub-features

- `review-open` shows that header on the default layout.
- `review-baseline` would list hunks from the snapshot git. This drive does not seed a shadow repository, so it does not require a hunk.
- `review-stale` would mark a hunk edited after the agent. The rule is in `crates/cedian_review`. This drive does not edit a worktree file.

## How to get to it (user POV)

- Launch cedian with the helper.
- Read the right column. Review is the front tab. The header names snapshot and git HEAD.

## Driving it with verify-cedian

Preconditions:

- Launch has a live window on the default layout. Doctor prints `ready=yes` and `home_cedia=absent`.
- Run this before `editor-pane` and `terminal-pane`.

- **Read the header.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive review-pane`. A passing run prints `ocr_snapshot=yes`, `ocr_baseline=yes`, `review_pane=shown`, and exits `0`.
- **Proof.** `/tmp/verify-cedian/evidence/review-pane.txt` has the record. `/tmp/verify-cedian/evidence/review-pane.png` is the screenshot.

## Gotchas

- `ocr_baseline=yes` means the PNG contains the text `git HEAD`. It does not mean a hunk was accepted or rejected.
- Replay draws the agent conversation alone and does not draw this block. This drive is the live window.
- Do not point the pane at the cedian git repo to force a diff.
