# Desktop settings

cedian reads and writes the `desktop` key in `~/.cedia/settings.json`. cedia owns the rest of the file. Theme, font, keymap, layout, notifications, and browser live under that key. API keys stay out of the file.

## Sub-features

- `settings-read` shows the current `desktop` values in the settings UI.
- `settings-write` saves a change back to the same file.
- `settings-isolated` does the same write in the verification data directory, not in the owner's `~/.cedia`.

## How to get to it (user POV)

- Launch cedian.
- Open the settings UI.
- Change the theme. Quit and launch again. The theme is still the one you set.

## Driving it with verify-cedian

Preconditions:

- Launch has created an empty `CEDIAN_DATA_DIR` that is not `~/.cedia`.
- `home_cedia=absent` and `ready=yes`.
- The binary accepts that data directory. Without that flag, do not launch it.

- **Read and write.** Run `bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive desktop-settings`. A passing run prints `ready=yes`, writes the `desktop` key in the isolated settings file, and leaves `home_cedia=absent`.
- **Proof.** `/tmp/verify-cedian/evidence/desktop-settings.txt` has the command and the exit code. A second read of the isolated settings file shows the theme you set.

## Gotchas

- This checkout has no settings UI and no writer. The drive prints `settings_file=absent`, `desktop_key=unread`, `ready=no`, and exits `1`.
- Do not create `~/.cedia/settings.json` to give the journey a file. The owner's file is out of bounds, and a file you plant there is not the settings UI.
- API keys are not part of this journey. Do not write one into the settings file.
