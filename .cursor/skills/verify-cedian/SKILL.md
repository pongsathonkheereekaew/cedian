---
name: verify-cedian
description: >
  Verify cedian, the macOS desktop for the cedia agent, by launching its
  binary, driving a pane or a cedia thread from an isolated data directory,
  and capturing evidence. Use when a change touches the window, the daemon,
  a pane, desktop settings, or how cedian runs cedia.
---

# Verify cedian

cedian is the native macOS desktop for cedia. The window is a viewer. The daemon owns the threads and runs one cedia process per prompt. The design source is `docs/ARCHITECTURE.md`. The README status is phase B: the workspace builds `cedian` (the window, which also replays recorded runs with `--replay`) and `cedian-daemon`. The window starts the daemon when nobody answers on its socket. The scripts below do not drive them yet.

You drive the scripts in this skill. You do not start a binary that would write `~/.cedia`. That directory is the owner's settings and cedia's thread files. This skill never creates it, never reads `settings.json`, and never points a data directory at it.

Two runs stay apart by using different `CEDIAN_DATA_DIR` values. Both stay under `/tmp/verify-cedian/`. A run that starts the window or the daemon also sets `CEDIAN_STATE_DIR` (the daemon's socket and log) and `CEDIA_HOME` (cedia's threads and sessions) under the same directory, so it never meets the owner's daemon or `~/.cedia`. `crates/cedian_daemon/tests/fake/cedia`, pointed at by `CEDIAN_CEDIA_BIN`, plays a recorded fixture instead of calling a model.

cedia itself is checked out by `.cursor/install.sh` to `/home/ubuntu/cedia` when the agent token can read `github.com/pongsathonkheereekaew/cedia`. This skill does not clone it and does not run it. Running cedia directly would skip the desktop.

## Launch

From the repo root:

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch
```

The script deletes `CEDIAN_DATA_DIR` (default `/tmp/verify-cedian/data`), creates it empty, and prints `key=value` lines. It does this from an empty directory on purpose. Override the directory with `CEDIAN_DATA_DIR` when a second run is already using the default.

Ready means stdout contains `ready=yes` and the exit code is `0`. A shell prompt is not ready. Today the script prints `ready=no` and exits `1`: the spike binary has no daemon and no data-dir flag yet.

The script starts no process. `pid_file=none` is the teardown handle. Run the cleanup command below when you are finished, including after a failed launch.

## Doctor

Doctor is read-only. Run it before the first drive, and again after any drive that exited non-zero.

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh doctor
```

Worth driving means `ready=yes`, `home_cedia=absent`, and `data_dir` is the directory you passed. The script exits `1` when it prints `ready=no`. Anything else means stop. Do not point the binary at the owner's home to make the check pass.

`cedia_checkout_state=missing` means `/home/ubuntu/cedia` is not a git checkout. The spawn-cedia journey cannot see a cedia child until that checkout exists and cedian can start it.

## Drive

The harness is `verify-cedian.sh drive <feature>`. Feature ids are `agent-pane`, `spawn-cedia`, `terminal-pane`, `review-pane`, and `desktop-settings`. Recipes are in `features/`.

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive spawn-cedia
```

The script prints the same record as doctor, plus the feature's end-state keys, and writes that record to the evidence file. It does not exec `cedian` or `cedia`. This repo has no data-dir flag, so an exec would use `~/.cedia`.

There is no dry-run mode and no model client in this repo. Nothing in the drive calls a model API. cedia's `-m faux` flag is cedia's, and this drive does not reach cedia, so you cannot treat a green drive as proof that faux mode skipped the network.

## Evidence

Proof is the command, the `key=value` record, and the exit code. The drive writes them to `$CEDIAN_EVIDENCE_DIR/<feature>.txt`. The default evidence directory is `/tmp/verify-cedian/evidence`.

A passing user journey would also show the side effect in a second view. For spawn-cedia that view is a thread file under the isolated data directory and a cedia child of the cedian process. The drive records `thread_file=absent` and `child_process=not-started` when it cannot produce them. Do not replace that with a cedia invocation you ran yourself.

`home_cedia=absent` is the check that the owner's state was left alone. `data_dir_entries=0` after launch means the empty directory gained no thread file.

No screenshot is available. The window does not start. The platform in `docs/ARCHITECTURE.md` is macOS on Apple silicon, and a Linux cloud VM cannot show the GPUI window even after a binary exists.

## Cleanup

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh cleanup
```

Cleanup kills the pid in `/tmp/verify-cedian/cedian.pid` when that file exists and the pid is alive. It then deletes `CEDIAN_DATA_DIR`. It does not delete the evidence directory, and it does not touch `~/.cedia`. After cleanup, the evidence file must still be at the path the drive printed.

## Helpers

The helper is executable.

```bash
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh launch
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh doctor
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive spawn-cedia
bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh cleanup
```

`CEDIAN_DATA_DIR`, `CEDIAN_EVIDENCE_DIR`, `CEDIAN_PID_FILE`, and `CEDIA_CHECKOUT` override the defaults. Keep every override outside `~/.cedia`.

When the desktop gains a real binary and a data-dir flag, update this skill with `/maintain-verification-skill`. Until then the map's end states stay unmet, and the gotchas stay as written.
