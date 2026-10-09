# ADR-0054: cedian's code is GPL-3.0-or-later; vendored code keeps its own license

- **Status:** Accepted (owner, 2026-10-09, in chat: "แนะนำ license ให้เราเองเลยครับ", then "GPL-3.0-or-later ทั้งหมด")
- **Date:** 2026-10-09
- **Rule text:** ROADMAP Follow-ups (licensing), until built
- **Builds on:** ADR-0030 (fork layout), ADR-0042 (all code in the fork)

## Context

The cedian crates in the fork declare no license, except `cedian_panel` (GPL-3.0-or-later). Zed's `script/check-licenses` requires every first-party crate to declare `GPL-3.0-or-later` or `Apache-2.0` with a `LICENSE-GPL` or `LICENSE-APACHE` symlink, and fails on the cedian crates and on `vendor/omp-rpc` (MIT, upstream OMP's code). The app links Zed's GPL-3.0-or-later crates (`editor`, `project`, `workspace`, `zed`, `agent_ui`), so a distributed cedian binary is GPL-3.0-or-later whatever the cedian crates say. The fork is a public repository.

## Decision

1. Every `crates/cedian_*` crate declares `license = "GPL-3.0-or-later"` and carries the `LICENSE-GPL` symlink, as Zed's application crates do.
2. `vendor/omp-rpc` keeps OMP's MIT license; `script/check-licenses` names it as third-party code instead of failing on it.
3. pstack is the user's config, outside the fork, under its own MIT license (ADR-0051, ADR-0052).

## Consequences

- One rule for every cedian crate, and `script/check-licenses` can run in `cedian-check`.
- A crate later published for reuse outside a GPL program (the OMP client, for example) can be relicensed by the owner, who holds its copyright.
- This is not legal advice; a commercial distribution should be checked by a lawyer.
