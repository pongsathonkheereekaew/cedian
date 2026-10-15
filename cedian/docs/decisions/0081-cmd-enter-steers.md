# ADR-0081: Cmd+Enter is the Stop-and-send chord

- **Status:** Accepted (owner, 2026-10-15, ruling 36, AskUserQuestion: "Queue is the default for messages sent during a turn; Cmd+Enter steers once")
- **Date:** 2026-10-16
- **Rule text:** `assets/keymaps/default-*.json` (`CedianPanel > Editor`), test `cmd_enter_steers_a_running_turn_once`
- **Amends:** ADR-0064 (point 4, the surface)

## Context

ADR-0064 point 4 binds Stop and send to `cmd-shift-enter` (`ctrl-shift-enter` on Linux and Windows), the chord upstream Zed uses for `agent::SendImmediately`. The composer is single-line, so `cmd-enter` had no job, and the "Send With" setting chose between two chords for one action. Ruling 36 makes queueing the default for a message sent during a turn and gives steering one obvious chord.

## Decision

1. `cmd-enter` (`ctrl-enter` on Linux and Windows) in the composer runs Stop and send. The `cmd-shift-enter` binding is removed.
2. Plain Enter sends. During a turn it follows `follow_up` (default `queue`). Enter while stopping is still refused.
3. The `SendMessage` action and the "Send With" setting are removed. Multi-line input is deferred; when it lands, its newline chord gets its own decision.

The rest of ADR-0064 stands, including the "Stop and send" button.

## Consequences

- People used to upstream Zed's `cmd-shift-enter` get nothing from it in the cedian panel.
- Multi-line composing must pick a chord that does not collide with `cmd-enter`.
