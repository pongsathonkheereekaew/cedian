# ADR-0018: Settings and policy live in one TOML file

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §64, §77; ROADMAP S9 (app shell) and S3 gate 3

## Context

The plan, the CI policy gate (ARCHITECTURE §64) and the settings UI all assume one permission TOML; the headless shell implemented `cedian.json`.

## Decision

`cedian.toml` is the single settings/policy file (permissions, reviewer allow-list, gate floor, automation schedules, update channel). The settings UI edits this same file. JSON stops being accepted before S3.

## Consequences

Migration of `cedian_shell` is part of S3's gate (stand-in row E).
