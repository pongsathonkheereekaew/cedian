---
name: verify-cedian
description: >
  Verify cedian, the macOS desktop for the cedia agent, by launching its
  binary, driving a pane or a cedia thread from an isolated data directory,
  and capturing evidence. Use when a change touches the window, the daemon,
  a pane, desktop settings, or how cedian runs cedia.
---

# Verify cedian

cedian is the native desktop for cedia. The window is a viewer. The daemon owns the threads and runs one cedia process per prompt. The design source is `docs/ARCHITECTURE.md`.

`cargo build -p cedian_window -p cedian_daemon` produces `target/debug/cedian` and `target/debug/cedian-daemon`. `cedian --cwd <dir>` opens that project and starts the daemon when nothing is listening. `cedian --replay <stream.jsonl>` plays a recorded run in the agent pane and does not start the daemon. This skill's launch uses `--cwd`, so one window can show the thread, the review block, the composer, and the sidebar.

You drive the scripts in this skill. You do not start a binary that would write `~/.cedia`. That directory is the owner's settings and cedia's thread files. Launch sets `CEDIA_HOME`, `CEDIAN_STATE_DIR`, and `CEDIAN_NO_LOGIN_ENV=1` under `/tmp/verify-cedian/live/`, so the daemon keeps this process environment and never reads the owner's login shell. `CEDIAN_DATA_DIR` stays an empty scratch directory. The binary does not read it. If a launch finds that the binary created `~/.cedia`, the script deletes that directory and exits `1`.

The cedia pin is the commit in `crates/cedian_protocol/cedia/REV`. `.cursor/install.sh` clones `https://github.com/pongsathonkheereekaew/cedia.git` to `/home/ubuntu/cedia`, checks out the remote default branch, and runs `bun install`. It does not check out the pin and it does not compile `bin/cedia`. Launch uses `bin/cedia` when that file is executable. Otherwise, when `bun` can run `src/cli.ts --version --json` from that checkout, launch writes a wrapper and the daemon uses it with `--model faux` and `CEDIA_SANDBOX=off`. Otherwise launch uses `crates/cedian_daemon/tests/fake/cedia`, which plays `crates/cedian_protocol/cedia/sessions/edit-and-check` and writes the argv the daemon passed. A prompt is `cedia --print=<text> --output-format stream-json --cwd <dir>`.

## Launch

From the repo root, build once if the binaries are missing.

```bash
cargo build -p cedian_window -p cedian_daemon
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch
```

Ready means stdout contains `ready=yes`, a live `pid`, `daemon_sock` pointing at a socket, and exit code `0`. The client area is 1280 by 800. Launch records that window id. A shell prompt is not ready.

On Linux the script exports `VK_DRIVER_FILES` to `/usr/share/vulkan/icd.d/lvp_icd.json` when that ICD exists and you have not set `VK_DRIVER_FILES` or `VK_ICD_FILENAMES`. `DISPLAY` must already be set. The script does not start Xvfb.

If `cedian` is missing, launch prints `ready=no` and the reason names `cargo build -p cedian_window`. If `cedian-daemon` is missing, the reason names `cargo build -p cedian_daemon`.

## Doctor

Doctor is read-only. Run it after launch, before the first drive, and again after any drive that exited non-zero.

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh doctor
```

Worth driving means `ready=yes`, `home_cedia=absent`, and `daemon_sock` is the socket under the live state directory. The script exits `1` when it prints `ready=no`.

`cedia_pin` is the REV file. `cedia_checkout_rev` is `git rev-parse HEAD` in `/home/ubuntu/cedia`, or `missing`. `cedia_kind=pinned` means the daemon will exec the checkout. `cedia_kind=fake` means it will exec the fixture stand-in.

## Drive

The harness is `verify-cedian.sh drive <feature>`. Feature ids are `agent-pane`, `editor-pane`, `terminal-pane`, `spawn-cedia`, `review-pane`, `desktop-settings`, `composer`, `sidebar`, and `project-tab`. Recipes are in `features/`.

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive agent-pane
```

Drive uses the process launch started. It does not exec a second `cedian`. `agent-pane` sends one prompt through the daemon socket, the same `prompt` request the composer sends, with `worktree` false, then opens that thread with `ctrl+k`. `editor-pane` and `terminal-pane` send `ctrl+alt+shift+e` and `ctrl+alt+shift+t`. Run `agent-pane` before `spawn-cedia`, and run `review-pane`, `composer`, `sidebar`, and `project-tab` before those keys. The keys split the largest pane, and Review stays on screen, so the drives that read the default columns run first.

There is no dry-run mode and no model client in this repo. The fake stand-in plays a recorded fixture. A pinned cedia run uses faux and does not call a model API.

## Evidence

Proof is the command, the `key=value` record, the exit code, and for a window feature a PNG. The drive writes the record to `$CEDIAN_EVIDENCE_DIR/<feature>.txt`. The default evidence directory is `/tmp/verify-cedian/evidence`.

`screenshot_scope=window` means the PNG is the 1280x800 client window. `cedia.argv` is the cedia command line captured while the prompt runs. `home_cedia=absent` means the owner's `~/.cedia` was left alone.

## Cleanup

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh cleanup
```

Cleanup kills the window pid and the daemon pid. The daemon is in its own process group, so killing the window does not stop it. `shell_after_quit=dead` means the terminal shell died with the window. The script then deletes `CEDIAN_DATA_DIR` and `/tmp/verify-cedian/live`. It does not delete the evidence directory, and it does not touch `~/.cedia` unless launch already removed a directory this run created.

## Helpers

The helper is executable.

```bash
cargo build -p cedian_window -p cedian_daemon
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh doctor
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive agent-pane
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh cleanup
```

`CEDIAN_DATA_DIR`, `CEDIAN_EVIDENCE_DIR`, `CEDIAN_PID_FILE`, `CEDIAN_BIN`, `CEDIA_CHECKOUT`, and `CEDIAN_CEDIA_BIN_OVERRIDE` override the defaults. Keep every override outside `~/.cedia`.
