# Desktop settings

cedian stores its settings in the `desktop` object of `$CEDIA_HOME/settings.json`. With `CEDIA_HOME` unset, that path is `~/.cedia/settings.json`. The daemon reads `desktop` and rewrites the file through a rename when one key changes. Every other top-level key stays, in the same order. The window reads the file once before the first frame, then applies the daemon's `settings` notices. It writes `layout` when the window moves. This drive plants `sandbox.mode` `off`, which this cedia pin accepts, and then sets `desktop.theme` to `light`.

## Sub-features

- `settings-read` leaves `desktop.layout` in the isolated file after launch.
- `settings-write` sets `theme` to `light` through the daemon's `set_desktop` request.
- `settings-isolated` keeps that file under `/tmp/verify-cedian/live/home` and leaves `sandbox.mode` at `off`. `~/.cedia` stays absent.

## How to get to it (user POV)

- Launch cedian with the helper. The live home already contains `{"sandbox":{"mode":"off"}}`.
- The window saves its bounds under `desktop.layout`.
- The settings drive sets the theme. The file still has `sandbox.mode` `off`.

## Driving it with verify-cedian

Preconditions:

- Launch has a live window and a daemon socket.
- Doctor prints `home_cedia=absent` and `ready=yes`.

- **Read and write.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive desktop-settings`. A passing run prints `settings_file=present`, `desktop_theme=light`, `sandbox=kept`, `layout=present`, and exits `0`.
- **Proof.** `/tmp/verify-cedian/evidence/desktop-settings.txt` has the record and `settings_path`. That path is `$CEDIA_HOME/settings.json`.

## Gotchas

- Do not create `~/.cedia/settings.json`. Launch deletes `~/.cedia` if the binary creates it, and exits `1`.
- API keys are not part of this journey. Do not write one into the file.
- The settings window is `crates/cedian_window/src/settings_window.rs`. This drive changes the theme through the daemon request that window sends. It does not click the Settings control.
