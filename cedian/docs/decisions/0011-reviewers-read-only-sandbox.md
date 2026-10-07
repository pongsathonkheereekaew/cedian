# ADR-0011: Reviewer agents are read-only, enforced by the OS

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §57–§60

## Context

"Reviewers generally receive read tools" is not enforcement — a reviewer subagent with `bash` can still write.

## Decision

Reviewer subagents run under a dedicated Seatbelt profile: `edit`/`write`/`computer`-actuate are `Deny` at the sandbox layer; `bash` is allow-listed to read-only prefixes in the same policy file as §64 (one owner, one CI gate). Outside the list → `Deny`, never `Ask`. A reviewer that needs a write files a finding.

## Consequences

This is S3's gate: profile + bypass-proof test before any reviewer runs.
- **Independent verdict (2026-10-06, from pstack):** a reviewer runs with a fresh context and, where OMP routing allows, a different model from the implementer. A finding is dismissed only with a recorded reason (audit log), never silently.
- **Refined by ADR-0041:** the reviewer is a separate OMP process cedian spawns at OMP's request, so the sandbox applies to it.
