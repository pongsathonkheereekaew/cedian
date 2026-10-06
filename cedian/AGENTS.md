# cedian — OMP-native agentic IDE (Zed fork): cedian = environment, OMP = intelligence.

Stack lock: Rust + GPUI only in this process (no TS/Electron/WebView). TS lives in the OMP repo (`packages/coding-agent/src/`). UI accelerators OK: `gpui-kit`, `elygpui.com` for panels/dialogs; editor/buffer/diff stay Zed-native.

Where things live (one home per fact):
- Status: `README.md` slice table ONLY. Build/test: `README.md` "Build / test".
- Rules: `docs/ARCHITECTURE.md` (§NN numbers kept from the original plan; chapter 2 = must-not-build, chapter 17 = tests).
- Why: `docs/decisions/` ADRs — never edit an accepted decision, supersede it with a new ADR.
- Schedule + exit criteria + headless stand-ins: `docs/ROADMAP.md`.
- Per-slice implementation plans: `docs/plans/` (finished → `docs/plans/done/`). Old monolithic plan: `docs/archive/` (frozen, do not edit).

Before editing: read the ARCHITECTURE chapter for the area and the slice in ROADMAP.

Compressed index:
- One crate per area (`cedian_<area>`), one file per concept.
- Never duplicate OMP tool names; cedian implements the HOST side (`set_host_tools`, `cedian://` URIs, extensions/MCP) — ADR-0004.
- Review baselines are per-task (`clock::Global`), never git HEAD; accept = keep-buffer, reject = inverse patch, never on STALE — ADR-0006.
- Permissions strict-wins at the cedian gate (`Deny > Ask > Allow`) — ADR-0012. Gates: OMP gathers evidence, cedian checks (A1-narrow) — ADR-0010.
- Pre-S9 stand-ins need a row in ROADMAP "Headless stand-ins"; ✅ only when the exit holds through an OMP turn or hermetic replay — ADR-0017.
- Timebox every slice + define partial-exit: commit what's green, mark the rest follow-up, never block.
- Desktop automation = CUA `cua-driver` contract only; driver + Seatbelt profile + bypass-proof test land atomically — until then `computer` stays hard-disabled, including as fallback (ADR-0008).
