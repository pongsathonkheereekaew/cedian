# ADR-0075: cedian forks OMP narrowly, as it forks Zed, and ships the fork's build

- **Status:** Accepted (owner, 2026-10-15, in chat: "Forked omp เลยครับ ทำเองได้ผลดีกว่า"; then AskUserQuestion: "Fork แคบแบบ Zed (แนะนำ)", the upstream PR "ปิดทิ้ง", and "build จาก fork แล้วแนบไปกับแอป (แนะนำ)")
- **Date:** 2026-10-15
- **Rule text:** AGENTS.md (stack lock, fork rules); [ARCHITECTURE.md](../ARCHITECTURE.md) §7 stack lock, §8; [ROADMAP.md](../ROADMAP.md) "OMP changes", the build list
- **Supersedes:** ADR-0027 decision 1 ("No OMP fork, ever"). ADR-0027 decisions 2 to 6 stand: they never needed an OMP change.
- **Refines:** ADR-0073 decision 3 (the exception now ends when the fork carries host debug sessions, not when upstream does)

## Context

ADR-0027 kept cedian on upstream OMP and sent every OMP change upstream as a PR. The first such need was the `debug` tool attaching to a debugger session the host already runs (issue can1357/oh-my-pi#15703). The PR for it, can1357/oh-my-pi#15727, was opened on 2026-10-14. A day later it had only a review bot's comments, and OMP's contribution rules make the human contributor write, answer and push every change, which puts each upstream round trip on the owner. Until it ships, cedian carries `cedian_debug`, a duplicate of an OMP tool that ADR-0073 allows only as an interim exception.

The owner decided that changing OMP directly gives better results than waiting on upstream.

## Decision

1. **cedian keeps a narrow OMP fork.** It lives at `pongsathonkheereekaew/oh-my-pi`. The fork changes OMP only where cedian needs a capability OMP lacks, the same bar ADR-0034 sets for Zed. Anything that can be done from the host (host tools, `cedian://`, skills, the spawn profile, importing OMP's effects) is still done there.
2. **The fork starts from the pinned version.** Its work branch is cut from the tag of the current pin (18.8.6). A pin bump stays its own change with an ADR, and it becomes a merge of the newer upstream tag into the fork.
3. **Upstream comes in by merge, never rebase,** as ADR-0069 does for Zed: at least at every pin bump, through a fork PR that records the footprint (the upstream files the fork changes).
4. **The first patch is host debug sessions:** `set_host_debug` and the `host_dap_message`/`host_dap_close` frames, from the closed upstream PR, with the review bot's two fixes. When the app runs it, `cedian_debug` is removed, the app uses OMP's own `debug`, and ADR-0073 is superseded.
5. **The app ships the fork's build.** cedian builds OMP from the fork in CI (bun) and bundles that binary with the app. The spawn profile runs the bundled binary. The person's own OMP (`~/.local/bin/omp`) is not replaced or changed.
6. **The upstream PR is closed** (owner's choice). A later patch may still be offered upstream when that is cheap; it is never waited on.
7. **TypeScript is allowed in the OMP fork only.** The cedian process stays Rust and GPUI. No TypeScript is added anywhere else.

## Consequences

- An OMP capability cedian needs can land in days instead of waiting for upstream review.
- One person now maintains two forks. Each OMP patch is a merge cost at every pin bump, so the footprint is kept small and reported, as for Zed.
- The OMP that cedian runs differs from what upstream ships. `vendor/omp-revision.json` names the fork commit as well as the upstream version, and the parity ledger (ADR-0034) lists the fork's additions.
- The build gains a bun step and a bundled `omp` to sign with the app.
- The closed upstream PR stays readable as the design record for the first patch.
