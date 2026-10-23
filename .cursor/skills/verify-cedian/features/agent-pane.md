# Agent pane

The agent pane is one conversation of blocks folded from cedia stream-json. `cedian --cwd` shows it for the open thread after a prompt. `cedian --replay <stream.jsonl>` shows one recorded conversation and does not start the daemon. This drive uses the live window. cedian has no agent loop of its own.

## Sub-features

- `agent-open` shows the prompt text in the conversation.
- `agent-state` is the thread's state on the live window. The drive does not require a particular state word, because the labels are Thai and OCR misses them.
- `agent-reopen` would quit and open the same thread from cedia's files. This drive does not quit until cleanup.

## How to get to it (user POV)

- Build with `cargo build -p cedian_window -p cedian_daemon`.
- Launch with the helper. The window opens on an empty project.
- The agent drive sends `make f.txt say good`. The fake stand-in plays `crates/cedian_protocol/cedia/sessions/edit-and-check/stream.jsonl`. A pinned cedia answers with the faux line `say good`.

## Driving it with verify-cedian

Preconditions:

- `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch` has a live window and a daemon socket.
- `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh doctor` prints `ready=yes` and `home_cedia=absent`.

- **Open the pane.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive agent-pane`. A passing run prints `ready=yes`, `window=started`, `prompt=sent`, `ocr_prompt=yes`, and `thread_file=present`, and exits `0`.
- **Proof.** `/tmp/verify-cedian/evidence/agent-pane.txt` has the record. `/tmp/verify-cedian/evidence/agent-pane.png` is the 1280x800 window. OCR contains `say good`. The thread file is under `$CEDIA_HOME/projects/<slug>/threads`, not under `CEDIAN_DATA_DIR`.

## Gotchas

- `thread_file=present` is a file cedia or the fake stand-in wrote. `data_dir_entries=0` means `CEDIAN_DATA_DIR` stayed empty.
- Do not pass `--replay` for this drive. Replay does not start the daemon, so the prompt has nowhere to go.
- The drive does not call a model API.
