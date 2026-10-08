# ADR-0049: The app owns one Chromium per workspace; OMP attaches to it over CDP

- **Status:** Accepted (owner, 2026-10-08, in chat: "Profile แยกใหม่", "เมื่อ agent ใช้ครั้งแรก"; the rest decided by the agent as the safer option)
- **Date:** 2026-10-08
- **Rule text:** `docs/plans/s9-real-app.md` U7; ARCHITECTURE §25, §29 R4
- **Closes:** ROADMAP stand-in row D; refines ADR-0038 for S4

## Context

ARCHITECTURE §25 asks for one Chromium shared by the agent and the person, the same tab for both. Stand-in D (`cedian_browser`) starts a fresh headless Chrome per CLI command, so frame sequence numbers never carry across captures and `stale-frame` is never real (S4 exit).

OMP 18.6.1 (the pin), read from `omp config list` on 2026-10-08:
- `browser.cdpUrl`: an HTTP CDP endpoint to attach to instead of launching a browser;
- `browser.relay` (default on): drives the person's own Chrome tabs through an extension, and wins over `cdpUrl`;
- without `cdpUrl`, OMP runs its own browser; the browser is a Puppeteer prelude inside `eval`.
The fork has no web view, and the stack lock forbids one, so "the same tab" is a real Chromium window.

## Decision

1. **The app owns the browser.** One Chromium per workspace, launched by cedian, with its own profile in the workspace's state dir (ADR-0044) and a loopback debug port. Not the person's everyday Chrome profile (owner).
2. **Started on first use** (owner). cedian listens on the CDP port OMP is given and starts Chromium when OMP first connects; the panel has an "Open browser" button to start it by hand.
3. **OMP attaches to it.** The spawn overlay sets `browser.cdpUrl` to cedian's endpoint and `browser.relay: false`, so OMP neither launches its own browser nor reaches the person's Chrome. This holds under `policy = "omp"` too (ADR-0035 hands approvals and `computer` to OMP, not the browser).
4. **Frames are bound to one browser.** Captures carry a frame id and a sequence number that run across the whole browser lifetime, so evidence from an earlier frame reads `stale-frame` (§29 R4).
5. **The person's input wins** (§25). When the person acts in the shared window during a turn, cedian holds the agent's next browser call and shows a banner. OMP has no event for this, so it is enforced on cedian's side. A command already read by cedian's proxy when the hold starts can still reach Chromium, like one already in Chromium's queue; closing that window would mean holding a lock across a socket write, which can stall the app if Chromium stops reading.
6. **Later, not now:** an inline screencast pane in the app, and promoting the browser gate to a required gate (ROADMAP Follow-ups).

## Consequences

- The person logs in to test sites once in cedian's browser; those logins stay in the workspace's state dir.
- The headless CLI keeps no browser of its own once stand-in D is deleted; headless browser use is OMP's own.
- `OMP_PARITY.md`: rows for `browser.cdpUrl`, `browser.relay`, and the `browser` prelude.
