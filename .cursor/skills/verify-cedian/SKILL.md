---
name: verify-cedian
description: >
  Verify cedian, the macOS desktop for the cedia agent, by launching its
  binary, driving a pane or a cedia thread from an isolated data directory,
  and capturing evidence. Use when a change touches the window, the daemon,
  a pane, desktop settings, or how cedian runs cedia.
---

# Verify cedian

cedian is the native macOS desktop for cedia. The window is a viewer. The daemon owns the threads and runs one cedia process per prompt. The design source is `docs/ARCHITECTURE.md`. The README status is phase B: the workspace builds `cedian` (the window) and `cedian-daemon`, and the window starts the daemon when nobody answers on its socket. The scripts below drive only the window's replay mode: `cargo build -p cedian_window` produces `target/debug/cedian`, and `--replay` plays a recorded cedia run without starting the daemon, spawning cedia, or taking a data-directory flag.

You drive the scripts in this skill. You do not start a binary that would write `~/.cedia`. That directory is the owner's settings and cedia's thread files. This skill never creates it, never reads `settings.json` there, and never points a data directory at it. If a launch finds that the binary created `~/.cedia`, the script deletes that directory and exits `1`.

Two runs stay apart by using different `CEDIAN_DATA_DIR` values. Both stay under `/tmp/verify-cedian/`. Replay mode has no socket. A run that starts the window or the daemon outside replay also sets `CEDIAN_STATE_DIR` (the daemon's socket and log) and `CEDIA_HOME` (cedia's threads and sessions) under the same directory, so it never meets the owner's daemon or `~/.cedia`. `crates/cedian_daemon/tests/fake/cedia`, pointed at by `CEDIAN_CEDIA_BIN`, plays a recorded fixture instead of calling a model.

cedia itself is checked out by `.cursor/install.sh` to `/home/ubuntu/cedia` when the agent token can read `github.com/pongsathonkheereekaew/cedia`. This skill does not clone it. The phase A window does not exec it. Running `bin/cedia` yourself would skip the desktop.

## Launch

From the repo root, build once if `target/debug/cedian` is missing.

```bash
cargo build -p cedian_window
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch
```

The script deletes `CEDIAN_DATA_DIR` (default `/tmp/verify-cedian/data`), creates it empty, and starts `cedian --replay` on `crates/cedian_protocol/cedia/sessions/edit-and-check/stream.jsonl`. The replay path is absolute. Override it with `CEDIAN_REPLAY`. Override the data directory with `CEDIAN_DATA_DIR` when a second run is already using the default.

Ready means stdout contains `ready=yes`, a live `pid`, and exit code `0`. The script waits about 30 seconds while that process stays up. The 69-line fixture is played during the wait, one line every 40 ms, so the receipt is on screen before launch returns. A shell prompt is not ready.

On Linux the script exports `VK_DRIVER_FILES` to `/usr/share/vulkan/icd.d/lvp_icd.json` when that ICD exists and you have not set `VK_DRIVER_FILES` or `VK_ICD_FILENAMES`. Set one of those when the machine has a GPU. `DISPLAY` must already be set. The script does not start Xvfb.

If the binary is missing, launch prints `ready=no`, the reason names `cargo build -p cedian_window`, and the exit code is `1`. `pid_file` is `/tmp/verify-cedian/cedian.pid` after a start, and `none` when the window did not start.

## Doctor

Doctor is read-only. Run it after launch, before the first drive, and again after any drive that exited non-zero.

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh doctor
```

Worth driving means `ready=yes`, `home_cedia=absent`, and `data_dir` is the directory you passed. The script exits `1` when it prints `ready=no`. Anything else means stop. Do not point the binary at the owner's home to make the check pass.

`ready=no` with `reason=window is not running` means launch did not leave a live pid. Run launch again. Do not drive a window you have not health-checked since it last exited.

`cedia_checkout_state=missing` means `/home/ubuntu/cedia` is not a git checkout. The spawn-cedia journey cannot see a cedia child until that checkout exists and cedian can start it. Phase A cannot start it. `crates/cedian_window/src/main.rs` accepts only `--replay`.

## Drive

The harness is `verify-cedian.sh drive <feature>`. Feature ids are `agent-pane`, `editor-pane`, `spawn-cedia`, `terminal-pane`, `review-pane`, and `desktop-settings`. Recipes are in `features/`.

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive agent-pane
```

Drive uses the process launch started. It does not exec a second `cedian`, and it does not exec `cedia`. One window shows the agent pane, Zed's editor, and the shell.

`agent-pane`, `editor-pane`, and `terminal-pane` exit `0` when their phase A checks pass. `spawn-cedia`, `review-pane`, and `desktop-settings` exit `1` on this checkout. Their end states are not built yet. A non-zero drive of those three, with the window still alive, is the current result, not a crashed window. Doctor after them still prints `ready=yes`.

There is no dry-run mode and no model client in this repo. The fixture was recorded with cedia's faux provider. Nothing in the drive calls a model API.

## Evidence

Proof is the command, the `key=value` record, the exit code, and for the window features a PNG. The drive writes the record to `$CEDIAN_EVIDENCE_DIR/<feature>.txt`. The default evidence directory is `/tmp/verify-cedian/evidence`.

`agent-pane.png`, `editor-pane.png`, and `terminal-pane.png` are screenshots of the cedian window. OCR text sits beside them as `<feature>.ocr.txt` when a check needs a word from the pane. `cedian.log` is the binary's stdout.

A passing agent pane shows the fixture prompt in that screenshot. It does not write a thread file. `thread_file=absent` is the phase A result. A passing editor pane shows the text `cedian spike`. A passing terminal pane has a shell child of the cedian pid. `keys_sent=0` because the spike draws terminal cells and does not take keystrokes.

`home_cedia=absent` is the check that the owner's state was left alone. `data_dir_entries=0` after launch means the empty directory gained no thread file.

## Cleanup

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh cleanup
```

Cleanup kills the pid in `/tmp/verify-cedian/cedian.pid` when that file exists and the pid is alive. It then checks the shell pids recorded by the terminal drive. `shell_after_quit=dead` means the shell died with the window. `shell_after_quit=alive` means it outlived the window. The script then kills any shell that remained. It deletes `CEDIAN_DATA_DIR`. It does not delete the evidence directory, and it does not touch `~/.cedia` unless launch already removed a directory this run created. After cleanup, the evidence files must still be at the paths the drives printed. Cleanup appends `shell_after_quit` to `terminal-pane.txt` when that drive ran.

## Helpers

The helper is executable.

```bash
cargo build -p cedian_window
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh doctor
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive agent-pane
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh cleanup
```

`CEDIAN_DATA_DIR`, `CEDIAN_EVIDENCE_DIR`, `CEDIAN_PID_FILE`, `CEDIAN_BIN`, `CEDIAN_REPLAY`, and `CEDIA_CHECKOUT` override the defaults. Keep every override outside `~/.cedia`.

Phase A still has no daemon, no review pane, and no settings writer. Those three drives stay unmet until the code grows them. Update this skill with `/maintain-verification-skill` when that happens.
