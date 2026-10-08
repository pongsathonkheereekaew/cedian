# ADR-0048: The app runs one language server, Zed's; OMP's `lsp` tool is turned off only once Zed covers it

- **Status:** Accepted (owner, 2026-10-08: chose one language server, Zed's, then left the staging to the agent: "ใช้อะไรดีครับ")
- **Date:** 2026-10-08
- **Rule text:** `docs/plans/s9-real-app.md` U6; ARCHITECTURE §21, §22
- **Refines:** ADR-0034 (cutting an OMP feature needs an ADR)

## Context

ARCHITECTURE §21 asks for one LSP, Zed's. In the app today OMP starts its own language servers next to Zed's, so a Rust project runs two rust-analyzers on an 8 GB machine, and the agent's diagnostics come from the files on disk while the person sees diagnostics for their unsaved buffers.

What OMP 18.6.1's LSP gives the agent (read from `omp config list` on 2026-10-08):
- the `lsp` tool: definitions, references, diagnostics, rename (`lsp.enabled`, default on);
- diagnostics returned to the model after it writes a code file (`lsp.diagnosticsOnWrite`, default on), so it can fix its own errors in the same turn;
- formatting after a write (`lsp.formatOnWrite`, default off);
- servers start on first use (`lsp.lazy`) and are shared across OMP processes of one project (`lsp.shared`).
`--no-lsp` turns off the tool, formatting and diagnostics together.

## Decision

1. **One language server in the app, Zed's.** cedian serves the agent's code intelligence from Zed's project through `cedian://` URIs (host side, ADR-0004, ADR-0022): definitions, references, symbols, diagnostics, from the buffers the person sees.
2. **Staged.** The app keeps OMP's LSP until Zed covers what the agent loses: definitions, references, diagnostics, rename, and diagnostics after a write. Only then does the app's spawn profile add `--no-lsp`. U6 builds the first four reads; rename and diagnostics-after-write are a later unit (ROADMAP Follow-ups).
3. **Headless keeps OMP's LSP.** The CLI has no Zed project; its `cedian_lsp` stand-in is deleted (row B) and OMP's own `lsp` tool serves it.

## Consequences

- Until the switch, the app still runs two language servers for a project OMP edits.
- After the switch, rename is a cedian action on Zed's buffers, so it goes through review like any agent edit.
- `OMP_PARITY.md`: the `lsp` tool row says "replaced by Zed through `cedian://` once covered"; `--no-lsp` gets a row for the app profile when it lands.
