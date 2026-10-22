# Spawn cedia

Sending a prompt asks the daemon to run one cedia process (`cedia -p <text> --thread <id> --output-format stream-json`) that exits when the prompt is done. The child is the bundled cedia, or the checkout at `/home/ubuntu/cedia` when you are proving the spawn against a local build. The thread file is cedia's to write. cedian only reads it.

## Sub-features

- `spawn-one` runs one cedia process for one prompt in a new thread.
- `spawn-isolated` writes that thread outside `~/.cedia`.
- `spawn-faux` uses cedia's faux provider so the proof does not call a paid model API.

## How to get to it (user POV)

- Launch cedian with an empty data directory that is not `~/.cedia`.
- Start a thread. The daemon's child list includes one `cedia` process for that thread.
- Send a prompt. With the faux provider, cedia answers without a model API key.
- Quit cedian. The thread file remains in the isolated data directory, and `~/.cedia` is still absent.

## Driving it with verify-cedian

Preconditions:

- `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch` has created an empty `CEDIAN_DATA_DIR`.
- `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh doctor` prints `home_cedia=absent`.
- `ready=yes`.
- `/home/ubuntu/cedia` is a git checkout that contains `bin/cedia`, or the cedian binary under test is bundled with a cedia build.

- **Start one thread.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive spawn-cedia`. A passing run prints `ready=yes`, `cedia_entrypoint=present`, `child_process=started`, and `thread_file=present`.
- **Confirm the side effect.** The isolated data directory contains the new thread file. `home_cedia` is still `absent`. `data_dir_entries` is not `0`.
- **Proof.** `/tmp/verify-cedian/evidence/spawn-cedia.txt` has the feature id, the command, the record, and `exit=0`.

## Gotchas

- The drive does not reach this end state. On this checkout it prints `ready=no`, `cedia_checkout_state=missing`, `cedia_entrypoint=missing`, `child_process=not-started`, `thread_file=absent`, and exits `1`.
- `.cursor/install.sh` clones `https://github.com/pongsathonkheereekaew/cedia.git` into `/home/ubuntu/cedia`. With this agent's GitHub token the clone fails with `remote: Repository not found.` and exit `128`. Unauthenticated `gh` and `https://github.com/pongsathonkheereekaew/cedia` return HTTP 404. A clone with no credentials stops earlier, on `could not read Username for 'https://github.com'`, because `GIT_TERMINAL_PROMPT=0` refuses to hang. `repositoryDependencies` in `.cursor/environment.json` asks a future environment token to include cedia. It does not grant access to a token that was already issued.
- Do not run `bin/cedia -m faux` yourself and record that as this feature. That skips the daemon. What faux mode skips is also unobserved until a cedian child actually starts.
- No model API key was required for this attempt, and none was read. A journey that needs a live provider stays unverified until cedian can spawn cedia with one. Do not pick a model name to paper over that.
- There is no data-dir flag in this repo. The helper does not exec a binary that would default to `~/.cedia`.
