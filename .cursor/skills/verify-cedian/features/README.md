# cedian verification map

This directory is the source for user-facing checks of cedian. Read this index, then follow one feature file. The README status is phase A. `cargo build -p cedian_window` builds `target/debug/cedian`. Launch starts that binary. A drive that exits `1` with `ready=no` has not verified the feature. `spawn-cedia`, `review-pane`, and `desktop-settings` exit `1` with `ready=yes` because those panes are not in this checkout.

## Baseline preconditions

- Run from the cedian repo root.
- Build `target/debug/cedian` with `cargo build -p cedian_window` when it is missing.
- Set `CEDIAN_DATA_DIR` to an empty directory under `/tmp/verify-cedian/`. The default is `/tmp/verify-cedian/data`.
- Never set that directory to `~/.cedia`, and never create `~/.cedia`.
- `DISPLAY` is set. On a Linux VM with no GPU, leave `VK_DRIVER_FILES` unset so launch selects the lavapipe ICD.
- cedia, when the install script could clone it, lives at `/home/ubuntu/cedia`. Do not run that checkout in place of cedian.
- Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch`, then `doctor`, and require `home_cedia=absent` and `ready=yes`.

## Driving conventions

- Start every recipe from the launch command so the data directory is empty and one window is up.
- Treat every command as literal.
- Run actions through `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh`.
- Do not exec `cedian` or `bin/cedia` yourself. Launch is the only start.
- Cleanup removes the data directory and the window, and leaves `/tmp/verify-cedian/evidence/`.

## Proof and skip reporting

- Capture the command, the `key=value` record, and the exit code.
- A passing window journey also has the PNG named in the feature file.
- Record the feature id in the evidence file. The drive command does this.
- An unmet end state stays unmet. Say which command you ran and which key failed.
- Do not report a skipped entry point as verified through a different path.

## Feature entry contract

Each feature file starts with an H1 and one paragraph, then four H2 sections in order.

1. `Sub-features`
2. `How to get to it (user POV)`
3. `Driving it with verify-cedian`
4. `Gotchas`

## Features

- [Agent pane](./agent-pane.md) is the thread, drawn as blocks from a `--replay` fixture.
- [Editor pane](./editor-pane.md) is Zed's editor in the same window, showing the spike line.
- [Spawn cedia](./spawn-cedia.md) is the daemon running one cedia process for each prompt. Phase A does not do this.
- [Terminal pane](./terminal-pane.md) is the shell drawn under the editor. It dies with the window.
- [Review pane](./review-pane.md) is the diff review against the task baseline. It is not in this checkout.
- [Desktop settings](./desktop-settings.md) is the `desktop` key in cedia's settings file. It is not in this checkout.
