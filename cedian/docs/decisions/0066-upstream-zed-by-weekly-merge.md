# ADR-0066: The fork takes upstream Zed by a weekly merge, never a rebase

- **Status:** Accepted (owner, 2026-10-07 in the S9 plan's U2; recorded as an ADR 2026-10-14, in chat)
- **Date:** 2026-10-14
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §7 "Fork scope"; [ROADMAP.md](../ROADMAP.md) S9 "Fork Hygiene"; [S9 plan](../plans/s9-real-app.md) "Upstream sync (U2)"
- **Refines:** ADR-0030 (its "upstream rebases"), ADR-0034 (the fixed cadence it names)

## Context

ADR-0030 and ARCHITECTURE §7 say the fork "rebases on upstream Zed on a fixed cadence", but no ADR says how or how often. The owner settled both on 2026-10-07: merge, because `cedian/s9` is published, and weekly. That ruling lives only in the S9 plan, which moves to `done/` when S9 closes, and §7 and the ROADMAP still say "rebase".

The syncs so far have followed the ruling. The 2026-10-07 merge (fork `52a2e8fe0f`) brought in 35 upstream commits. PR #3 (fork `1305faed1f`, 2026-10-14) brought in 85, with no conflicts and one upstream API change reaching cedian's code. Both merges were small because they came a week apart. A fork that waits months between syncs takes the whole gap in one merge, and that is how a fork falls versions behind its upstream.

What makes a sync hard is the set of upstream files cedian patches, since a new `crates/cedian_*` file never conflicts. That set was six files at the first sync. After the rename (fork PR #7) it is 25 files, +683/−53 lines against upstream `main` (`git diff upstream/main...cedian/s9`, excluding `crates/cedian_*`, `vendor/`, `cedian/` and `script/cedian-bench`).

## Decision

1. **Merge, never rebase.** Upstream Zed `main` comes into `cedian/s9` as a merge commit, through a fork PR that is merged with a merge commit too, not squashed, so later syncs know which upstream commits are already in. A published branch is never rebased.
2. **Weekly, and before every release build.** A sync is not skipped because nothing seems to need it. A small merge every week costs less than a large one later.
3. **The agent that syncs resolves the conflicts.** A sync is done when `script/cedian-check` passes and the app builds (`cargo build -j4 -p zed --bin cedian --features gpui_platform/runtime_shaders`). If resolving a conflict means changing what one of cedian's patches to Zed core does, the owner decides.
4. **Every sync PR states the core footprint**: the count of upstream files cedian patches, measured as above, and whether the merge changed it. If the footprint grows, that shows in the sync PR, so a patch that could have been a `cedian_*` crate gets noticed.

## Consequences

- ARCHITECTURE §7 and ROADMAP S9 "Fork Hygiene" say merge, not rebase.
- The fork's history carries upstream merges. `git log --first-parent cedian/s9` shows cedian's own history, and `git diff upstream/main...cedian/s9` shows cedian's whole change against Zed.
- Keeping the footprint small stays the rule of ADR-0030 and ADR-0034. This ADR makes its size visible every week.
