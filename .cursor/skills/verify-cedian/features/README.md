# cedian verification map

This directory is the source for user-facing checks of cedian. Read this index, then follow one feature file. The app status in the README is planning. This checkout has no binary. A drive that exits `1` with `ready=no` has not verified the feature.

## Baseline preconditions

- Run from the cedian repo root.
- Set `CEDIAN_DATA_DIR` to an empty directory under `/tmp/verify-cedian/`. The default is `/tmp/verify-cedian/data`.
- Never set that directory to `~/.cedia`, and never create `~/.cedia`.
- cedia, when the install script could clone it, lives at `/home/ubuntu/cedia`. Do not run that checkout in place of cedian.
- Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh doctor` and require `home_cedia=absent`.
- `ready=yes` is required before a journey can pass. This checkout prints `ready=no`.

## Driving conventions

- Start every recipe from the launch command so the data directory is empty.
- Treat every command as literal.
- Run actions through `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh`.
- Do not exec `cedian` or `bin/cedia` yourself. The helper refuses for the same reason.
- Cleanup removes the data directory and leaves `/tmp/verify-cedian/evidence/`.

## Proof and skip reporting

- Capture the command, the `key=value` record, and the exit code.
- A passing journey also shows the side effect named in the feature file.
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

- [Agent pane](./agent-pane.md) is the thread, drawn as blocks, which is the primary surface.
- [Spawn cedia](./spawn-cedia.md) is the daemon starting one cedia process for that thread.
- [Terminal pane](./terminal-pane.md) is a shell in the same window.
- [Review pane](./review-pane.md) is the diff review against the task baseline.
- [Desktop settings](./desktop-settings.md) is the `desktop` key in cedia's settings file.
