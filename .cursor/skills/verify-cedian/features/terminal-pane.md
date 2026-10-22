# Terminal pane

The terminal pane is the login shell in the bottom right of the phase A window. `SpikeView` starts it with Zed's terminal crate and draws the cells as text lines. The shell is a child of the cedian process. Quitting the window kills it. There is no daemon, and the view does not take keystrokes.

## Sub-features

- `term-open` shows the shell's cells under the editor.
- `term-type` would run a command you type in the pane. This checkout sends no keys. The drive records `keys_sent=0`.
- `term-survive` would keep the shell after the window quits. Cleanup records `shell_after_quit=dead` when the shell dies with cedian.

## How to get to it (user POV)

- Launch cedian with the helper.
- Look at the bottom right of the window. The cells are the shell's own output.
- Quit through cleanup. The shell process is gone.

## Driving it with verify-cedian

Preconditions:

- Launch has created an empty `CEDIAN_DATA_DIR` and a live window.
- Doctor prints `ready=yes` and `home_cedia=absent`.

- **Open a shell.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive terminal-pane`. A passing run prints `ready=yes`, `window=started`, `shell_process=present`, and `keys_sent=0`, and exits `0`.
- **Quit.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh cleanup`. It prints `shell_after_quit=dead` and appends that key to the evidence file.
- **Proof.** `/tmp/verify-cedian/evidence/terminal-pane.txt` has the command, the shell pid, and `shell_after_quit`. `/tmp/verify-cedian/evidence/terminal-pane.png` is the screenshot.

## Gotchas

- A shell you start in your own terminal is not this pane. The child has to belong to the cedian pid.
- `keys_sent=0` is not a typed command. Do not send xdotool keys and call that `term-type`.
- `shell_after_quit=alive` means the shell outlived the window. That contradicts this checkout. Kill the leftover pid. Cleanup does that after it records the key.
