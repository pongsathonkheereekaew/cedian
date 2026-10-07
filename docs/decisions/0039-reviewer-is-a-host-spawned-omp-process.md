# ADR-0039: A reviewer is a separate OMP process that cedian spawns at OMP's request

- **Status:** Accepted (owner, 2026-10-07, in chat: decision 1). Decision 2 takes the first option ADR-0028 already lists, so it needs no new owner call.
- **Date:** 2026-10-07
- **Rule text:** [ROADMAP.md](../ROADMAP.md) S3 exit and gate items 1–2; `docs/plans/done/s3-review-agents.md`
- **Refines:** ADR-0011 (reviewers in a read-only sandbox), ADR-0028 (known gap)

## Context

The S3 exit says "an OMP turn spawns at least one reviewer subagent under the reviewer profile". ADR-0011 asks for a reviewer that cannot write, in a fresh context, on a different model where OMP routing allows. Gate item 2 asks for a bypass-proof test: the kernel, not a policy check, denies a reviewer's write.

OMP's own subagents (`task`, `hub`, `vibe_spawn`) run inside the parent's process and config (ARCHITECTURE §42: "headless-yolo inside the parent task boundary"). A Seatbelt profile applies to a process, so an in-process subagent cannot get its own. It can also only use another model as far as OMP's `task` routing allows, which is unverified.

Two prototypes on 2026-10-07 (OMP 18.6.1):

- **Seatbelt holds with OMP.** Under a deny-default write profile that allows only a state directory, the temp directories and `~/.omp/run/daemons`, `omp -p` completes a turn. In the same profile, `touch` and `>>` inside the workspace fail with "Operation not permitted" from the kernel. The user's `~/.omp/agent` (config, skills, auth) stays unwritable.
- **The ADR-0028 gap can be read.** Run in a workspace, `omp config get tools.approval --json` returns OMP's merged record, including a project's `allow` for a tool the overlay does not name.

## Decision

1. **The reviewer is its own OMP process.** The implementing OMP turn asks for a review through a cedian host tool. cedian spawns a separate OMP process for the reviewer:
   - through the spawn profile, with the reviewer variant (`RunKind::Unattended`, never `policy = "omp"`);
   - wrapped in `sandbox-exec` with a generated reviewer profile: deny-default writes, the workspace explicitly denied, process execution limited to OMP and the reviewer allow-list;
   - on the model named in the user's `cedian.toml`, which must differ from the implementer's when one is set;
   - in a fresh session, with `cedian_review_finding` as its host tool.
   This counts as "an OMP turn spawns a reviewer" for the S3 exit.
2. **The ADR-0028 gap closes at spawn.** Before each spawn, cedian reads the effective `tools.approval` through `omp config get` (the P8 mechanism) and pins every `allow` the overlay does not name. The default profile pins it to `prompt`; the reviewer profile pins it to `deny`. Host tools keep their `allow` (ADR-0028 decision 2).

## Consequences

- Reviewers do not appear in OMP's own subagent list. The app (S9) shows them from cedian's records.
- Each review costs one more OMP spawn, about 1 s (S2 benchmark, median 966 ms).
- The reviewer profile is generated per run. Its allow-list comes only from the user's `cedian.toml` (ADR-0018), so a workspace cannot widen it.
- If OMP later gives subagents their own process and profile, this ADR is revisited.
