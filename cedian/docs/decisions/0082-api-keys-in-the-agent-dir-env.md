# ADR-0082: API keys live in the agent dir's `.env`, written by cedian at mode 0600

- **Status:** Accepted (owner, 2026-10-15, AskUserQuestion, rulings 35 to 37: API-key providers get a masked key field stored in the agent dir's `.env`, Keychain later; saving a key restarts the agent when idle, else after the turn. The writer's contract below is the agent's design, taken as the default.)
- **Date:** 2026-10-16
- **Rule text:** `crates/cedian_omp/src/api_keys.rs` (`ApiKey`, `EnvFile`, `API_KEY_PROVIDERS`), Settings > Account
- **Builds on:** ADR-0078 decision 5 (the agent keeps cedian's own storage)

## Context

Settings > Account offers a key field for providers the person pays directly. The bundled agent reads `.env` from its config dir (`PI_CODING_AGENT_DIR`) once, at start. Its RPC refuses secrets, and a key must never land in a log, a project file or a chat.

## Decision

1. **Where.** A key is one line `VAR=value` in `<agent config dir>/.env`. `VAR` is the name the agent's provider catalog reads (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`, `OPENROUTER_API_KEY`). Other lines in the file are kept as they are.
2. **How it is written.** cedian writes the whole file to a sibling created at mode 0600, syncs it and renames it over `.env`. The file is never readable by others and never half written. Remove drops the key's lines the same way.
3. **What a key may be.** Printable ASCII with no space, quote, `#`, `$` or `\`, so it fits one unquoted line and cannot add a second variable.
4. **Where a key never goes.** The `ApiKey` type has no `Serialize`, and its `Debug` shows only the last four characters. The page shows "•••• ends in xxxx". No RPC command carries a key.
5. **When it applies.** Saving or removing a key restarts the agent when no turn runs. During a turn the row says "Takes effect after this turn" and the restart follows the turn's end.

## Consequences

- The key sits in plain text on disk, readable by the person's account. The Keychain move is a later unit and replaces decision 1.
- Import from OMP never copies `.env`, so a key always comes from this page.
- A restart drops the agent's in-memory state the way Restart agent does.
