# ADR-0046: The app refuses a session another process drives, detected by who holds OMP's session files

- **Status:** Accepted (owner, 2026-10-08, in chat: "ไม่เปิด บอกเหตุ")
- **Date:** 2026-10-08
- **Rule text:** `docs/plans/s9-real-app.md` U4
- **Refines:** ADR-0040 decision 5

## Context

ADR-0040 decision 5 allows one driver per session. The app and OMP's CLI share OMP's session store (U3a), so both can reach the same session. Probes on OMP 18.6.1 (the pin) on 2026-10-08, with two RPC processes on one session directory:

- A second `open_session` on a session another live process holds resumes it: `resumed: true`, same session id, same file. OMP does not refuse. Both processes' turns append to one file, and the first process never sees the second's turns.
- The process that creates a session holds an exclusive `flock` on `~/.omp/run/session-owners/<sessionId>.lock`, released when it exits. A process that resumes a session never takes that lock, at open or after a turn.
- A process that has written to a session keeps the session `.jsonl` open for writing, created or resumed.
- `PI_CODING_AGENT_DIR` does not move `~/.omp/run`.

## Decision

1. **The app checks before it drives.** After its `open_session` resumes a session, and before each prompt, cedian lists the processes that have the session's lease file or `.jsonl` open. A pid other than the app's own OMP means another driver.
2. **On another driver the app refuses and says why.** It sends that session no prompt. The panel shows that the session is open in another OMP process and offers "Start a new session" (OMP `new_session`) and "Retry".
3. **The detection is OMP's own files, read from outside.** cedian holds no lock of its own: OMP's CLI would not honour it. The holder list comes from `lsof` behind one boundary function, so a future OMP that refuses a held session, or reports its owner over RPC, replaces it in one place.
4. **The refusal goes upstream.** `open_session` refusing (or reporting) a session another live process holds is an OMP request (ADR-0027).

## Consequences

- A process that resumed the session and has not written yet holds neither file, so the app cannot see it until its first write.
- The protection is one-directional. OMP's CLI still resumes a session the app drives; the app sees it only at its next check.
- One `lsof` run per open and per prompt.
