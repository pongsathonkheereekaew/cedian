# Spawn cedia

Sending a prompt is supposed to ask a daemon to run one cedia process (`cedia -p <text> --thread <id> --output-format stream-json`) that exits when the prompt is done. That command is the phase B contract in `docs/ARCHITECTURE.md`. Phase A does not run it. `crates/cedian_window/src/main.rs` accepts only `--replay`. The thread file would be cedia's to write. cedian only reads it.

## Sub-features

- `spawn-one` would run one cedia process for one prompt in a new thread.
- `spawn-isolated` would write that thread outside `~/.cedia`.
- `spawn-faux` would use cedia's faux provider so the proof does not call a paid model API.

## How to get to it (user POV)

- Launch cedian with an empty data directory that is not `~/.cedia`. The phase A window comes up on a recorded fixture.
- Look at the cedian process tree. There is no `cedia` child.
- Look in the data directory. There is no thread file. `~/.cedia` is still absent.

## Driving it with verify-cedian

Preconditions:

- `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch` has created an empty `CEDIAN_DATA_DIR` and a live window.
- Doctor prints `home_cedia=absent` and `ready=yes`.
- A pass also needs `/home/ubuntu/cedia` to be a git checkout that contains `bin/cedia`, and a cedian that starts it. This checkout has neither.

- **Watch the process tree.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive spawn-cedia`. A passing run prints `ready=yes`, `cedia_entrypoint=present`, `child_process=started`, and `thread_file=present`, and exits `0`.
- **This checkout.** The same command prints `cedia_entrypoint=missing`, `child_process=not-started`, and `thread_file=absent`, and exits `1`. The window can still be `window=started`. That is the unmet end state, not a missed flag.
- **Proof.** `/tmp/verify-cedian/evidence/spawn-cedia.txt` has the feature id, the command, the record, and `exit=1`.

## Gotchas

- `.cursor/install.sh` clones `https://github.com/pongsathonkheereekaew/cedia.git` into `/home/ubuntu/cedia`. With this agent's GitHub token the clone fails with `remote: Repository not found.` and exit `128`. Unauthenticated `gh` and `https://github.com/pongsathonkheereekaew/cedia` return HTTP 404. A clone with no credentials stops earlier, on `could not read Username for 'https://github.com'`, because `GIT_TERMINAL_PROMPT=0` refuses to hang. `repositoryDependencies` in `.cursor/environment.json` asks a future environment token to include cedia. It does not grant access to a token that was already issued.
- Do not run `bin/cedia -m faux` yourself and record that as this feature. That skips the desktop. The phase A window never reaches cedia, so faux mode stays unobserved.
- No model API key was required for this attempt, and none was read.
- There is no data-dir flag. The helper still refuses a data directory inside `~/.cedia`. The replay path does not write thread files there.
