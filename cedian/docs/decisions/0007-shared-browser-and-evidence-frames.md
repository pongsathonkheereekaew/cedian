# ADR-0007: One shared browser, user input preempts, evidence bound to frames

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §25, §26, §29, Phase 9 user-preempts rule (now in §25)

## Context

Two browsers (OMP's and the IDE's) split state. Shared tabs invite input races. A dropped screencast frame must never become evidence the user never saw.

## Decision

- One shared Chromium over CDP; user and OMP see the same tab. WKWebView is never the automation browser.
- Same-tab sharing is view-sharing, not input-sharing: user activity pauses the agent (`browser_input_preempted`) until it re-requests focus.
- Every screenshot evidence carries its CDP frame id; older-than-rendered = `stale-frame`, re-capture required for required gates. Presentation backpressure is latest-frame-wins — separate from RPC transport backpressure (ADR-0002).

## Consequences

Headless S4 uses a separate cedian Chrome (stand-in row D); its captures can't satisfy required browser gates until S9 unifies the browser.
