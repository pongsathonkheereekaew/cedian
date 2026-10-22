# cedian

The native macOS desktop for [cedia](https://github.com/pongsathonkheereekaew/cedia), in Rust.

cedia is the agent. A background daemon runs cedia for each prompt and owns the terminals, worktrees and shared browser, so work keeps running after the window closes. A native GPUI window draws them as panes: agent, terminal, file and editor, diff review, browser, with images and video shown inline. Settings live in cedia's `~/.cedia/settings.json`.

The design decisions are in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). The earlier version of cedian (a Zed fork driven by the OMP agent) is kept whole in the git history; its last commit is `f442670`.

## Status

Phase A. The window builds and runs: an agent pane beside Zed's editor and a shell from Zed's terminal crate. The agent pane plays recorded cedia runs; there is no daemon or live run yet.

```sh
cargo run -p cedian_window -- --replay crates/cedian_protocol/cedia/sessions/edit-and-check/stream.jsonl
cargo test --workspace

# a 10k-block thread for the scroll and memory budgets
cargo run -p cedian_agent --example long_thread -- 10000 > /tmp/long.jsonl
cargo run -p cedian_window -- --replay /tmp/long.jsonl --instant
```

On Linux, install the packages from Zed's `script/linux` first. On a Mac without full Xcode, add `--features runtime_shaders`.
