# cedian

The native macOS desktop for [cedia](https://github.com/pongsathonkheereekaew/cedia), in Rust.

cedia is the agent. A background daemon runs cedia for each prompt and owns the terminals, worktrees and shared browser, so work keeps running after the window closes. A native GPUI window draws them as panes: agent, terminal, file and editor, diff review, browser, with images and video shown inline. Settings live in cedia's `~/.cedia/settings.json`.

The design decisions are in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). The earlier version of cedian (a Zed fork driven by the OMP agent) is kept whole in the git history; its last commit is `f442670`.

## Status

Planning. No code yet.
