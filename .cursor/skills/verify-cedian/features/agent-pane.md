# Agent pane

The agent pane is the thread. You read your prompt, the agent's blocks, and the thread state on the tab. cedian draws cedia's events. It does not run an agent loop of its own.

## Sub-features

- `agent-open` opens one thread as a pane of blocks.
- `agent-state` shows idle, working, done, failed, or stopped on the tab.
- `agent-reopen` shows the same thread after the window quits and comes back.

## How to get to it (user POV)

- Launch cedian. A headed start with no daemon running starts the daemon as its own process.
- Open a thread. The pane shows the thread's blocks.
- Quit the window and open cedian again. The same thread is still there.

## Driving it with verify-cedian

Preconditions:

- `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch` has created an empty `CEDIAN_DATA_DIR`.
- `home_cedia=absent`.
- `ready=yes` with a `cedian` binary that can take that data directory.

- **Open the pane.** Launch the desktop against the empty data directory. Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive agent-pane`. A passing run prints `ready=yes` and `window=started`, and the data directory contains the thread the pane is showing.
- **Proof.** The evidence file is `/tmp/verify-cedian/evidence/agent-pane.txt`. It names the feature, the command, and the exit code. The data directory's thread file is the side effect.

## Gotchas

- This checkout has no `Cargo.toml` and no `cedian` binary. The drive prints `ready=no`, `window=not-started`, and exits `1`. That is not a pass.
- The platform is macOS on Apple silicon. A Linux VM still cannot show the GPUI window after a binary exists.
- Do not open a recorded JSONL fixture by hand and call that the pane. The pane is the window.
