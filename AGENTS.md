# cedian rules

- Read `docs/ARCHITECTURE.md` before changing anything; it is the only design source.
- Rust only. GPUI and Zed crates are pinned git dependencies, never vendored or forked.
- The daemon owns all state. The window is a viewer and holds nothing the daemon cannot rebuild.
- Every session is an append-only event log; never mutate past events.
- One crate per area (`cedian_<area>`), one file per concept, no `mod.rs`.
- No `unwrap()` outside tests; propagate errors with `?` or log them.
- A decision that changes `docs/ARCHITECTURE.md` updates it in the same change.
