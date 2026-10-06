# Phase 0.5 RPC Spike

Minimal Rust host proving `omp --mode rpc-ui` interop. Throwaway by design —
the Phase 1 client lives in `crates/cedian_omp` and MUST reuse upstream
`sdk/rust/omp-rpc` (blocking transport + generated `wire.rs`), not this decoder.

```sh
cargo run -p rpc-spike -- --non-model  # no model calls (safe, cheap)
cargo run -p rpc-spike -- --full        # includes prompt/stream/abort/images (needs ambient OMP auth)
```

Uses ambient default OMP auth — never pin `--provider/--model` (keyed providers
fail without env keys). Results: `CAPABILITY_TABLE.md`.
