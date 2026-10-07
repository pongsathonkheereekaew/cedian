# cedian — cedian is a Cursor-style agentic IDE — the same agent + IDE workflow and product features — built for OMP: OMP runs as upstream ships it, every OMP feature gets a native surface, and Zed is forked only as far as OMP needs. OMP decides; cedian executes, renders, and verifies. (identity: ADR-0034; scope: ADR-0023)

Stack lock: Rust + GPUI only in this process (no TS/Electron/WebView). No TS anywhere: OMP is used as upstream ships it (ADR-0027). UI accelerators OK: `gpui-kit`, `elygpui.com` for panels/dialogs; editor/buffer/diff stay Zed-native.

Where things live (one home per fact):
- Status: `README.md` slice table ONLY, one cell = glyph + date + one sentence + link to the plan's "Exit evidence" (the narrative lives there; a run's decisions tsv is a log, never status). Gaps no exit names: ROADMAP "Follow-ups". Per-feature OMP coverage: `docs/OMP_PARITY.md` ONLY (ADR-0034). Build/test: `README.md` "Build / test".
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
- OMP runs only through the spawn profile — never a bare spawn (ADR-0020). Default: `--approval-mode write` + generated `--config` overlay (`computer.enabled: false`, eval gate) + scrubbed env. Per-project opt-in in the user's `cedian.toml` only: `policy = "omp"` hands approvals and `computer` to the user's OMP config; badge + audit always; never for reviewers or automations (ADR-0035).
- Every OMP feature (RPC command, agent event, UI request, tool, launch/config option) has a row in `docs/OMP_PARITY.md`; an OMP pin bump updates it in the same change; cutting a feature needs an ADR (ADR-0034).
- Zed is forked only as far as OMP needs: patch Zed core only to give an OMP capability a native surface or to bind a cedian crate; everything else stays upstream Zed (ADR-0034, ADR-0030).
- Only Zed is forked; OMP is never forked — cedian adapts to OMP: host tools, OMP skills, spawn profile, and importing OMP's disk writes as agent transactions (ADR-0022, ADR-0025, ADR-0027).
- Evidence is bound to code state (stale after a later edit), has outcome pass|fail|inconclusive, and completion takes a claims ledger (ADR-0024).
- Fast lane: a normal turn has no workflow/gates; rigor engages only by gate floor (kind × risk) or on request; inline edit + revert turn are core; speed is benchmarked, never claimed (ADR-0026).
- Stateful headless work runs in `cedian shell`; one-shot commands are store-backed only (ADR-0021).
- Pre-S9 stand-ins need a row in ROADMAP "Headless stand-ins"; ✅ only when the exit holds through an OMP turn or hermetic replay — ADR-0017.
- Timebox every slice + define partial-exit: commit what's green, mark the rest follow-up, never block.
- Desktop automation defaults to the CUA `cua-driver` contract; driver + Seatbelt profile + bypass-proof test land atomically, and until then `computer` is off by default, including as fallback (ADR-0008). The one exception is a project with `policy = "omp"` in the user's `cedian.toml`, where the user's OMP config decides (ADR-0035).
