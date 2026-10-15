# ADR-0019: Scope cuts: PR v1, local-only automations, out-of-scope list, daemon track

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §89; ROADMAP S6, S7

## Context

Market parity (Synara, Claude Desktop) pulls toward PR workspaces, scheduled runs, cloud sessions and survive-reboot daemons. Each is a second infrastructure if done fully.

## Decision

- PR workspace v1 = list, diff review on PR base, inline comments → Fix, CI status + bounded auto-fix, merge with explicit confirm. Fix-button grouping, stacked-PR position, pinned repos, safe-prefix merge are separate features.
- Automations are LOCAL only: cron expressions typed directly, same permission profile/gates/provenance as interactive runs, consecutive-failure limit 3. No NL→cron in v1, no wake-from-sleep promise.
- Out of scope: cloud/SSH sessions, second harness adapters, custom browser engine (Servo), Ghostty terminal, VM sandbox for `computer`.
- Daemon track deferred until real reboot-lost-work complaints after quit-resume ships.

## Consequences

Each cut returns only with its own acceptance criteria — never bundled into a half-done slice.

## Original research notes (verbatim from the 2026-10-06 plan)

> **Rationale (market parity, 2026-10-06).** Synara + Claude Desktop both ship browse/review/merge PRs + stacked PRs + CI auto-fix in-app. cedian's Review Changes (§§15–20) stops at the working tree — the PR is where review actually ships. Cheap to build: `gh` CLI + existing review pipeline, no new engine.

> **Ponytail cut (2026-10-06).** Dropped from v1: Fix-button (comment grouping), stacked-PR position/readiness, pinned repos, safe-prefix merge — each is a separate feature needing its own acceptance; bundling them guarantees a half-done phase.

> **Rationale + scope (user decision, 2026-10-06).** Synara ships scheduled recurring runs; cedian adopts a LOCAL-only version — no cloud runner, no remote queue. Justification: the machine stays on 24/7, so a local scheduler suffices and avoids an entire second infrastructure (server, auth, billing, remote sandbox).

> **Ponytail cut (2026-10-06).** Dropped from v1: NL→cron parsing (cron expr typed directly; OMP translation is its own feature with its own misparse risk), lid-closed guarantee (acceptance is "history + evidence visible on return from sleep", not a power-management promise — no `IOPMAssertion`, no wake-from-sleep; machine-on-24/7 is the user's setup, not cedian's contract).

> **Daemon track (future boundary, NOT scheduled).** Covers the last 10%: survive quit AND reboot like tern/herdr (engine as launchd daemon, not a child of the window). Trigger: real users complaining about reboot-lost work AFTER v0.4 quit-resume (§76) ships. Cost when triggered: §3 child→daemon+IPC redesign, §§71–76 lifecycle rewrite, IPC auth + grant expiry + stale-lock ownership (Synara 1.0.0 pattern), separate TCC/sandbox profile for the daemon, two-process dev loop. Do NOT start without the trigger — quit-resume covers 90%.
