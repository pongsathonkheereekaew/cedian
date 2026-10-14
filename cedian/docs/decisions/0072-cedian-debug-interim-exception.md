# ADR-0072: `cedian_debug` is an interim exception to "no duplicate debugger tool", until OMP's `debug` can attach to Zed's session

- **Status:** Accepted (owner, 2026-10-14, AskUserQuestion. First "ค่อยๆ เปลี่ยนแบบ LSP" and "lldb-dap ตัวจริง + 1 turn จริง"; then, told that `cedian_debug` conflicts with ARCHITECTURE §10, §22 and ADR-0004, chose "ยกเว้นชั่วคราว + ขอ OMP"; on approvals, "ถามเฉพาะตอนเริ่มและคุม session ของคุณ"; on Developer mode, "ผมเปิดเอง")
- **Date:** 2026-10-14
- **Rule text:** ARCHITECTURE chapter 2 ("duplicate debugger tool"), §10, §22; ROADMAP S1; the fork's `cedian/OMP_PARITY.md` row `debug`
- **Refines:** ADR-0004 (OMP tool names are never duplicated) and ADR-0048 (staging an OMP tool against Zed's), for the debugger only

## Context

S1's exit asks that an agent breakpoint round-trips on a headless debug adapter, and §22 asks that it be Zed's own session: the breakpoint in the gutter, the same stack and variables, and continue from either side. §22 expects OMP's `debug` tool, unchanged, to do that through `cedian://` URIs and host tools.

OMP 18.8.6's `debug` cannot. It always starts its own adapter (gdb, lldb-dap, debugpy and others), the RPC has no debug command, and there is no seam for a host-owned session. OMP is never forked (ADR-0027). So on the pin, the only way to the §22 effects is a host tool that drives Zed's debugger, which is the "duplicate debugger tool" chapter 2 forbids.

## Decision

1. **`cedian_debug` is allowed for now.** It is a host tool with an `op` field (sessions, start, set and remove breakpoint, continue, pause, step over, threads, stack, variables, stop) that acts on Zed's own debug session. Its name never equals an OMP tool name.
2. **OMP's `debug` stays on** beside it, staged as ADR-0048 staged `lsp`. Turning it off needs its own ADR.
3. **The exception ends when OMP can attach.** cedian asks OMP upstream for a way to point `debug` at an adapter or session the host already runs. When a pin has it, `cedian_debug` is removed in the same change that moves the app to OMP's `debug`, and this ADR is superseded.
4. **Which tool the model picks is measured, not assumed.** The live turn that proves S1 does not name `cedian_debug` in its prompt. If the model uses OMP's `debug`, that is recorded as a finding and S1 stays ◐.
5. **Approvals** (ADR-0012, decided at cedian's gate). `start` asks. Continue, pause and step ask only on a session the person started. Setting and removing breakpoints and reading threads, stack and variables are allowed. An unknown scenario or a path outside the worktree, private or under `.git`, is denied. Every call is audited. Reviewers and automations never get the tool.
6. **Evidence for S1.** The hermetic test on Zed's fake adapter runs in CI. S1 is ✅ only after a run on the real Command Line Tools `lldb-dap` and one live `cursor/default` turn from ADR-0070's 20. Both need macOS Developer mode, which the owner turns on.

## Consequences

- S1 can close on the pin without forking OMP or waiting for it.
- Until OMP can attach, the model sees two debugger tools. Decision 4 makes that visible rather than hidden.
- The exception has an end condition, an upstream request, and one place (this ADR) that the ledger row points to.
