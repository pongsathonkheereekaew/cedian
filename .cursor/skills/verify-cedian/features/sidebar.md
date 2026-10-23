# Sidebar

The sidebar lists the project's threads. On the default layout its header reads `Threads`. A new project shows the new-thread row. Replay does not draw this block.

## Sub-features

- `sidebar-open` shows the Threads header.
- `sidebar-pin` and `sidebar-archive` are row actions. This drive does not hover a row.

## How to get to it (user POV)

- Launch cedian with the helper.
- Read the left column. The header is `Threads`.

## Driving it with verify-cedian

Preconditions:

- Launch has a live window on the default layout.
- Doctor prints `ready=yes` and `home_cedia=absent`.
- Run this before the editor and terminal keys.

- **Read the header.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive sidebar`. A passing run prints `ocr_threads=yes` and exits `0`.
- **Proof.** `/tmp/verify-cedian/evidence/sidebar.txt` has the record. `/tmp/verify-cedian/evidence/sidebar.png` is the screenshot.

## Gotchas

- Thai group names are on this list. OCR often misses them. The drive requires the English header `Threads`.
- A thread created by `agent-pane` can show up here. This drive does not require that row.
