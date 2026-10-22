# Terminal pane

The terminal pane is a shell the daemon owns. You type in it from the window. Closing the window does not kill the shell, because the daemon keeps it.

## Sub-features

- `term-open` opens a shell pane.
- `term-type` runs a command and shows its output in the pane.
- `term-survive` keeps the shell after the window quits.

## How to get to it (user POV)

- Launch cedian.
- Open a terminal pane.
- Run a command. The pane shows the output.
- Quit the window. The daemon still has the shell.

## Driving it with verify-cedian

Preconditions:

- Launch has created an empty `CEDIAN_DATA_DIR`.
- `home_cedia=absent` and `ready=yes`.

- **Open a shell.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive terminal-pane`. A passing run prints `ready=yes` and `window=started`, and the pane shows the command output.
- **Proof.** `/tmp/verify-cedian/evidence/terminal-pane.txt` has the command and the exit code. The shell's output in the pane is the side effect.

## Gotchas

- This checkout cannot open a window. The drive prints `window=not-started`, `ready=no`, and exits `1`.
- A shell you start in your own terminal is not this pane. The daemon has to own it.
- Linux is the wrong platform for the GPUI window. See `docs/ARCHITECTURE.md`.
