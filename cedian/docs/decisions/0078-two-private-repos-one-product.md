# ADR-0078: cedian is one private product in two repos, `cedian` and `cedian-agent`

- **Status:** Proposed (owner's answers 2026-10-15, AskUserQuestion: personal and closed ("ใช้เองแบบปิดส่วนตัวครับ"); two private repos ("ตกลงครับ"); clean up only the two old cedian forks; the shipped app runs only the bundled agent; no GitHub CI, the local check is the gate; v0.1 unsigned; names: cedian only where a user sees it; the agent keeps its own storage; projects' `AGENTS.md` and `.agents/` are read; repo names `cedian` and `cedian-agent` ("Cedian-agent"). Accepted when the owner merges this ADR.)
- **Date:** 2026-10-15
- **Rule text:** AGENTS.md ("Where things live"); README "Code, build and test"; ARCHITECTURE §7, §8
- **Builds on:** ADR-0042 (code moved into the Zed fork), ADR-0069 (weekly upstream merge), ADR-0075 (the narrow OMP fork), ADR-0077 (v0.1's gates)
- **Supersedes in part:** ADR-0042 (where the code and the docs live), standing "CI green before merge" rules (the local check replaces CI)

## Context

cedian is for the owner's own use and is not published. Today it spans three repositories, two of them public: the docs (`cedian`, private), the app (`zed`, a public fork of Zed) and the agent (`oh-my-pi`, a public fork of OMP). GitHub does not let a fork of a public repository become private. The docs were kept apart only because the code was public. The app now bundles the agent's build from a submodule (fork PR #19).

## Decision

1. **Two private repositories.**
   - **`cedian`**: the app (the Zed fork's history, default branch `main` = today's `cedian/s9`) and the docs, imported with their history into `cedian/` (the status table at `cedian/README.md`, the rest under `cedian/docs/`, cedian's rules merged into the root `AGENTS.md`). The agent is the submodule `vendor/cedian-agent`.
   - **`cedian-agent`**: the OMP fork (default branch `main` = today's `cedian/main`).
   - Upstream Zed and OMP stay git remotes and come in by merge (ADR-0069, ADR-0075). Neither repository is a GitHub fork, so nothing offers a PR to upstream.
2. **The move.** The current docs repo is renamed `cedian-docs-archive` and archived after its import, so the name `cedian` goes to the product. The old public forks `zed` and `oh-my-pi` are deleted by the owner after the move is verified; the agent never deletes a repository. Links in the docs that point at the old repositories are rewritten in the same change.
3. **The gate is local.** GitHub Actions on private repositories costs minutes, so the repositories carry no CI. `script/cedian-check` runs before every commit; each PR carries its log and an independent review on a different model; the owner merges. Branch protection needs a paid plan, so the "owner merges" rule is a working rule, not a setting.
4. **One product to the user.** Everything a user sees says cedian: the app, menus, settings, messages, and the bundled agent binary `cedian-agent`. Internal names (the Zed crates, `omp` in code, RPC names) stay, so upstream merges stay cheap. About and the licenses keep Zed's (GPL) and OMP's (MIT) notices, as their licenses require.
5. **The agent keeps cedian's own storage.** Its configuration lives in `~/.config/cedian/agent` and what it writes (sessions, logs, tokens, locks) in `~/.local/state/cedian`, never in the person's `~/.omp`. The person logs in once inside cedian; an import command brings skills over from `~/.omp`. A shipped app ignores `CEDIAN_OMP_BINARY`; only dev and test builds honor it.
6. **Projects' shared agent files are read.** cedian reads a project's `AGENTS.md` and `.agents/` (read only), so instructions written for other agents also reach cedian's.
7. **v0.1 is not signed or notarized.** It runs on this machine only.

## Consequences

- One product, two repositories: the app and its docs change in one PR; the agent keeps its own history for upstream merges.
- No CI means no second machine checks a change. The local check, its log in the PR, and the independent review carry that weight.
- The move needs a quiet tree: no other agent writes in either clone while it runs. Every open branch is merged or carried over first.
- Decisions 4 to 6 are their own units after the move, ahead of v0.1's declaration.
