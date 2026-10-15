# ADR-0080: the spawn overlay pins the agent's issue reports and update check off

- **Status:** Accepted (owner, 2026-10-15, AskUserQuestion: "the spawn overlay pins dev.autoqa false and startup.checkUpdate false (shown as Set by cedian on Privacy)")
- **Date:** 2026-10-15
- **Rule text:** `crates/cedian_omp/src/spawn_profile.rs` (`PRIVACY_PINS`), Settings > Privacy
- **Amends:** ADR-0020 (what the overlay sets), ADR-0035 (what `policy = "omp"` hands to the user's config)

## Context

The bundled agent (OMP 18.8.6) has two settings that send data off the computer by default:

- `dev.autoqa` (default `true`): tool-failure reports to `https://qa.omp.sh/v1/grievances`, after a first consent prompt.
- `startup.checkUpdate` (default `true`): an update check on every start. The bundled agent updates only with cedian (ADR-0075), so the check is useless and still contacts the network.

The Privacy page promises "By default, nothing leaves this computer". ADR-0020's overlay set neither key, and under `policy = "omp"` (ADR-0035) the overlay carries only cedian's denies.

## Decision

1. The overlay writes `dev.autoqa: false` and `startup.checkUpdate: false` under every policy: cedian's, the reviewer's, and `policy = "omp"`. The ADR-0035 opt-in is about approvals and desktop control, not about sending data away.
2. The list lives in one place, `PRIVACY_PINS`. The overlay applies it, and Settings > Privacy shows each entry read-only as "Set by cedian", so the page cannot drift from what the agent gets.
3. Adding a pin is a change to that list and this ADR's successor; removing one needs a new ADR.

## Consequences

- The person cannot turn the agent's issue reports on from cedian. A person who wants them can report through the agent's upstream project directly.
- The overlay under `policy = "omp"` is no longer only denies; the opt-in golden test names both pins.
