# ADR-0044: cedian's per-workspace state lives outside the workspace

- **Status:** Accepted (owner, 2026-10-07, in chat: "ย้ายออกจาก workspace")
- **Date:** 2026-10-07
- **Rule text:** ARCHITECTURE §77 (config ownership); `docs/plans/done/s3-review-agents.md` "Security review (2026-10-07)"
- **Refines:** ADR-0012 decision 3 (protected metadata paths: the `.cedian/` part), ADR-0021 (store and lock location), ADR-0032 (ledger location)

## Context

Every cedian store sits in `<workspace>/.cedian/`. That covers the review baseline and resolutions, the workflow and its gate evidence, verification profiles, the browser head, screenshots, reviewer findings, the audit log, the correction ledger and classes, the shell lock, the worker registry and the session list.

The implementer OMP runs with `--approval-mode write`, so it writes inside the workspace without a prompt. ADR-0012 says agent writes to `.cedian/` are `Deny`, but no code enforces that. The S3 security review (2026-10-07) found that the implementer can empty `findings.json` and then pass `cedian_complete` past an open blocker. It can also edit `workflow.json` evidence and rewrite or append audit and correction rows. A guard on `.cedian/**` would have to catch every way OMP can write: `write`, `edit`, `bash` redirection, and any program a `bash` allow pattern runs. Moving the files out of reach is simpler and closes the class.

## Decision

1. **One state directory per workspace, outside it.** The root is `$CEDIAN_STATE_DIR` if set, else `$XDG_STATE_HOME/cedian`, else `~/.local/state/cedian`. Each workspace gets `<root>/workspaces/<name>-<hash>/`:
   - `<name>` is the workspace directory's last component, cut to letters, digits, `-` and `_`;
   - `<hash>` is the first 16 hex digits of the SHA-256 of the canonical workspace path.
   cedian writes the canonical path to a `workspace` file inside the directory, so a person can see which workspace it belongs to.
2. **Every store moves.** Each file that was `.cedian/<x>` is now `<state dir>/<x>`, with the same name and format.
3. **A refused state directory.** cedian refuses to run if the state directory resolves inside the workspace, because then the implementer could write it again.
4. **An old `.cedian/` is not read.** It is not migrated either: no one else uses cedian yet. `.cedian/` stays in the scan's skip list, so files already in a workspace are not loaded as source.

## Consequences

- With the default profile, the implementer cannot write the stores. Out-of-workspace writes prompt, and headless refuses them. Under `policy = "omp"` (ADR-0035), the user's OMP config may let the implementer write anywhere, the state directory included. The P8 badge already says the user's OMP config decides.
- ADR-0012's other protected paths (`.git`, the OMP session dir) are still not enforced. That is a ROADMAP follow-up, not part of this ADR.
- Integration tests and the benchmark harness set `CEDIAN_STATE_DIR`, as they set `CEDIAN_CONFIG`. Unit tests use a per-process temp root, never the user's state. The worker registry and the worktree host tool take the state directory from their caller.
- The ROADMAP and plan text that says `.cedian/<x>` now means `<state dir>/<x>`. Accepted ADRs keep their wording; this ADR is the pointer.
