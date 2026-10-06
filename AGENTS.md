# cedian — OMP-native agentic IDE (Zed fork): cedian = environment, OMP = intelligence.

Stack lock: Rust + GPUI only in this process (no TS/Electron/WebView). TS lives in the OMP repo (`packages/coding-agent/src/`). UI accelerators OK: `gpui-kit`, `elygpui.com` for panels/dialogs; editor/buffer/diff stay Zed-native.

Status: slice table in `README.md`. Plan §84 phase numbers are work-breakdown, not schedule. Build/test: `README.md` "Build / test".

Before editing: read the plan section for the area (`CEDIAN_AGENTIC_IDE_PLAN.md` §§1–12 architecture, §88 must-not-build, §84 roadmap, §86 tests). Grill/Scrutinize rulings live inline in the plan — follow them there, not here.

Compressed index (details in plan §§10/16/64):
- One crate per area (`cedian_<area>`), one file per concept.
- Never duplicate OMP tool names; cedian implements the HOST side (`set_host_tools`, `cedian://` URIs, extensions/MCP).
- Review baselines are per-task (`clock::Global`), never git HEAD; accept = keep-buffer, reject = inverse patch.
- Permissions strict-wins at the cedian gate (`Deny > Ask > Allow`).
- Timebox every slice + define partial-exit: commit what's green, mark the rest follow-up, never block.
- Desktop automation = CUA `cua-driver` contract only (plan §§9/64); driver + Seatbelt profile + bypass-proof test land atomically — until then `computer` stays hard-disabled, including as fallback.
