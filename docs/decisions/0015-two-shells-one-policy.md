# ADR-0015: Two shells, one policy

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §23, §24

## Context

OMP `bash` (headless, sandboxed) and the user's terminal PTY share one screen but are different execution contexts.

## Decision

Agent commands never execute in the user's PTY (rendered as-if-terminal from the sandboxed channel); user commands never inherit agent grants; the agent sees a scrubbed env (no `SSH_AUTH_SOCK`, no credential vars).

## Consequences

Grant scopes (`once|task|session`) bind to the agent context only.
