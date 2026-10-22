# Desktop settings

cedian is supposed to read and write the `desktop` key in `~/.cedia/settings.json`. cedia owns the rest of the file. Theme, font, keymap, layout, notifications, and browser live under that key. API keys stay out of the file. The settings UI is phase B in `docs/ARCHITECTURE.md`. This checkout has no writer. `settings::init` in the window is Zed's settings init, not that file.

## Sub-features

- `settings-read` would show the current `desktop` values in the settings UI.
- `settings-write` would save a change back to the same file.
- `settings-isolated` would do that write in the verification data directory, not in the owner's `~/.cedia`.

## How to get to it (user POV)

- Launch cedian. The phase A window has no settings UI.
- The isolated data directory stays empty. `~/.cedia` stays absent.

## Driving it with verify-cedian

Preconditions:

- Launch has created an empty `CEDIAN_DATA_DIR` that is not `~/.cedia`.
- Doctor prints `home_cedia=absent`.
- Do not launch a binary that writes the owner's settings file. This helper refuses that path.

- **Read and write.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive desktop-settings`. A passing run would print `ready=yes` and leave a `desktop` key in the isolated settings file. This checkout prints `settings_file=absent`, `desktop_key=unread`, and `exit=1`.
- **Proof.** `/tmp/verify-cedian/evidence/desktop-settings.txt` has the command and the exit code. `home_cedia` is still `absent`.

## Gotchas

- Do not create `~/.cedia/settings.json` to give the journey a file. The owner's file is out of bounds, and a file you plant there is not the settings UI.
- API keys are not part of this journey. Do not write one into a settings file.
- `window=started` means the phase A window is up. It does not mean a theme was saved.
