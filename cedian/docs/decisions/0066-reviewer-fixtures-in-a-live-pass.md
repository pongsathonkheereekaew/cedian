# ADR-0066: A live pass records the reviewer on its own pinned model and binary

- **Status:** Accepted (owner, 2026-10-14, in chat: "กติกาแยกสำหรับ reviewer")
- **Date:** 2026-10-14
- **Rule text:** the fork's `crates/cedian_fake_omp/src/recording.rs` (`write_fixture_on`, `RECORD_OMP_REAL_ENV`), `crates/cedian_cli/tests/replay_cli.rs` (the s3 and shell reviewer lanes), `script/cedian-bench/run.sh` (`record`)
- **Amends:** ADR-0061 (the pass pins its model) for reviewer fixtures only
- **Builds on:** ADR-0043 (the reviewer's sandbox), ADR-0039 (the independent reviewer), ADR-0065 (the second pass)

## Context

Pass `u12b` spent four turns on `s3review` and `s3same` and kept neither. The implementer's turn recorded, but the reviewer's OMP never ran. Two rules collide:

1. **The sandbox.** The reviewer runs under a Seatbelt profile that may exec only the binary it was given and the shells (ADR-0043). In a record lane that binary is the test's fake-omp proxy, and the proxy must exec the pass's OMP wrapper. The wrapper is a bash script that runs `tee` into a tap directory outside the reviewer's `run/` and then execs the pinned OMP. All three are denied, so the reviewer dies at once ("server closed").
2. **The model.** ADR-0061 makes the pass refuse any fixture whose OMP ran a model other than the pass's. An independent reviewer runs on a different model by design (ADR-0039), so its fixture would be refused even if it ran.

## Decision

1. **The reviewer's fixture is checked against the reviewer's pinned model**, the one the lane's roles config gives it (`opencode-go/glm-5.3` today, the model the lanes already assert), not against the pass's model. The implementer's fixture keeps ADR-0061's check.
2. **In record mode only, the reviewer runs the pinned OMP directly.** The harness exports the real pinned binary (`CEDIAN_RECORD_OMP_REAL`). The reviewer's record marker names that binary, and the lane's `cedian.toml` adds it to `reviewer_allow_list`, so the sandbox may exec it. The wrapper and its tap are not used for the reviewer. Replay and the app are unchanged, and the reviewer's profile still allows nothing else.
3. **The reviewer's turns still count.** The harness books a lane's turns from the `model turns` each written fixture prints, so the reviewer's turns are counted though no wrapper taps them.

## Consequences

- `s3_review*` and `s3_same_model*` can be recorded on 18.8.6, the ROADMAP row that kept them hand-made can go, and `u12b` spends four turns on them again.
- A record lane widens the reviewer's exec allow-list by exactly one binary, and only in a test root. That is the only place the sandbox differs from the app's.
