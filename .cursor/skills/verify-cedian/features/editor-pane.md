# Editor pane

The editor is one Zed `Editor` owned by the window. It starts empty. A file appears when a person opens one from Files or Git. The default layout is Threads, Agent, and Review beside Session, so the editor is not on screen until something opens it. `ctrl+alt+shift+e` opens or focuses the Editor block. The block title is `Editor` while no file is loaded.

## Sub-features

- `editor-open` shows the empty Editor block.
- `editor-file` would load a path into that buffer, read only. This drive does not pick a file.

## How to get to it (user POV)

- Launch cedian with the helper.
- Press `ctrl+alt+shift+e`. The Editor block opens. Its title is `Editor`.

## Driving it with verify-cedian

Preconditions:

- Launch has a live window. Doctor prints `ready=yes` and `home_cedia=absent`.
- Run this after the drives that read the default layout.

- **Open the editor.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive editor-pane`. A passing run prints `ready=yes`, `window=started`, `keys_sent=1`, `key=ctrl+alt+shift+e`, and `ocr_editor=yes`, and exits `0`.
- **Proof.** `/tmp/verify-cedian/evidence/editor-pane.txt` has the record. `/tmp/verify-cedian/evidence/editor-pane.png` is the screenshot. OCR of that PNG contains `Editor`.

## Gotchas

- The editor and the terminal are different blocks. Quitting the window closes both.
- A passing drive does not claim `editor-file`.
- There is no `cedian spike` line in this checkout.
