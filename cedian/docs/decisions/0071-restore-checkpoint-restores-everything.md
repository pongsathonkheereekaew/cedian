# ADR-0071: "Restore checkpoint and branch" restores every later agent change

- **Status:** Accepted (owner, 2026-10-14, AskUserQuestion on S10 U2: chose "สองปุ่ม restore ทุกอย่าง" over the agent's recommendation to skip accepted hunks; the option named the risk of overwriting the person's own later edits)
- **Date:** 2026-10-14
- **Rule text:** `docs/plans/s10-omp-parity.md` U2; the fork's `cedian/OMP_PARITY.md` rows `branch`, `fork`
- **Refines:** ADR-0006 (accept keeps the buffer, reject is the inverse patch, never on STALE), for this one action only; ADR-0026 (revert turn) is unchanged

## Context

On OMP 18.8.6, `branch {entryId}` never moves the current point inside a session. It writes a new child session holding the path up to that user message's parent and switches OMP to it. The files on disk stay as the later turns left them. Cursor's checkpoints restore the files too. ADR-0006 lets cedian undo only pending agent hunks, and never a STALE one, so a branch alone could not give the person the files as they were at that message.

## Decision

1. **Two actions on an earlier user message.** "Branch here" switches OMP to the new branch and leaves files and the review alone. A note counts the agent changes from later turns that stay. "Restore checkpoint and branch" first restores the files to how they were before the later turns, then branches.
2. **Restore covers every later agent change** made by the turns after that message: pending, accepted and STALE hunks alike. An accepted hunk is restored by its inverse patch. A STALE hunk is restored to the agent's baseline for that region, which overwrites what the person typed there after the agent's edit.
3. **The person sees the cost before it happens.** A confirmation names each file, the number of accepted hunks, and every STALE hunk whose own later edits would be lost. Nothing happens until the person confirms.
4. **One undo.** The whole restore is one transaction per buffer, grouped so that one undo brings everything back, the person's overwritten text included. Files that are not open are opened as buffers for the restore, so the undo covers them too.
5. **Only agent changes.** Edits the person made outside agent hunks, and files no agent turn touched, are never changed. Bash writes count as agent changes when ADR-0047 attributed them.

## Consequences

- The checkpoint behaves like Cursor's, and the person can go back to the message and its files in one action.
- ADR-0006's "never on STALE" still holds for reject and revert turn. This action is the only exception, and it names what it overwrites before it runs.
- U2 needs the link from each OMP user message to its review turn (the design reads `get_branch_messages` at each `agent_end`). Without that link the action is not offered.
