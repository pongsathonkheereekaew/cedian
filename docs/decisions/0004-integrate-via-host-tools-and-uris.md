# ADR-0004: Integrate through host tools and `cedian://` URIs — OMP has no backend seam

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §9, §10, §11, §12, §14, §39, §40

## Context

The original plan imagined swapping backends inside OMP's `edit`/`lsp`/`debug` tools. Upstream verification showed there is NO `WorkspaceBackend`/`LspHost`/`DapHost` seam, and no `set_host_services` command. What exists: `set_host_tools`, `set_host_uri_schemes`, extensions/MCP.

## Decision

- cedian implements the HOST side only: host tools (e.g. `cedian_apply_edit`) and the `cedian://` URI scheme. OMP tool names are never duplicated (`cedian_edit`, `zed_debug` are forbidden).
- Cedian-relevant edits route out of OMP through an OMP-side addition (§8) calling host tools; OMP's `EditTool` itself stays unchanged.
- "Host service" is a cedian-internal grouping, never a wire message.

## Consequences

- Until the OMP-side route lands, OMP writes the filesystem directly — handled by stand-in row G (ROADMAP).
- `set_host_tools`/`set_host_uri_schemes` replace the whole set: cedian always sends its complete set.
- **Amended by ADR-0027:** edits are not routed out of OMP by an OMP-side addition; OMP writes disk and cedian imports each write as an agent transaction.
