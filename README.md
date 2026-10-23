# cedian

The native macOS desktop for [cedia](https://github.com/pongsathonkheereekaew/cedia), in Rust.

cedia is the agent. A background daemon runs cedia for each prompt and owns the terminals, worktrees and shared browser, so work keeps running after the window closes. A native GPUI window draws them as panes: agent, terminal, file and editor, diff review, browser, with images and video shown inline. Settings live in cedia's `~/.cedia/settings.json`.

The design decisions are in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). The earlier version of cedian (a Zed fork driven by the OMP agent) is kept whole in the git history; its last commit is `f442670`.

## Status

Phase B. The window and a background daemon run real cedia threads: a thread list, the agent pane with a composer, and Zed's editor and a shell beside them. The window starts the daemon when it is not running; quitting the window leaves runs going.

```sh
cargo build -p cedian_daemon -p cedian_window
target/debug/cedian --cwd <project>    # finds cedia on the login shell's PATH, or $CEDIAN_CEDIA_BIN
cargo test --workspace

# a recorded run, without the daemon
cargo run -p cedian_window -- --replay crates/cedian_protocol/cedia/sessions/edit-and-check/stream.jsonl

# a 10k-block thread for the scroll and memory budgets
cargo run -p cedian_agent --example long_thread -- 10000 > /tmp/long.jsonl
cargo run -p cedian_window -- --replay /tmp/long.jsonl --instant
```

`target/debug/cedian-daemon install` registers the daemon as a LaunchAgent so it starts at login; `uninstall` removes it. The daemon logs to `daemon.log` beside its socket, in `~/Library/Application Support/cedian` on macOS. `CEDIAN_STATE_DIR` moves both, which keeps a test daemon apart from the real one.

`script/bundle-mac --cedia <compiled cedia> [--sign <identity>] [--dmg]` builds `Cedian.app` on a Mac; see the script for notarizing.

On Linux, install the packages from Zed's `script/linux` first. On a Mac without full Xcode, add `--features runtime_shaders`.

## License

All rights reserved. The source is public to read, but no one may use, copy, modify or distribute it without written permission from the owner. See `LICENSE`, and `NOTICE` for third-party parts under their own licenses.
