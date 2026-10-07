# ADR-0043: The reviewer writes only its own run directory, cannot read credentials, and gets a fixed tool set

- **Status:** Accepted (owner, 2026-10-07, in chat: "กันอ่านโฟลเดอร์ลับ + ADR", and "Act On + Consider" from the S3 security review)
- **Date:** 2026-10-07
- **Rule text:** `docs/plans/done/s3-review-agents.md` "Security review (2026-10-07)"; ROADMAP S3 gate item 2
- **Refines:** ADR-0041 (reviewer is a separate sandboxed OMP process), ADR-0011

## Context

An independent review of S3 by three models (opus, fable, sonnet), with each claim checked against the fork's code, found that ADR-0041's profile left these holes:

- **Shared writable roots.** The profile allowed writes to all of `/private/tmp`, `/private/var/folders` and `~/.omp/run/daemons`. The default session dir (`$TMPDIR/cedian-cli-session`) is under `/private/var/folders`, so a steered reviewer could write `roles/.omp/config.yml`. That let it pick its own review model, which defeats ADR-0039 decision 3. It could also write its own overlay and the implementer's session files, and it could plant presence files in the implementer's project daemon directory.
- **Credentials readable, network open.** `(allow file-read*)` plus open network means a steered reviewer can read `~/.ssh` or `~/.aws` and send them out, for example with bash builtins and `/dev/tcp`.
- **Tools pinned, not chosen.** The overlay pinned only the `allow`s that OMP's config held when cedian read it. Built-ins with no approval entry (`lsp`, `notebook`, `web_search`), project MCP servers and project extensions still loaded in the reviewer.

A probe on 2026-10-07 (OMP 18.6.1, the pin) shows OMP puts its `run/`, `logs/` and agent state under `$XDG_STATE_HOME/omp` and its cache under `$XDG_CACHE_HOME/omp` when those directories exist. Auth and config are still read from `~/.omp/agent`. With those set to a private directory, `omp -p` completes a turn under a profile that has no temp-root or daemon-dir write allow and denies reads of `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.config/gh` and `~/Library/Keychains`. With the flags of decision 5 added, the reviewer lists only `read`, `grep`, `glob` and `bash` as its tools.

Seatbelt cannot filter network traffic by host name.

## Decision

1. **Writes: one fresh directory per review.** cedian creates `<reviewer dir>/run/` fresh for each review and the profile allows writes there only, plus `/dev/null`, `/dev/tty` and `/dev/fd`. OMP's `--session-dir`, `TMPDIR`, `XDG_STATE_HOME` and `XDG_CACHE_HOME` all point inside it. The overlay and the Seatbelt profile sit in `<reviewer dir>/`, outside `run/`, so the reviewer cannot rewrite them. The workspace deny stays last.
2. **Paths are canonical and checked.** `ReviewerSandbox::profile` refuses a path that is not canonical. It also refuses the OMP binary, any allow-listed program, or the session dir under a writable root. `reviewer_dir` compares canonical paths.
3. **Reads: everything except credentials.** The kernel denies reads under `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.config/gh`, `~/.config/gcloud`, `~/.azure`, `~/.kube`, `~/.docker`, `~/.password-store`, `~/Library/Keychains`, and of `~/.netrc`, `~/.git-credentials`, `~/.npmrc` and `~/.pypirc`. OMP's own auth store in `~/.omp/agent` stays readable: the reviewer needs it to reach its model.
4. **Network stays open.** The residual risk is accepted and stated: a steered reviewer can still send out what it can read, which includes the workspace, other repositories and OMP's auth token. Limiting egress to provider hosts through a cedian proxy is a ROADMAP follow-up.
5. **A fixed tool set.** The reviewer runs with `--tools read,grep,glob,bash`, `--no-extensions`, `--no-skills` and `--no-lsp`, and its overlay sets `mcp.enableProjectConfig: false`. A workspace cannot add tools to it. The `tools.approval` pin from ADR-0041 decision 2 stays as a second layer, and OMP's always-on `manage_skill` and `learn` are denied there.
6. **Bash allow-list patterns are exact.** An allow-list entry `c` becomes the patterns `c` and `c *`, never `c*`.
7. **Reviewer text is bounded and quoted.** A finding's message is capped at 2000 characters, with newlines and control characters folded to spaces. `count` is capped at the file's line count, a `line` past the end is refused, and a review records at most 50 findings. The implementer's reply marks reviewer text as quoted reviewer output, not instructions.

## Consequences

- The four existing kernel tests stay. New kernel tests cover: the session dir, roles dir and overlay are unwritable; `/private/tmp` is unwritable; a credential file cannot be read. A live test (ignored, needs real `omp`) runs a turn under the new profile.
- The reviewer leaves nothing behind outside `<reviewer dir>/run/`, which the next review deletes.
- The `mach-lookup` and `iokit-open` allows stay broad. Exploiting them needs code execution inside an allow-listed program, and narrowing them needs a logged probe of what OMP uses (ROADMAP follow-up).
- If OMP changes how it finds its state directory, the live test fails rather than the reviewer silently writing elsewhere: those writes are denied.
