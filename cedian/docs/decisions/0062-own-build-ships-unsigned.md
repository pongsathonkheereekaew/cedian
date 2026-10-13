# ADR-0062: The owner's own build ships unsigned

- **Status:** Accepted (owner, 2026-10-13, in chat: "apple signin ข้ามไปก่อนครับ เราทำใช้เองคนเดียว")
- **Date:** 2026-10-13
- **Rule text:** the fork's `script/cedian-bundle` (`CODESIGN_IDENTITY`), `script/cedian-icon`, `docs/plans/s9-real-app.md` U13, `docs/ROADMAP.md` S9 ("macOS signing / notarization owner"), the S9 "fully native" checklist (no signing item)
- **Builds on:** ADR-0054 (GPL-3.0-or-later), ADR-0058 decision 4 (U13 is the benchmark's release-build unit), ADR-0034 (branding is the fork's)

## Context

U13's exit needs a `cedian.app` that carries the bundled OMP, and the plan made signing and notarization part of it, which needs a full Xcode install and an Apple Developer identity — an owner step, so it sat open as the last S9 unit against everything else being green.

On 2026-10-13 the owner ruled on it: this build is the owner's alone ("we do it for ourselves"), so Apple signing is skipped for now. That is a real scope change: signing is the gate for shipping a bundle to anyone else, not for running one on the machine that built it.

## Decision

1. `script/cedian-bundle` produces the bundle unsigned, as it does today. `CODESIGN_IDENTITY` stays the one switch that signs when the owner sets an identity (it keeps `--options runtime` and a timestamp, which is what a notarized build would need).
2. No notarization, stapling or Apple Developer work in S9. U13's exit is met by an unsigned `cedian.app` that launches on the owner's machine with the pinned OMP inside it.
3. Gatekeeper's quarantine marks files that arrive by download, not files assembled locally, so a locally built `.app` opens with no `xattr` work. If the bundle is zipped and moved, `xattr -d com.apple.quarantine cedian.app` (or right-click → Open) is the workaround, and it is the person's own step, never cedian's.
4. arm64 needs every executable ad-hoc signed, and the toolchain does that at link time for the app and for the OMP sidecar; nothing here turns it off. So an unsigned build is not an unloadable one.
5. Handing the bundle to anyone else reopens this: a redistributed bundle needs the identity and notarization, plus the source offer and the bundled OMP's license that ADR-0054's `script/check-licenses` already names. That is a new ADR, not a silent build change.

## Consequences

- U13 closes without an Apple identity, and its row names signing as the thing a redistributed bundle needs rather than something S9 owes.
- CI needs no signing secrets, and no step in the repo can fail for a missing keychain profile.
- The build the owner runs is the build the tests cover: no signing step sits between a green check and a launchable app.
- A person other than the owner cannot be given this artifact as-is; the Gatekeeper path above is theirs to walk, and step 5 is the decision that would have to be taken first.
