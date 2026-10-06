# ADR-0002: OMP runs as a child process over RPC v2

- **Status:** Accepted
- **Date:** 2026-10-06
- **Rule text:** [ARCHITECTURE.md](../ARCHITECTURE.md) §3, §4, §5, §29 (upstream fact), §71, §72, §73

## Context

Embedding OMP in the GPUI process would let a provider hang, tool deadlock or OMP crash take down the editor. Upstream already ships an RPC mode and a generated Rust client — but it is BLOCKING, and its stdout backpressure is a disk spool, not credits.

## Decision

- Spawn `omp --mode rpc-ui` as a child; speak RPC v2 through the vendored `omp-rpc` client.
- Blocking client calls run on dedicated threads, bridged to GPUI async; never on the UI thread.
- The EventRouter reader drains continuously; backpressure is applied at the GPUI batching layer (§73), never by pausing the read. Spool failure (`exit 1`) is its own crash class.
- Startup steps 9–10 (subagent subscription, event filters) are typed calls; unknown events are tolerated and counted; filter schema mismatch fails startup.

## Consequences

- Crash isolation and independent restarts come for free (§74).
- cedian must never assume bounded memory on the transport.
- Regenerate `wire.rs` on every OMP pin bump.

## Original research notes (verbatim from the 2026-10-06 plan)

> **Upstream fact (oh-my-pi `sdk/rust/omp-rpc`, verified 2026-10-06).** The real crate exposes a BLOCKING client (`Client::spawn`/`prompt_and_wait` on threads, v2 auto-negotiation, host tools/URIs dispatched on handler threads, SIGTERM→1s→SIGKILL teardown), types generated from `wire/rpc-wire.schema.json` via `bun run gen:rpc`. There is no `async omp_rpc::Client` — cedian MUST bridge blocking handler threads to GPUI async (never the UI thread). Staleness is CI-enforced upstream (`bun check`, `test/rpc-wire`); cedian pins the schema revision alongside `vendor/omp-revision` and re-runs generation on bump.
