# Project tab

The project tab strip names the open project. Launch uses a directory named `project`, and the client window title is that name. `--cwd` brings that folder forward. The saved tab list lives in `desktop.layout.projects`.

## Sub-features

- `project-open` shows one tab named `project`.
- `project-switch` would open a second folder. This drive does not.

## How to get to it (user POV)

- Launch cedian with the helper.
- Read the title. It is the directory name `project`.

## Driving it with verify-cedian

Preconditions:

- Launch has a live window. Doctor prints `ready=yes` and `home_cedia=absent`.

- **Read the title.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive project-tab`. A passing run prints `window_name=project` and exits `0`.
- **Proof.** `/tmp/verify-cedian/evidence/project-tab.txt` has the record.

## Gotchas

- The window manager's frame is slightly larger than 1280x800. The id launch stores is the client window, whose geometry contains `1280x800`.
- This drive does not open a second project.
