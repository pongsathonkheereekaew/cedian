# cedian verification map

This directory is the source for user-facing checks of cedian. Read this index, then follow one feature file. Launch starts `target/debug/cedian --cwd` and `target/debug/cedian-daemon` with `CEDIA_HOME` under `/tmp/verify-cedian/live/`. A drive that exits `1` with `ready=no` has not verified the feature.

## Baseline preconditions

- Run from the cedian repo root.
- Build `target/debug/cedian` and `target/debug/cedian-daemon` with `cargo build -p cedian_window -p cedian_daemon` when either is missing.
- Leave `CEDIAN_DATA_DIR` empty. The default is `/tmp/verify-cedian/data`. The binary does not read it.
- Never point `CEDIA_HOME` at `~/.cedia`, and never create `~/.cedia`.
- `DISPLAY` is set. On a Linux VM with no GPU, leave `VK_DRIVER_FILES` unset so launch selects the lavapipe ICD.
- The cedia pin is `crates/cedian_protocol/cedia/REV`. A checkout at `/home/ubuntu/cedia` is optional. Launch records its HEAD. It uses `bin/cedia` or `bun src/cli.ts` from that checkout when one of those runs, and otherwise `crates/cedian_daemon/tests/fake/cedia`.
- Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch`, then `doctor`, and require `home_cedia=absent` and `ready=yes`.

## Driving conventions

- Start every recipe from the launch command so one window and one daemon are up.
- Treat every command as literal.
- Run actions through `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh`.
- Do not exec `cedian` or `bin/cedia` yourself. Launch is the only start.
- Run `review-pane`, `composer`, `sidebar`, and `project-tab` before `editor-pane` and `terminal-pane`. Those two send keys that open more blocks.
- Run `agent-pane` before `spawn-cedia`. The spawn drive reads the prompt the agent drive sent.
- Cleanup removes the live directory, the daemon, and the window, and leaves `/tmp/verify-cedian/evidence/`.

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

- [Agent pane](./agent-pane.md) is the thread. A prompt draws the fixture text or the faux reply.
- [Editor pane](./editor-pane.md) is Zed's editor, opened with `ctrl+alt+shift+e`. It starts empty.
- [Spawn cedia](./spawn-cedia.md) is the daemon running one cedia process for the prompt.
- [Terminal pane](./terminal-pane.md) is the shell opened with `ctrl+alt+shift+t`. It dies with the window.
- [Review pane](./review-pane.md) is the snapshot diff. The default layout shows its header.
- [Desktop settings](./desktop-settings.md) is the `desktop` key in `$CEDIA_HOME/settings.json`.
- [Composer](./composer.md) is the prompt field under the thread.
- [Sidebar](./sidebar.md) is the thread list on the left of the default layout.
- [Project tab](./project-tab.md) is the tab named for the project directory.

## Also implemented

These are in the window source. This harness does not open them. A drive that only shows the default layout has not verified them.

- Files. `crates/cedian_window/src/file_tree.rs`. A click opens a file in the editor, read only.
- Git. `crates/cedian_window/src/git_pane.rs`. Read only, against git HEAD.
- Thread search, find file, and content search. `crates/cedian_window/src/thread_search.rs` and the keys in `crates/cedian_window/src/main.rs`.
- Worktree creation. The composer shows the toggle. The daemon creates the worktree in `crates/cedian_daemon/src/worktree.rs`. The prompt this harness sends sets `worktree` false.
- Archive and the archived list. `crates/cedian_window/src/root_view.rs`.
- Project chat. `crates/cedian_window/src/root_view/chat.rs`.
- Thread details and Markdown export. `crates/cedian_window/src/thread_details.rs`.
- Layout overview. `crates/cedian_window/src/overview.rs`.
- Questions above the composer while a thread is asking. `crates/cedian_window/src/root_view.rs`.
- Tear-off window. `crates/cedian_window/src/root_view/tear_off.rs`.

The tool menu marks Browser, Outputs, Memory, Usage, and Background as not built. `crates/cedian_window/src/tool_menu.rs` function `built`.
