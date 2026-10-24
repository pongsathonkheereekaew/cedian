# Terminal pane

The terminal is a `ShellPane`. The window starts it the first time a Terminal block is shown. The default layout has no terminal. On Linux `ctrl+alt+shift+t` opens it. On macOS the chord is `cmd+shift+t`. The shell is a pty child of the cedian pid. It dies when the window exits. The daemon is a different process and keeps running after the window quits. Cleanup kills the daemon itself.

## Sub-features

- `term-open` shows the shell and records its pid.
- `term-type` would type a command. This drive sends only the open chord. `keys_sent=1` is that chord, not a shell command.
- `term-survive` would keep the shell after the window quits. Cleanup records `shell_after_quit=dead` when the shell dies with the window.

## How to get to it (user POV)

- Launch cedian with the helper.
- Press `ctrl+alt+shift+t`.
- Quit through cleanup. The shell process is gone. The daemon is gone too, because cleanup kills it.

## Driving it with verify-cedian

Preconditions:

- Launch has a live window. Doctor prints `ready=yes` and `home_cedia=absent`.
- Run this after `agent-pane`, so the fixture stand-in's bash has already exited and is not counted as the shell.

- **Open a shell.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive terminal-pane`. A passing run prints `ready=yes`, `window=started`, `keys_sent=1`, and `shell_process=present`, and exits `0`.
- **Quit.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh cleanup`. It prints `shell_after_quit=dead` and appends that key to the evidence file.
- **Proof.** `/tmp/verify-cedian/evidence/terminal-pane.txt` has the command and the shell pid. `/tmp/verify-cedian/evidence/terminal-pane.png` is the screenshot.

## Gotchas

- A shell you start in your own terminal is not this pane. The child has to belong to the cedian pid, and its command line must not be the daemon or cedia.
- `shell_after_quit=alive` means the shell outlived the window. Cleanup kills that pid after it records the key.
- `shell_after_quit=no-shell` means the terminal drive did not record a pid.
