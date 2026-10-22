# Editor pane

The editor pane is Zed's editor in the top right of the phase A window. It opens on the spike line `cedian spike: Zed's editor in a GPUI window.` It does not open a file from the data directory.

## Sub-features

- `editor-open` shows that line in the editor above the shell.
- `editor-file` would open a file from the thread's worktree. This checkout sets the spike line in `SpikeView::new` and does not.

## How to get to it (user POV)

- Launch cedian with the helper. The same window as the agent pane shows the editor on the right, above the shell.
- Read the editor. The first line starts with `cedian spike`.

## Driving it with verify-cedian

Preconditions:

- Launch has created an empty `CEDIAN_DATA_DIR` and a live window.
- Doctor prints `ready=yes` and `home_cedia=absent`.

- **Open the editor.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive editor-pane`. A passing run prints `ready=yes`, `window=started`, and `ocr_editor=yes`, and exits `0`.
- **Proof.** `/tmp/verify-cedian/evidence/editor-pane.txt` has the record. `/tmp/verify-cedian/evidence/editor-pane.png` is the screenshot. OCR of that PNG contains `cedian spike`.

## Gotchas

- The editor and the terminal are one `SpikeView`. Quitting the window closes both.
- A passing drive does not claim `editor-file`.
- Do not treat `cargo test` as this pane. The pane is the window.
