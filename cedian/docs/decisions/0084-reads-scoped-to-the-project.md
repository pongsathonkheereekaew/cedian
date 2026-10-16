# ADR-0084: The agent's read tools see the project; anything outside asks, protected paths are denied

- **Status:** Accepted (owner, 2026-10-15, ADR-0077 decision 4 is the rule; the mechanism below is the agent's design, taken as the default)
- **Date:** 2026-10-16
- **Rule text:** fork `packages/coding-agent/src/tools/read-scope.ts` (`tools.readScope`), `crates/cedian_omp/src/spawn_profile.rs` (`ReadScope`, `READ_TOOLS`), `crates/cedian_shell/src/launch.rs` (`read_scope`), Settings > Permissions
- **Implements:** ADR-0077 decision 4
- **Builds on:** ADR-0012 (strict-wins, protected metadata paths), ADR-0020 (the spawn overlay), ADR-0035 (`policy = "omp"`), ADR-0044 (state outside the workspace), ADR-0075 (the narrow fork)

## Context

OMP's `read`, `grep`, `glob`, `find`, `ast_grep` and the read half of `lsp` declare the `read` tier, which every approval mode auto-approves, so the agent reads any file the person can. ADR-0077 decided reads are scoped to the project and anything outside asks. SET-4 stored the folders the person allows ahead of time (`[projects."<path>"] read_folders` in the user's `cedian.toml`, read through `Settings::read_folders_for`) and the Safe actions tier, but nothing applied either.

Two places could check a path. OMP sees it first, in each tool's `approval(args)`, before the call runs; cedian sees it only once OMP raises a dialog, and never for a call OMP auto-approved. A check in cedian alone would have to intercept every read call over RPC, which the protocol does not offer, so the check lives in the fork and cedian's gate stays the one that asks and audits.

## Decision

1. **The fork checks, cedian asks.** A new setting, `tools.readScope`, carries the scope into the agent through the spawn overlay (ADR-0020). Each read tool's `approval(args)` resolves every path it is about to touch against it and returns the decision the resolver already understands: a tool `deny`, the plain `read` tier, or a forced prompt (`override: true`) with the path in the reason. No read tool changes what it does once allowed. With no `tools.readScope` configured the tools behave as upstream, so the fork's CLI and its tests are unchanged.
2. **The shape.**
   ```yaml
   tools:
     readScope:
       roots: ["/Users/me/src/app"]        # the project; reads inside pass
       allow: ["/Users/me/src/shared"]     # read_folders from the user's cedian.toml
       protectedNames: [".git", ".cedian", ".omp"]   # denied under any root or allowed folder
       protectedPaths: ["/Users/me/.omp", "<agent config dir>", "<agent state dir>", "<cedian state dir>"]
       outside: prompt                     # or allow
   ```
   cedian writes `roots` as the canonical workspace, `allow` from `read_folders_for(workdir)` (the user's file only; a repository cannot widen its own scope), `protectedNames` and `protectedPaths` from `ProtectedRoots` (ADR-0012, ADR-0044, ADR-0078), and `outside: prompt`.
3. **Order of evaluation**, per path, strictest first (ADR-0012):
   1. protected: a component named in `protectedNames` under any root or allowed folder, or a path under a `protectedPaths` entry, is a tool `deny`. It is denied even inside an allowed folder, and the prompt never opens, so it cannot be clicked through.
   2. inside a root: `read`.
   3. inside an allowed folder: `read`.
   4. otherwise `outside` applies: `prompt` forces a dialog that names the path, `allow` passes.
   The user's `tools.approval.<tool>` is consulted after the tool's decision, so the Safe tier below can tighten but never loosen this: `allow` does not skip the outside prompt, `deny` and `prompt` apply to every read.
4. **What a path is.** The argument is unwrapped (`[path#TAG]`, `@`, `~`), resolved against the agent's cwd (so `..` is applied), then canonicalized: the longest existing prefix goes through `realpath`, the rest is appended. The roots and folders are canonicalized the same way. A symlink inside the project that points outside therefore resolves outside and asks; a path that does not exist yet is judged by where it would be. Comparison is by path components; on macOS it ignores case, since the default filesystem does. A URL or internal URI (`scheme://`) is not a filesystem path and keeps its own tier. A tool with no path argument searches the cwd, which is inside.
5. **Which tools.** `read` (text, images, PDFs, archives), `grep`, `glob`, `find`, `ast_grep`, and `lsp` for its read-only actions, through one helper. `bash` keeps its own approvals (ADR-0077 decision 4; ADR-0012's patterns). Write tools are out of scope: the write tier already asks outside the workspace.
6. **The Safe actions tier applies.** `permissions.safe` in `cedian.toml` maps onto `tools.approval.<tool>` for every tool in `READ_TOOLS`: `ask` writes `prompt` (every read asks, inside the project too), `deny` writes `deny` (the agent cannot read), `allow` writes nothing. Under `policy = "omp"` only the `deny` survives, as for every other tier (ADR-0035 decision 3).
7. **Under `policy = "omp"`** (ADR-0035) the overlay still carries the scope with `outside: allow`: protected paths stay denied (a cedian deny applies in every mode), and whether an outside read asks is the person's OMP config's call, as every other approval is there.
8. **Asked once per call, audited.** The prompt is OMP's ordinary approval dialog (`Allow tool: read`, `Reason: outside the project`, `Path: …`), so the app shows it in the panel like any approval, and the answer lands in `audit.jsonl` as the gate row cedian already writes (tool, label, decision, who answered). A protected deny is audited by the fork's tool result; cedian's gate also refuses a dialog whose title names a protected path, as it does for writes.
9. **A read folder is never the project or its ancestor.** The settings writer refuses `read_folders` entries equal to the project key or containing it, and the Permissions and Projects pages show the refusal on the row. A `cedian.toml` found inside the open workspace is reported on both pages (`SettingsError::InWorkspace`), not silently ignored.

## Consequences

- The agent reads the project and the allowed folders without a dialog; the first read anywhere else is a question in the panel. A person who finds the questions tiresome adds the folder on Settings > Permissions, where SET-4 put it.
- `.git`, `.cedian` and `.omp` under the project or an allowed folder are unreadable by the agent, as are `~/.omp` and cedian's own agent and state dirs. A workflow that needs the agent to read git metadata goes through `bash` and its own approvals.
- The fork gains one setting and one module; upstream merges touch the six tools' `approval` properties. Each tool test in the fork covers its own tier; the scope matrix is tested once on the helper.
- The read half of `lsp` asks when the file is outside; its write actions already did.
- Subagents (`task`) run headless under `yolo`; a read that resolves to `prompt` there is rejected, as every prompt is (approval-mode.md "Subagents"). A protected deny holds there too.
- The reviewer (ADR-0043) runs under Seatbelt and gets no `tools.readScope`: the kernel already bounds what it reads.
