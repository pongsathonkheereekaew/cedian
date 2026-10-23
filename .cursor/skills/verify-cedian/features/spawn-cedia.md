# Spawn cedia

A prompt asks the daemon to run one cedia process. The argv is `--print=<text> --output-format stream-json --cwd <dir>`, then `--thread` only when the thread id is already known, then `--model` when the request sets one. The child is its own process group. The thread file is cedia's to write, under `$CEDIA_HOME`. cedian reads it and does not write a second copy.

## Sub-features

- `spawn-one` runs one cedia process for one prompt with `worktree` false.
- `spawn-isolated` writes that thread under the live `CEDIA_HOME`, not `~/.cedia`.
- `spawn-faux` uses the fixture stand-in, or a pinned cedia with `--model faux`, so the proof does not call a model API.

## How to get to it (user POV)

- Launch cedian with the helper. The window starts `cedian-daemon`.
- Send a prompt. The agent drive does that through the daemon socket.
- The process list shows `--print=make f.txt say good` and `stream-json` while the child is alive. The fake stand-in also writes that argv to `$CEDIA_HOME/projects/<slug>/args`.

## Driving it with verify-cedian

Preconditions:

- Launch and doctor have `ready=yes` and `home_cedia=absent`.
- `drive agent-pane` has already sent the prompt.

- **Read the argv.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive spawn-cedia`. A passing run prints `cedia_argv=print-stream-json`, `thread_file=present`, and `exit=0`.
- **Proof.** `/tmp/verify-cedian/evidence/spawn-cedia.txt` has the record. `/tmp/verify-cedian/evidence/cedia.argv` has the command line. `cedia_kind` is `pinned` or `fake`. `cedia_pin` is the REV file. `cedia_checkout_rev` is the checkout HEAD, or `missing`.

## Gotchas

- The pin is `crates/cedian_protocol/cedia/REV`. `.cursor/install.sh` clones the default branch and does not check out that commit or build `bin/cedia`.
- The fake stand-in's process name is `bash`, because it is a shell script. Match the argv, not the comm name `cedia`.
- Do not run `bin/cedia` yourself and record that as this feature. The daemon has to be the parent.
