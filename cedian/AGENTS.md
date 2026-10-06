# cedian — an agentic IDE built in Rust + GPUI (Zed fork), with OMP as its only harness: OMP decides, cedian executes, renders, and verifies (scope: ADR-0023).

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
- OMP runs only through the spawn profile (`--approval-mode write` + generated `--config` overlay: `computer.enabled: false`, eval gate, scrubbed env) — never a bare spawn (ADR-0020).
- OMP reports to cedian through host tools; playbooks/verification profiles are OMP skills; TS in the OMP repo only as a last resort (ADR-0022, ADR-0025).
- Evidence is bound to code state (stale after a later edit), has outcome pass|fail|inconclusive, and completion takes a claims ledger (ADR-0024).
- Fast lane: a normal turn has no workflow/gates; rigor engages only by gate floor (kind × risk) or on request; inline edit + revert turn are core; speed is benchmarked, never claimed (ADR-0026).
- Stateful headless work runs in `cedian shell`; one-shot commands are store-backed only (ADR-0021).
- Pre-S9 stand-ins need a row in ROADMAP "Headless stand-ins"; ✅ only when the exit holds through an OMP turn or hermetic replay — ADR-0017.
- Timebox every slice + define partial-exit: commit what's green, mark the rest follow-up, never block.
- Desktop automation = CUA `cua-driver` contract only; driver + Seatbelt profile + bypass-proof test land atomically — until then `computer` stays hard-disabled, including as fallback (ADR-0008).
