# S4 Browser Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** CLI drives a real page via cedian-owned headless Chromium over CDP, captures screenshot + DOM, and a workflow gate consumes the capture as evidence.

**Architecture:** New `cedian_browser` crate (process spawn + sync CDP client + session), same headless-subprocess discipline as S1 (`cedian_lsp`/`cedian_dap`): blocking API on its own thread, no GPUI, dies at S9 binding. CLI `browser open|shot|dom|close|status` + `shot --attach <gate>` into `.cedian/workflow.json`. Two OPTIONAL gates added to V1 playbooks (spec-faithful: plan §56 lists them as conditionals).

**Tech Stack:** Rust (edition 2021), `tungstenite` (sync WebSocket, unit-tested CDP framing), system Chrome headless (`--remote-debugging-port`), `serde_json`.

**Spec:** `CEDIAN_AGENTIC_IDE_PLAN.md` Phase 9 (shared browser; R1 user-input-preempts — GPUI concern, deferred to S9 with note), Phase 11 (browser as evidence provider), §29 R4 (evidence carries CDP frame id; stale-frame requires re-capture), §56 (bug_fix conditional "live behavior", feature conditional "visual verification"), §88 (no second browser tool), §86 (hermetic tests default; live-model/chrome lane `#[ignore]`d, never gating).

## Global Constraints

- Rust edition 2021; `[lints] workspace = true` in the new crate (`unsafe_code = deny`, clippy `print_stdout` warn — stdout printing ONLY in `cedian_cli`).
- Blocking/sync API only (matches `cedian_lsp`/`cedian_dap`); never spawn threads inside gate evaluation; gate engine stays pure (S2 untouched except playbook gate lists).
- Hermetic unit tests run in `cargo test --workspace`; anything touching real Chrome is `#[ignore]` (live lane: `cargo test -p cedian_browser -- --ignored`).
- No Node, no Playwright, no second Chromium: one cedian-owned Chrome profile dir per session under `.cedian/chrome-profile-<pid>`, removed on close.
- Every screenshot records `{png_path, frame_id, seq}`; staleness is a pure function; CLI always captures fresh (re-capture, never reuse).

## File Structure

- Create `crates/cedian_browser/Cargo.toml` — deps: `tungstenite = "0.26"`, `serde`/`serde_json` (workspace), `[lints] workspace = true`.
- Create `crates/cedian_browser/src/lib.rs` — re-exports + crate docs (plan refs §§9/11/29, R1/R4 notes, S9 handoff).
- Create `crates/cedian_browser/src/process.rs` — `BrowserProcess::spawn(chrome_exe, profile_dir) -> Result<BrowserProcess, BrowserError>`; owns `Child`, debug port, `GET /json/version` over std `TcpStream`; `Drop` kills child + removes profile dir.
- Create `crates/cedian_browser/src/cdp.rs` — `CdpClient::connect(ws_url)`, `call(method, params, timeout) -> Result<Value, CdpError>` (id-multiplexed, read-timeout loop, pumps `Page.frameNavigated`/`Page.loadEventFired` into an event queue), `drain_events() -> Vec<CdpEvent>`.
- Create `crates/cedian_browser/src/session.rs` — `BrowserSession::{open(url), screenshot(dir) -> Shot, dom() -> DomSnapshot, current_seq(), close()}`; `Shot { path, frame_id, seq }`; pure `is_stale(shot_seq, current_seq) -> bool`.
- Modify `crates/cedian_cli/Cargo.toml` — add `cedian_browser` dep.
- Modify `crates/cedian_cli/src/main.rs` — `browser` subcommand (`open|dom|shot|close|status`, `shot --attach <gate> [--note]`).
- Modify `crates/cedian_workflow/src/playbook.rs` — bug_fix += optional gate `live` (`Behavior`, kinds `[Browser, Screenshot]`); feature += optional gate `visual` (`Visual`, kinds `[Screenshot]`); unit tests for both.
- Modify `README.md` — S4 row → ✅ once exit holds.

---

### Task 1: Chrome process spawn + version handshake

**Files:**
- Create: `crates/cedian_browser/Cargo.toml`
- Create: `crates/cedian_browser/src/lib.rs`
- Create: `crates/cedian_browser/src/process.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `BrowserProcess::spawn(exe: &str, profile_dir: &Path) -> Result<BrowserProcess, BrowserError>`; `BrowserProcess { port: u16, browser_ws_url: String }`; `Drop` kills child.

- [ ] **Step 1: Write the failing test** (`process.rs` `#[cfg(test)]`, `#[ignore]` — needs real Chrome):

```rust
#[test]
#[ignore]
fn spawn_answers_version() {
    let dir = std::env::temp_dir().join("cedian-browser-test");
    let proc = BrowserProcess::spawn(CHROME_EXE, &dir).unwrap();
    assert!(proc.browser_ws_url.starts_with("ws://127.0.0.1:"));
}
```

with `#[cfg(target_os = "macos")] const CHROME_EXE: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";`

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cedian_browser -- --ignored spawn_answers_version`
Expected: FAIL (crate does not exist yet → create skeleton first so it fails on missing symbol, not missing crate).

- [ ] **Step 3: Write minimal implementation**

`Cargo.toml`:

```toml
[package]
name = "cedian_browser"
version = "0.1.0"
edition = "2021"

# S4: cedian-owned headless Chromium over CDP (plan Phase 9 + 11).
# Blocking API like cedian_lsp/cedian_dap; GPUI binding lands at S9.

[dependencies]
tungstenite = "0.26"
serde = { workspace = true }
serde_json = { workspace = true }

[lints]
workspace = true
```

`process.rs`: pick free port via `TcpListener::bind("127.0.0.1:0")` then drop; spawn `exe --headless=new --remote-debugging-port={port} --no-first-run --no-default-browser-check --user-data-dir={profile_dir} about:blank`; poll `GET http://127.0.0.1:{port}/json/version` over std `TcpStream` (write `GET /json/version HTTP/1.0\r\n\r\n`, read to end, split header/body, `serde_json::from_str` body, take `webSocketDebuggerUrl`) for up to 20s; `BrowserError::{Spawn(String), NoPort, Handshake(String)}`; `Drop`: `child.kill()` + `remove_dir_all(profile_dir)` (best-effort).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p cedian_browser -- --ignored spawn_answers_version`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cedian_browser
git commit -m "S4-task1: cedian_browser process spawn + version handshake"
```

### Task 2: Sync CDP client (id-multiplexed call + event pump)

**Files:**
- Create: `crates/cedian_browser/src/cdp.rs`
- Modify: `crates/cedian_browser/src/lib.rs` (re-export)

**Interfaces:**
- Consumes: `BrowserProcess.browser_ws_url` (Task 1).
- Produces: `CdpClient::connect(url: &str) -> Result<CdpClient, CdpError>`; `client.call(method: &str, params: Value, timeout: Duration) -> Result<Value, CdpError>`; `client.drain_events() -> Vec<CdpEvent>`; `CdpEvent::{FrameNavigated { frame_id }, LoadEventFired}`; `CdpError::{Connect(String), Io(String), Timeout(String), Rpc { code: i64, message: String }}`.

- [ ] **Step 1: Write the failing test** (hermetic — fake CDP server on std `TcpListener` + `tungstenite::accept`, NOT ignored):

```rust
#[test]
fn call_matches_response_id_and_pumps_events() {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    std::thread::spawn(move || {
        let mut ws = tungstenite::accept(server.accept().unwrap().0).unwrap();
        // Push an event first, then answer the call with interleaved traffic.
        ws.send(tungstenite::Message::Text(
            r#"{"method":"Page.loadEventFired","params":{}}"#.into(),
        ))
        .unwrap();
        let msg = ws.read().unwrap().into_text().unwrap();
        let v: serde_json::Value = serde_json::from_str(&msg).unwrap();
        let id = v["id"].as_u64().unwrap();
        ws.send(tungstenite::Message::Text(
            format!(r#"{{"id":{id},"result":{{"frameId":"F1"}}}}"#).into(),
        ))
        .unwrap();
    });
    let mut client = CdpClient::connect(&format!("ws://{addr}")).unwrap();
    let result = client
        .call("Page.navigate", serde_json::json!({"url": "about:blank"}), TIMEOUT)
        .unwrap();
    assert_eq!(result["frameId"], "F1");
    assert!(matches!(
        client.drain_events()[..],
        [CdpEvent::LoadEventFired]
    ));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cedian_browser call_matches_response_id`
Expected: FAIL (`CdpClient` undefined).

- [ ] **Step 3: Write minimal implementation**

`cdp.rs`: `connect` via `tungstenite::connect(url)`; internal `next_id: u64`; `call`: serialize `{"id","method","params"}`, `send`, then loop `read()` (set 100ms socket timeout via `get_mut()` → `TcpStream::set_read_timeout`, retry on `WouldBlock` until deadline): parse each message — if `id` matches, return `result` (or `Err(Rpc)` on `error`); if it has `method`, push `FrameNavigated`/`LoadEventFired` (only those two; ignore unknown methods per plan tolerance rule) and keep waiting; deadline exceeded → `Timeout`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p cedian_browser`
Expected: PASS (hermetic, no Chrome needed).

- [ ] **Step 5: Commit**

```bash
git add crates/cedian_browser/src/cdp.rs crates/cedian_browser/src/lib.rs
git commit -m "S4-task2: sync CDP client with event pump"
```

### Task 3: Session — open, screenshot, DOM, frame seq, stale check

**Files:**
- Create: `crates/cedian_browser/src/session.rs`
- Modify: `crates/cedian_browser/src/lib.rs` (re-export)

**Interfaces:**
- Consumes: `CdpClient` (Task 2).
- Produces: `BrowserSession::open(client, url, timeout) -> Result<BrowserSession, BrowserError>`; `session.screenshot(dir: &Path) -> Result<Shot, BrowserError>`; `session.dom(max_bytes: usize) -> Result<DomSnapshot, BrowserError>`; `session.current_seq() -> u64`; pure `is_stale(shot_seq: u64, current_seq: u64) -> bool`; `Shot { path: PathBuf, frame_id: String, seq: u64 }`; `DomSnapshot { url: String, title: String, html: String }`.

- [ ] **Step 1: Write the failing tests**

Hermetic (not ignored):

```rust
#[test]
fn stale_when_session_advanced() {
    assert!(!is_stale(3, 3));
    assert!(is_stale(2, 3));
}
```

Live (ignored, real Chrome + `data:` URL so no network):

```rust
#[test]
#[ignore]
fn open_shot_dom_roundtrip() {
    let dir = std::env::temp_dir().join("cedian-browser-sess");
    let proc = BrowserProcess::spawn(CHROME_EXE, &dir.join("profile")).unwrap();
    let client = CdpClient::connect(&proc.browser_ws_url).unwrap();
    let session = BrowserSession::open(
        client,
        "data:text/html,<title>hi</title><h1>hello</h1>",
        TIMEOUT,
    )
    .unwrap();
    let shot = session.screenshot(&dir).unwrap();
    assert!(shot.path.exists());
    assert!(!shot.frame_id.is_empty());
    let dom = session.dom(4096).unwrap();
    assert!(dom.title == "hi");
    assert!(dom.html.contains("hello"));
    assert!(!is_stale(shot.seq, session.current_seq()));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cedian_browser`
Expected: FAIL (`BrowserSession` undefined).

- [ ] **Step 3: Write minimal implementation**

`session.rs`: `open`: `Page.enable`, `Page.navigate`, wait for `LoadEventFired` or `frameId` in result (deadline = timeout); each navigation sets `frame_id` (from `Page.frameNavigated` event or navigate result) and bumps `seq`. `screenshot`: `Page.captureScreenshot {"format":"png"}` → base64 decode (note: `base64` crate is a `cedian_omp` dep, add `base64 = "0.22"` to the new crate) → write `{dir}/shot-{seq}.png` → `Shot { path, frame_id: self.frame_id.clone(), seq: self.seq }`. `dom`: `Runtime.evaluate {"expression":"({url:location.href,title:document.title,html:document.documentElement.outerHTML})","returnByValue":true}` → truncate html to `max_bytes` with `…[truncated N bytes]` suffix. `is_stale`: `shot_seq < current_seq`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cedian_browser && cargo test -p cedian_browser -- --ignored`
Expected: PASS both lanes.

- [ ] **Step 5: Commit**

```bash
git add crates/cedian_browser/src/session.rs crates/cedian_browser/src/lib.rs crates/cedian_browser/Cargo.toml
git commit -m "S4-task3: browser session open/shot/dom + frame seq"
```

### Task 4: Gate consumption — optional `live` / `visual` gates

**Files:**
- Modify: `crates/cedian_workflow/src/playbook.rs`

**Interfaces:**
- Consumes: `EvidenceKind::{Browser, Screenshot}` (already exist in S2).
- Produces: `bug_fix` playbook gains optional gate `live` (`GateKind::Behavior`, kinds `[Browser, Screenshot]`, `min_items: 1`, `require_ok: true`); `feature` gains optional gate `visual` (`GateKind::Visual`, kinds `[Screenshot]`, `min_items: 1`, `require_ok: true`).

- [ ] **Step 1: Write the failing tests** (append to `playbook.rs` tests):

```rust
#[test]
fn bugfix_has_optional_live_gate_for_browser_evidence() {
    let pb = Playbook::bug_fix();
    let g = pb.gates.iter().find(|g| g.id == "live").unwrap();
    assert!(!g.required);
    let e = Evidence::attributed(
        "s1",
        EvidenceKind::Screenshot,
        &["live"],
        "shot frame F1 seq 1",
        true,
        "t",
        "c",
    );
    let r = g.evaluate(std::slice::from_ref(&e));
    assert_eq!(r.status, GateStatus::Passed);
    assert!(!r.unverified_origin);
}
```

(same shape for `feature` → `visual`). Needs imports: `Evidence`, `EvidenceKind`, `GateStatus` in the test module.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cedian_workflow live_gate`
Expected: FAIL (`find … .unwrap()` on `None`).

- [ ] **Step 3: Write minimal implementation**

In `bug_fix()` gates vec append: `Self::gate("live", GateKind::Behavior, false, vec![EvidenceKind::Browser, EvidenceKind::Screenshot], 1, true)`. In `feature()` gates vec append: `Self::gate("visual", GateKind::Visual, false, vec![EvidenceKind::Screenshot], 1, true)`. No changes to gate evaluation (S2 semantics already handle optional + provenance).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cedian_workflow`
Expected: PASS (17 tests: 15 existing + 2 new).

- [ ] **Step 5: Commit**

```bash
git add crates/cedian_workflow/src/playbook.rs
git commit -m "S4-task4: optional live/visual gates consume browser evidence"
```

### Task 5: CLI `browser` surface + `shot --attach`

**Files:**
- Modify: `crates/cedian_cli/Cargo.toml` (add `cedian_browser` dep)
- Modify: `crates/cedian_cli/src/main.rs` (`browser` subcommand + session persistence)
- Create: `crates/cedian_cli/src/browser_store.rs` (persist `{port, ws_url, seq, url}` to `.cedian/browser.json` across invocations; Chrome child is reaped per-invocation — `open` spawns, `close` kills; document the headless limitation like `session.rs` does)

**Interfaces:**
- Consumes: `BrowserSession`/`Shot` (Task 3), `workflow_store::{load, save}` (exists), `Evidence::attributed` (exists).
- Produces CLI: `cedian browser open <url>` · `cedian browser dom` · `cedian browser shot [--attach <gate>] [--note <text>]` · `cedian browser close` · `cedian browser status`.

- [ ] **Step 1: Write the failing test** — CLI has no test harness; test `browser_store` round-trip instead (`browser_store.rs` `#[cfg(test)]`):

```rust
#[test]
fn store_roundtrip() {
    let dir = std::env::temp_dir().join("cedian-browser-store-test");
    let _ = std::fs::create_dir_all(&dir);
    let state = BrowserHead {
        port: 9222,
        ws_url: "ws://127.0.0.1:9222/devtools/page/1".into(),
        url: "about:blank".into(),
        seq: 1,
    };
    save(&dir, &state).unwrap();
    assert_eq!(load(&dir).unwrap().url, "about:blank");
    clear(&dir);
    assert!(load(&dir).is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cedian_cli browser_store`
Expected: FAIL (module undefined).

- [ ] **Step 3: Write minimal implementation**

`browser_store.rs`: JSON `{port, ws_url, url, seq}` at `.cedian/browser.json` (+ `CHROME_EXE` const for macos, error when missing). `main.rs` `cmd_browser`: `open` kills any stale session file entry (best-effort `close`), spawns process + connects + `BrowserSession::open`, saves head; `dom` reconnects via saved `ws_url`, prints `title url` + truncated html; `shot` reconnects, captures to `.cedian/shots/`, prints `path frame seq`, and with `--attach GATE` loads workflow, attaches `Evidence::attributed(id, Screenshot, &[gate], "shot {path} frame {frame_id} seq {seq} {note}", true, "cli", "cli-browser-{n}")`, saves, prints gate result; `close` kills via saved port? (child belongs to the `open` invocation — already reaped; `close` clears the file + kills any Chrome holding the profile via port check; document that cross-process child ownership is the headless stopgap, S9 keeps one live session); `status` prints saved head or "no browser session".

- [ ] **Step 4: Run tests + live smoke to verify**

Run: `cargo test -p cedian_cli` then live:
`CEDIAN_WORKDIR=/tmp/bw-smoke cedian browser open "data:text/html,<title>t</title>hi"` → `browser shot --attach live` (after `workflow run feature …`) → `workflow status` shows gate `live`/`visual` Passed.
Expected: unit PASS; smoke shows `✓ gate live: Passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/cedian_cli
git commit -m "S4-task5: CLI browser open/shot/dom/close + shot --attach"
```

### Task 6: Full verification, README, final commit

**Files:**
- Modify: `README.md` (S4 row → ✅)

**Interfaces:**
- Consumes: all Tasks 1–5.

- [ ] **Step 1: Workspace green**

Run: `cargo test --workspace`
Expected: all PASS, zero failures.

- [ ] **Step 2: Lints + fmt clean**

Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: no warnings, no drift (fix by editing source, never `#[allow]` for new code except the `unsafe_code` pre-existing pattern — new crate must have zero unsafe).

- [ ] **Step 3: Live lane green**

Run: `cargo test -p cedian_browser -- --ignored --nocapture`
Expected: Chrome spawn + open/shot/dom round-trip PASS on `data:` URL.

- [ ] **Step 4: S4 exit script end-to-end** (fresh dir):

```sh
export CEDIAN_WORKDIR=/tmp/s4-exit
cedian workflow run bug_fix "render login" --risk high
cedian browser open "data:text/html,<title>login</title><form>hi</form>"
cedian browser shot --attach live
cedian workflow status   # expect: ✓ gate live: Passed
cedian browser close
```

Expected: matches.

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "S4: browser evidence (CDP screenshot/DOM as gate evidence)"
```

## Self-Review

- Spec coverage: Phase 9 lifecycle/CDP/screencast — screencast + input forwarding + console/network are S9/GPUI concerns, noted in `lib.rs` docs as deferred (CLI has no screen/user to share); R1 preempt has no headless surface. Phase 11 acceptance ("required browser gate blocks completion for browser-facing tasks") holds via optional `live`/`visual` gates + `can_complete` semantics for required promotion later — required promotion itself is S9 policy, out of scope. R4 frame binding holds via `Shot.frame_id/seq` + pure `is_stale` + always-fresh capture.
- No placeholders: every step names exact files, commands, expected outputs.
- Type consistency: `Shot`, `DomSnapshot`, `CdpEvent`, `BrowserError`/`CdpError` defined once in Task 2–3, reused in Task 5.
