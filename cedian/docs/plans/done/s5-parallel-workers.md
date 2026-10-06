# S5 Parallel Workers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parallel workers on isolated git worktrees, visible + steerable from the CLI; conflicts surface as STALE, merge-back requires explicit accept.

**Architecture:** New `cedian_worker` crate (worktree mechanism via `git` CLI subprocess + `WorkerRegistry` persisted at `<repo>/.cedian/workers.json`), same headless discipline as S1/S4: blocking API, no GPUI, S9 binds later. CLI `worker spawn|list|steer|merge-back|remove`. One-shot-per-invocation like `browser`: each command shells out to `git worktree`, never holds a daemon. Swarm/arena are presets (Phase 17) — explicitly NOT this plan.

**Tech Stack:** Rust (edition 2021), `serde`/`serde_json` (workspace), `git` CLI subprocess (`Command`, no new deps).

**Spec:** `CEDIAN_AGENTIC_IDE_PLAN.md` §43 R1 (cedian owns worktrees, OMP requests; conflict → STALE, never auto-merge; merge-back needs explicit accept), Phase 16 (mechanism + visualization land together — CLI list IS the headless visualization), §18 R3 (hunk precedence INTERRUPTED > UNATTRIBUTED > STALE — workers surface STALE through the existing tracker, no new states), §88 (no second orchestrator: registry is a MECHANISM record, OMP stays the only orchestrator), §86 (hermetic tests default; git-touching tests use a temp repo fixture, `#[ignore]`d live lane only if it needs network — local `git init` needs none, so NOT ignored).

## Global Constraints

- Rust edition 2021; `[lints] workspace = true` in the new crate (`unsafe_code = deny`, clippy `print_stdout` warn — stdout ONLY in `cedian_cli`).
- Blocking/sync API only; `git` via `std::process::Command` with explicit `--git-dir`/`-C` (never rely on caller cwd).
- Worker worktrees live at `<repo>/.worktrees/<worker-id>` (gitignored by convention — check `.gitignore` covers `.worktrees/`; S1 `workspace_files.rs` already skips `.worktrees` in scans).
- Registry at `<repo>/.cedian/workers.json`: `{workers: {id: WorkerHead{id, worktree: relpath, task_title, kind, status, note}}}`; `status ∈ {ready, running, done, stale, failed}`.
- Merge-back is `git diff worktree→base` preview + explicit `--accept` (no auto-merge, no rebase, no fast-forward tricks). Conflict detection: re-run the equivalent of `git merge-tree` dry check; on conflict report STALE files, refuse without `--force`? NO — never `--force`; refuse and tell the user to resolve in the worktree first.
- Hermetic tests: temp-dir git fixture (`git init -b main`, commit, then spawn) — no network, runs in `cargo test --workspace`.

## File Structure

- Create `crates/cedian_worker/Cargo.toml` — deps: `serde`/`serde_json` (workspace), `[lints] workspace = true`.
- Create `crates/cedian_worker/src/lib.rs` — re-exports + crate docs (plan refs §43/Phase 16/§18, S9 handoff, one-shot note).
- Create `crates/cedian_worker/src/registry.rs` — `WorkerHead`, `WorkerStatus`, `Registry::{open(repo), save, insert, get, set_status, remove}` + `store_roundtrip` test.
- Create `crates/cedian_worker/src/worktree.rs` — `spawn(repo, id, base) -> PathBuf` (`git worktree add`), `remove(repo, id)` (`git worktree remove --force`? NO — plain `remove`, fails on dirty (good: surfaces state); document), `merge_preview(repo, id, base) -> MergePlan{clean: Vec<String>, conflicted: Vec<String>}` via `git diff --name-only base...worktree-branch` + `git merge-tree` check, `merge_back(repo, id, base)` (only when no conflicted; uses `git merge --no-ff --no-commit`? NO — simplest correct: `git -C repo merge <branch> --no-edit` fails loud on conflict; pre-check guarantees clean so it fast-forwards or merges cleanly. Branch per worker: `cedian-worker/<id>` created at spawn from base).
- Modify `crates/cedian_cli/Cargo.toml` — add `cedian_worker` dep (alpha order).
- Create `crates/cedian_cli/src/worker_cmd.rs`? NO — follow existing convention: subcommands live in `main.rs` (`cmd_workflow`, `cmd_browser`), stores in `*_store.rs`. Registry IO stays in the crate; CLI holds only `cmd_worker` in `main.rs`.
- Modify `crates/cedian_cli/src/main.rs` — `worker` subcommand.
- Modify `README.md` — S5 row → ✅ once exit holds.

---

### Task 1: Registry (persist worker heads)

**Files:**
- Create: `crates/cedian_worker/Cargo.toml`
- Create: `crates/cedian_worker/src/lib.rs`
- Create: `crates/cedian_worker/src/registry.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `WorkerStatus::{Ready, Running, Done, Stale, Failed}` (serde snake_case); `WorkerHead{id, branch, worktree: String (repo-rel), task_title, kind, status, note: String}`; `Registry::{open(repo: &Path) -> (Registry, bool existed), save(&self, repo), insert(head) -> Result<(), WorkerError>, get(&self, id) -> Option<&WorkerHead>, set_status(&mut self, id, status, note), remove(&mut self, id) -> Option<WorkerHead>}`; `WorkerError::{Exists(String), NoSuch(String), Io(String)}` with Display.

- [ ] **Step 1: Write the failing test** (`registry.rs` `#[cfg(test)]`):

```rust
#[test]
fn registry_roundtrip() {
    let repo = std::env::temp_dir().join(format!("cedian-reg-test-{}", std::process::id()));
    std::fs::create_dir_all(repo.join(".cedian")).unwrap();
    let (mut reg, existed) = Registry::open(&repo);
    assert!(!existed);
    reg.insert(WorkerHead {
        id: "w1".into(),
        branch: "cedian-worker/w1".into(),
        worktree: ".worktrees/w1".into(),
        task_title: "fix login".into(),
        kind: "bug_fix".into(),
        status: WorkerStatus::Ready,
        note: String::new(),
    })
    .unwrap();
    assert!(reg.insert(WorkerHead {
        id: "w1".into(),
        branch: "b".into(),
        worktree: "w".into(),
        task_title: "t".into(),
        kind: "k".into(),
        status: WorkerStatus::Ready,
        note: String::new(),
    }).is_err());
    reg.save(&repo).unwrap();
    let (reg2, existed2) = Registry::open(&repo);
    assert!(existed2);
    assert_eq!(reg2.get("w1").unwrap().task_title, "fix login");
    std::fs::remove_dir_all(&repo).unwrap();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cedian_worker registry_roundtrip`
Expected: FAIL (crate does not exist yet → create skeleton first so it fails on missing symbol).

- [ ] **Step 3: Write minimal implementation**

`Cargo.toml` (name `cedian_worker`, edition 2021, workspace serde/serde_json, workspace lints + S4-style header comment). `lib.rs` (docs + `pub mod registry; pub mod worktree;` — declare `worktree` now so Task 2 only fills it; re-exports). `registry.rs`: JSON file `<repo>/.cedian/workers.json` shaped `{"workers": {id: head}}`; `open` reads or defaults `{"workers": {}}` + existed flag; `save` creates `.cedian/`; hand-rolled serde derives (Serialize/Deserialize on both types, `#[serde(rename_all = "snake_case")]` on the enum).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p cedian_worker`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cedian_worker
git commit -m "S5-task1: worker registry (persist heads)"
```

### Task 2: Worktree mechanism (spawn/remove/preview/merge-back)

**Files:**
- Create: `crates/cedian_worker/src/worktree.rs`
- Modify: `crates/cedian_worker/src/lib.rs` (re-export)

**Interfaces:**
- Consumes: `Registry`/`WorkerHead` (Task 1).
- Produces: `spawn(repo, id, base: &str) -> Result<WorkerHead, WorkerError>` — `git -C repo worktree add .worktrees/<id> -b cedian-worker/<id> <base>`; `remove(repo, head)` — `git -C repo worktree remove <worktree>` then `git -C repo branch -d <branch>` (force-delete ONLY when branch is fully merged? `branch -d` fails on unmerged = good, surfaces state; on `remove` failure return `WorkerError::Git` and do NOT delete the branch); `merge_preview(repo, head, base) -> Result<MergePlan, WorkerError>` — `MergePlan{clean: Vec<String>, conflicted: Vec<String>}` via `git diff --name-only <base>...<branch>` for the file list + `git merge-tree $(git merge-base <base> <branch>) <base> <branch>` parsed for `<<<<<<<` conflict markers? SIMPLER correct: `git -C repo merge-tree $(git -C repo merge-base base branch) base branch` and grep output for lines starting with `<<<<<<<`? merge-tree output format varies; MOST ROBUST without parsing: attempt `git -C repo merge --no-commit --no-ff <branch>` on a THROWAWAY clone? NO — simplest hermetic-correct: `git merge-tree` + check exit + scan for `<<<<<<< `. If scanner is uncertain, treat as conflicted (fail closed); `merge_back(repo, head, base)` — refuses when preview has conflicted entries (`WorkerError::Conflicted(files)`), else `git -C repo merge <branch> --no-edit` (fails loud on surprise conflict); `WorkerError::{Exists, NoSuch, Io, Git(String), Conflicted(Vec<String>)}`.

- [ ] **Step 1: Write the failing tests** (hermetic temp git fixture, NOT ignored):

```rust
fn fixture() -> (tempdir PathBuf, repo) {
    let dir = std::env::temp_dir().join(format!("cedian-wt-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-b", "main"]).unwrap();
    git(&dir, &["config", "user.email", "t@t"]).unwrap();
    git(&dir, &["config", "user.name", "t"]).unwrap();
    std::fs::write(dir.join("a.txt"), "base\n").unwrap();
    git(&dir, &["add", "."]).unwrap();
    git(&dir, &["commit", "-m", "base"]).unwrap();
    dir
}

#[test]
fn spawn_preview_merge_clean() {
    let repo = fixture();
    let head = spawn(&repo, "w1", "main").unwrap();
    assert_eq!(head.branch, "cedian-worker/w1");
    // Worker edits a disjoint file.
    std::fs::write(repo.join(".worktrees/w1/b.txt"), "worker\n").unwrap();
    git(&repo.join(".worktrees/w1"), &["add", "."]).unwrap();
    git(&repo.join(".worktrees/w1"), &["commit", "-m", "w"]).unwrap();
    let plan = merge_preview(&repo, &head, "main").unwrap();
    assert_eq!(plan.clean, vec!["b.txt".to_string()]);
    assert!(plan.conflicted.is_empty());
    merge_back(&repo, &head, "main").unwrap();
    assert!(repo.join("b.txt").exists());
    remove(&repo, &head).unwrap();
    std::fs::remove_dir_all(&repo).unwrap();
}

#[test]
fn conflicting_edit_refuses_merge() {
    let repo = fixture();
    let head = spawn(&repo, "w1", "main").unwrap();
    std::fs::write(repo.join(".worktrees/w1/a.txt"), "worker\n").unwrap();
    git(&repo.join(".worktrees/w1"), &["commit", "-am", "w"]).unwrap();
    std::fs::write(repo.join("a.txt"), "base-edit\n").unwrap();
    git(&repo, &["commit", "-am", "b"]).unwrap();
    let plan = merge_preview(&repo, &head, "main").unwrap();
    assert_eq!(plan.conflicted, vec!["a.txt".to_string()]);
    assert!(matches!(merge_back(&repo, &head, "main"), Err(WorkerError::Conflicted(_))));
    remove(&repo, &head).unwrap();
    std::fs::remove_dir_all(&repo).unwrap();
}
```

with helper `fn git(dir: &Path, args: &[&str]) -> Result<String, String>` (Command `git -C dir`, capture output, err on non-zero with stderr).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cedian_worker`
Expected: FAIL (`spawn` undefined).

- [ ] **Step 3: Write minimal implementation**

`worktree.rs` per interfaces above. `merge_preview` conflict check: run `git merge-tree <merge-base> <base> <branch>`; scan stdout for lines starting with `<<<<<<< ` — collect the filename from the marker tail (`<<<<<<< .our` style varies; SAFER: any `<<<<<<<` line marks the WHOLE preview conflicted? NO — per-file precision matters for UX. Middle ground: parse `merge-tree --write-tree`? OVERKILL. DECISION: use `git merge --no-commit --no-ff` dry-run on a scratch worktree of base? That mutates index. FINAL simplest-correct: create a temp clone (`git clone -s -b <base> repo tmp`), `git merge --no-edit <branch>` there, record conflicted files via `git diff --name-only --diff-filter=U`, delete tmp. Real merge semantics, zero parsing risk, hermetic (local clone `-s` is instant). DO THAT.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cedian_worker`
Expected: PASS (3 tests: registry + 2 worktree).

- [ ] **Step 5: Commit**

```bash
git add crates/cedian_worker
git commit -m "S5-task2: worktree spawn/remove/preview/merge-back"
```

### Task 3: CLI `worker` surface

**Files:**
- Modify: `crates/cedian_cli/Cargo.toml` (add `cedian_worker` dep, alpha order)
- Modify: `crates/cedian_cli/src/main.rs` (`worker` subcommand)

**Interfaces:**
- Consumes: `cedian_worker::{Registry, WorkerHead, WorkerStatus, spawn/remove/merge_preview/merge_back}` (Tasks 1–2), `workflow_store` pattern (NOT the module — worker state lives in the MAIN repo `.cedian/`, while each worktree has its own workflow.json; CLI takes `--repo` defaulting to `CEDIAN_WORKDIR`).
- Produces CLI: `cedian worker spawn <id> <kind> <title> [--base <branch>]` · `cedian worker list` · `cedian worker steer <id> <note>` (sets status Running + appends note — the actual agent turn in the worktree is a follow-up; document) · `cedian worker preview <id> [--base]` · `cedian worker merge-back <id> [--base]` (refuses on conflict with STALE file list) · `cedian worker remove <id>`.

- [ ] **Step 1: Write the failing test** — CLI has no harness; assert dispatch exists via build: no test, verification is the Step 4 smoke. (State this deviation in the commit message.)

- [ ] **Step 2: Verify it fails** — `cargo build -p cedian_cli` succeeds but `cedian worker` prints usage (no arm). Run: `cargo run -q -p cedian_cli -- worker` → usage line without `worker` arms.

- [ ] **Step 3: Write minimal implementation**

`main.rs`: `"worker" => cmd_worker(&workdir, &args[1..])` + usage line update. `cmd_worker`: `spawn` — Registry::open, `spawn(repo, id, base or "HEAD")`, registry insert (status Running, task_title, kind), save, print `worker {id} → {worktree} (branch {branch})`; `list` — open + print `id status worktree task` rows or `no workers`; `steer` — get, set_status(Running, note), save, print `steer {id}: {note}` (+ hint: run the turn with `CEDIAN_WORKDIR=<worktree> cedian prompt …`); `preview` — merge_preview, print `clean:` + `conflicted(STALE):` lists; `merge-back` — merge_back, on `Conflicted(fs)` print `refused (STALE):` + files and exit 1, else set_status(Done), save, print `merged {branch} → {base}`; `remove` — registry get + worktree::remove + registry remove + save, print `removed {id}`.

- [ ] **Step 4: Smoke to verify** (fresh git repo fixture):

```sh
export T=/tmp/wk-smoke && rm -rf $T && mkdir -p $T && cd $T && git init -b main -q && git config user.email t@t && git config user.name t && echo base > a.txt && git add . && git commit -qm base
export CEDIAN_WORKDIR=$T
cedian worker spawn w1 bug_fix "fix typo"        # expect: worker w1 → .worktrees/w1
cedian worker list                                # expect: w1 row
(cd $T/.worktrees/w1 && echo worker > b.txt && git add . && git commit -qm w)
cedian worker preview w1                          # expect: clean: b.txt
cedian worker merge-back w1                       # expect: merged
test -f $T/b.txt && echo MERGED_OK
cedian worker remove w1 && cedian worker list     # expect: no workers
```

- [ ] **Step 5: Commit**

```bash
git add crates/cedian_cli
git commit -m "S5-task3: CLI worker spawn/list/steer/preview/merge-back/remove"
```

### Task 4: Full verification, README, final commit

**Files:**
- Modify: `README.md` (S5 row → ✅)

**Interfaces:**
- Consumes: Tasks 1–3.

- [ ] **Step 1: Workspace green**

Run: `cargo test --workspace`
Expected: all PASS, zero failures.

- [ ] **Step 2: Lints + fmt clean**

Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: no warnings, no drift.

- [ ] **Step 3: S5 exit script end-to-end** — rerun the Task 3 Step 4 smoke on a fresh dir + a conflict case (both edit `a.txt` → `preview` shows STALE → `merge-back` refuses exit 1).
- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "S5: parallel workers on worktrees (spawn/steer/merge-back)"
```

## Self-Review

- Spec coverage: §43 R1 (cedian owns lifecycle via `git worktree`, OMP requests via CLI/host path; STALE on conflict; explicit merge-back) ✓; Phase 16 mechanism+visualization together (registry+list = headless visualization) ✓; Phase 17 presets explicitly deferred ✓; §88 no new engine (no scheduler/queue in the crate — registry is a record) ✓.
- No placeholders: exact commands, exact asserts, exact smoke transcript.
- Type consistency: `WorkerHead`/`WorkerStatus`/`Registry`/`WorkerError` defined Task 1, consumed Tasks 2–3; `MergePlan` defined Task 2, consumed Task 3.
