# ADR-0056: Inline edit is a turn in the live session, on ctrl-enter; revert turn from the editor

- **Status:** Accepted (owner, 2026-10-09, in chat: "ctrl-enter (ปุ่มของ Zed)", "ให้ cedian แทนที่", "ใน chat เดิม ต่อคิวถ้ากำลังรัน", "โมเดลของ chat ไปก่อน", "เฉพาะใน Review Changes", "palette และปุ่มลัด"; the rest decided by the agent as the safer option)
- **Date:** 2026-10-09
- **Rule text:** `docs/plans/s9-real-app.md` U10
- **Builds on:** ADR-0026 (fast lane: inline edit and revert turn are core), ADR-0034, ADR-0006, ADR-0050 decision 3

## Context

P6 built the inline edit headless, as `cedian shell edit <path> <range> <instruction>`. It ran as one turn in the shell's OMP session. The app has revert turn (U5), but only as a button in Review Changes, and it has no inline edit. Zed's own inline assistant (`ctrl-enter`) calls Zed's language models directly, not OMP. OMP 18.6.1 cannot run one prompt on another model or role: `set_model` takes an exact model id, `prompt` takes no model, and `--smol` applies only when a process starts.

## Decision

1. **ctrl-enter in an editor opens cedian's inline edit** (owner). It replaces Zed's inline-assistant binding in the editor, so there is one AI path through OMP. Zed's assistant stays reachable from the command palette.
2. **The surface is a cedian instruction block in the editor**, the same mechanism as the U9 finding blocks. Zed core is not patched (ADR-0034).
3. **An inline edit is a turn in the chat's live OMP session** (owner). It carries the selection and the instruction. The review records it as its own kind of turn, and revert turn treats it like any other turn. If a turn is running, the edit is queued as a follow-up with its chip (ADR-0050 decision 3) and runs after that turn.
4. **The model is the session's** (owner). The edit moves to OMP's `smol` role when an OMP pin can switch roles per prompt. ROADMAP Follow-ups already tracks this.
5. **Accept and Reject stay in Review Changes** (owner). The edit's hunks are ordinary agent hunks there.
6. **An edit outside the selection is shown with a warning**, as in P6, and is never rejected automatically.
7. **Revert turn from the editor** (owner): a command palette entry and a keybinding that collides with no Zed default. It reverts the latest turn, skips STALE hunks as U5 does, and is one undo.

## Consequences

- Zed's own inline assistant loses its key in editors.
- An inline edit adds to the chat's history and context.
- An edit asked for during a long turn waits for that turn to end.
