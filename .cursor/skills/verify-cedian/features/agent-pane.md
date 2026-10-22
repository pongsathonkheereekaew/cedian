# Agent pane

The agent pane is the thread, drawn as blocks. Phase A plays a recorded cedia stream into that pane with `cedian --replay`. cedian does not run an agent loop of its own. The header is the replay path. There is no tab and no thread file.

## Sub-features

- `agent-open` shows one recorded thread as blocks in the left half of the window.
- `agent-state` would show idle, working, done, failed, or stopped on a tab. This checkout has no tab. The drive records `agent_state=not-shown`.
- `agent-reopen` would show the same thread after the window quits. This checkout writes no thread file, so a later launch cannot restore one.

## How to get to it (user POV)

- Build cedian with `cargo build -p cedian_window`.
- Launch with the helper. It starts `cedian --replay` on `crates/cedian_protocol/cedia/sessions/edit-and-check/stream.jsonl`.
- Read the left pane. The prompt is `make f.txt say good`. The receipt shows `verified` when the fixture's evidence has no unverified paths.

## Driving it with verify-cedian

Preconditions:

- `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch` has created an empty `CEDIAN_DATA_DIR` and a live window.
- `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh doctor` prints `ready=yes` and `home_cedia=absent`.

- **Open the pane.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive agent-pane`. A passing run prints `ready=yes`, `window=started`, `ocr_blocks=yes`, `thread_file=absent`, and `agent_state=not-shown`, and exits `0`.
- **Proof.** The evidence file is `/tmp/verify-cedian/evidence/agent-pane.txt`. The screenshot is `/tmp/verify-cedian/evidence/agent-pane.png`. OCR of that PNG contains `say good` or `verified`. Tesseract often drops the dot in `f.txt`, so the check does not require the characters `f.txt`.

## Gotchas

- `Cargo.toml` is at the repo root. Before the binary exists, the drive prints `ready=no` and `reason=no cedian binary; cargo build -p cedian_window from the repo root`.
- Do not open the JSONL in an editor and call that the pane. The screenshot is the pane.
- `agent-state` and `agent-reopen` are not in this checkout. A passing drive does not claim them.
- The fixture is cedia's recorded faux run. The drive does not call a model API.
