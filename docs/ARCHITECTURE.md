# cedian architecture

Decided 2026-10-21 in a planning discussion with the owner, then corrected the same day by a review against cedia's code ("รีวิวแผน cedian ก่อน Phase A"). The full plan, with the roadmap and risks, is the "แผนสร้าง cedian v1" doc.

## What cedian is

The native macOS desktop for [cedia](https://github.com/pongsathonkheereekaew/cedia). cedia is the agent: loop, providers, tools, permission, sandbox, threads and trust rules all live there. cedian runs cedia, draws its threads as panes, and adds what only a desktop can give: a shared browser and terminal, editor buffers, and images and video shown inline. cedian never runs an agent loop of its own.

## Shape

```text
 window (Rust + GPUI)       later: phone, other Mac
      │                          │
      └──── Unix socket ─────────┘  (QUIC later)
                 │
          cedian daemon (Rust, its own process)
          runs cedia per prompt; terminals, worktrees,
          shared Chromium, keep-awake, settings watch
                 │ argv in, stream-json out
      ┌──────────┼──────────┐
   cedia -p   cedia -p   cedia threads send   (bundled, pinned build)
      └──────────┼──────────┘
          ~/.cedia: settings.json
                    projects/<slug>/threads/*.jsonl
                    sessions/<slug>/*.jsonl
                    snapshots/<worktree slug>/   (shadow git)
```

- The daemon is always its own process, registered as a LaunchAgent (`cedian-daemon install` until there is an app bundle, then `SMAppService`). The window starts it when it is not running and talks to it only over the socket, in development too. Quitting the window never stops a thread.
- One prompt is one cedia run: `cedia -p <text> --thread <id> --output-format stream-json`, with the thread's worktree as `--cwd`. A thread with no running prompt has no process. Text typed while a run works goes in with `cedia threads send <id> <text>` and reaches the model at the next turn boundary. SIGTERM aborts. A long-lived `cedia serve` is added only when host tools need it.
- `<slug>` is the project's: every worktree of a repository files its threads and sessions under the main worktree's root (cedia PR #19). Snapshots stay per worktree.
- Pane kinds in v1: agent, terminal, file (text, image, video, PDF and Office through Quick Look, with a file tree), editor, review, browser. No agent sidebar: the agent is a pane of blocks.
- One `open(target, how)` routes any file or URL to the right pane kind.

## Thread states

cedia writes five: `working`, `done`, `failed`, `stopped`, and `asking` when a run ended on a question for the owner (cedia PR #35, `--ask on`, which the daemon passes when cedia lists the `ask` capability). The daemon shows `working` only while a run of the thread is alive, and adds `interrupted` for a thread whose last state is `working` but whose run is gone (crash, reboot), which can go on with `--thread`. A thread in `asking` holds its question (the `question` thread event, cleared by the next state); the window shows it over the composer with a button per option, the recommended one first, and any answer goes in as the next prompt with `--thread`, which cedia frames with the question. The tool's description keeps the bar high: a goal-changing decision, an irreversible or costly step, or something only the owner has. Tabs and the thread list show the state as badges, with an optional sound when a thread finishes or fails, and a `verified` badge when its last receipt has no unverified or flagged files (Interface, below).

## Decisions

| Topic | Decision |
| --- | --- |
| Users | The owner only. No onboarding, accounts or team features |
| License | All rights reserved (`LICENSE`), the same as cedia: the repository is public to read and for CI, not open source. Zed's GPL crates link into the binary, so a cedian build is never distributed; it is built for the owner. Third-party parts are listed in `NOTICE` (2026-10-22) |
| Scope | Coding at the core, plus dev tools around it |
| Working mode | Delegate first, then parallel threads, a worktree each |
| Agent | cedia is the only agent |
| Stack | Rust + GPUI as a library; Zed's `editor`, `language`, `lsp`, `terminal` crates as pinned git dependencies where they save work |
| Zed pin | Zed v1.25.2 (`d25fa01`) with toolchain 1.98.1, the same as Zed's. Zed's `[patch.crates-io]` and its `Cargo.lock` are copied at the pin, and `util`'s `debug-embed` feature is on so dev builds embed Zed's assets. A bump moves every Zed rev, the patch section and the lock together |
| Process model | The daemon runs one cedia per prompt (above) and supervises it; the daemon itself restarts with backoff |
| Contract | cedia's `src/protocol/protocol.schema.json` is the source of cedian's Rust types (generated with `typify` in `cedian_protocol`'s build script, strict about unknown fields), and cedia's `test/fixtures/sessions/` are what cedian's replay tests read. `script/pin-cedia <checkout> <rev>` copies both into `crates/cedian_protocol/cedia` with the rev, so a pin bump is one reviewed diff |
| Tools | Agent tools live in cedia. cedian offers host tools only for desktop-owned things (open buffers, the shared terminal's output), never a second tool with a cedia tool's name. The browser tool is cedia's; cedian only hands it the shared Chromium's CDP endpoint |
| Media | Images and video go to a model that reads images: when a thread has media and the code model is text-only, cedia starts a new session of the thread on Haiku 5.5. Deferred to phase D (2026-10-22): the owner does not work mainly with images or video yet |
| Bundling | cedian ships a pinned cedia build (`bun build --compile`) and `rg`, never the copies on PATH |
| Environment | The daemon reads the login shell's environment once at start, so cedia finds `git`, `gh` and `claude` as it does in a terminal |
| Repos | `cedia` and `cedian`. cedia's protocol types are the source; cedian's Rust types follow them |
| Settings | One file, `~/.cedia/settings.json`, with a `desktop` key for cedian (below) |
| Secrets | API keys live in the macOS Keychain. cedia reads them there itself after the environment, so the CLI and the desktop share one store; cedian never passes or copies them |
| Models, auth, permission, sandbox | As cedia's SPEC: API keys only, never stops to ask, `sandbox-runtime` around bash |
| Git | The daemon's own git commands (worktrees, the git pane) run with hooks off (`core.hooksPath` set to `/dev/null`); a cloned repo's hooks are code nobody trusted yet |
| Remote | Protocol ready from day one; remote client later |
| Platform | macOS on Apple silicon; standard macOS keys; macOS dictation in the composer |
| Tests | Replay cedia's recorded fixtures; no model calls in tests |
| Performance | Budgets are gates, measured from the outside the same way on cedian and the Claude desktop app: cold start, keystroke latency, scrolling a 10k-block thread at 120 fps, idle memory |
| Look | Ink & Lime from the app icon, dark by default; Chrome-style project tabs, a tool panel on the right, badges instead of banners; type after the Claude desktop app with open faces (Interface, below) |
| Reactions | cedian draws cedia's `message` and `delivered` events as reactions on your messages; your 👍/👎 on an answer goes in through `cedia threads rate` as an event for picking eval cases and never reaches the model |
| Bots | A bot is a named profile in cedia's own `profiles` settings key (role, model, instructions, tools), started with `--profile`. Start a thread with one, or @mention one to send it work in its own thread through cedia's `message_thread`. Never several bots answering in one conversation (one session, one model) |

## Settings

`~/.cedia/settings.json` is the single source of truth for both programs. cedia owns its schema and precedence (flags > env > user > defaults; a repo's `.cedia/settings.json` only adds deny rules, `zdr` and checks), and a key it does not know fails the load. cedian's own keys live under `desktop` (theme, font, keymap, layout, default view mode, pinned and grouped threads, notifications, browser), which cedia accepts as an object of any shape. The settings UI edits that file, writing only the `desktop` key and replacing the file atomically; the daemon watches it, and the next run reads it. Agent concepts (roles, profiles) are cedia's keys, never `desktop`'s.

## Review and editing

- Review is against the thread's first snapshot, never git HEAD; diffs come from cedia's shadow git in `~/.cedia/snapshots`.
- Rejecting a hunk writes the file in the worktree, then tells the thread with `cedia threads send` (file, hunk, why), so the model sees the change at its next turn.
- A hunk the person edited after the agent is STALE and never rejected.
- When the agent writes a file whose buffer has unsaved edits, the disk keeps the agent's version and the buffer offers Override or Discard; Override tells the thread the same way.

## Interface

Decided 2026-10-22 with the owner over mockups (the "cedian UI design" canvas: agent pane at work, a finished thread with its receipt, diff review, settings, and the tokens). Built on Zed's theme tokens, which the window already draws from.

### Principles

1. Proof over prose: the `verified` badge, which check ran and which thread ran it, and evidence an edit outdated are UI, never buried in text.
2. Money is always visible: live cost and rate on the thread header, cost on every thread row, today's total in the sidebar. Figures are mono with tabular digits.
3. One timeline: sessions started in a terminal and in the window sit in one thread, split by a divider that says where each started; threads started from the CLI carry a `CLI` tag.
4. Quiet until it matters: no spinners, a slowly pulsing dot for working, a caret while text streams; tool cards stay one line unless they fail or carry a diff.
5. Thai is a first language: sizes and line heights are set for Thai marks first.
6. Keyboard first: every action has a key, and nothing waits on an animation.

### Layout

```text
┌ project tabs 42 (title bar, Chrome style): ● ● ● [cedian ●2 (2) ×] [aetheria ●1 (1) ×] [cedia ×] + ┐
├ toolbar 44: folder · worktrees · omnibox ⌘K · Thread ใหม่ ──────────────────────────────────────────┤
├ sidebar 264 ─┬ pane tabs 40 (open threads) ─────────────────────────────────────────────┬─────────┤
│ กำลังทำ      │ agent pane (flex)                   │ tool panel 456 (open tools stack)   │ rail 48 │
│ ต้องดู        │ header: title · $ · Stop             │ editor                              │ editor  │
│ เสร็จแล้ว     │ chips: branch · model · context · view│                                     │ term    │
│              │ timeline (answers ≤ 680)             ├─────────────────────────────────────┤ review  │
│ today $      │ composer with queued messages        │ terminal                            │ ...     │
├──────────────┴──────────────────────────────────────┴─────────────────────────────────────┴─────────┤
│ status bar 26: daemon · cedia rev · runs · keep-awake · cursor · today $                           │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Project tabs**, drawn like Chrome's: one tab per open project (cedia's project, a repository with all its worktrees) on a darker strip in the title bar. A tab is 236 px wide, shrinking when there are many, 34 px tall, with 10 px top corners and curved flares where the active tab joins the toolbar; inactive tabs are transparent with a 1 px divider between them. Each tab has the project's icon and name, a pulsing count of working threads, a badge counting threads that need you (`failed` fill when any failed, else `accent`), and a close ×. `⌘T` or `+` opens a project, `⌘1`–`⌘9` and `⌃Tab` switch, `⌘W` closes a tab without stopping its threads, and tabs drag to reorder.
- **Toolbar** under the tabs, in the active tab's color like Chrome's address bar: the project folder and its worktrees, a rounded omnibox (`⌘K`) that searches threads in every project or starts a thread when you type a task, and Thread ใหม่, the one primary button. Phase C replaces the omnibox with search at the top of the sidebar and removes Thread ใหม่ (below).
- Each project keeps its own sidebar, open threads, tools and layout (`desktop.layout`, per project).
- **Sidebar**: that project's threads in groups, กำลังทำ (working), ต้องดู (failed, interrupted, finished and not reviewed, asking), ปักหมุด (phase C), เสร็จแล้ว (done and stopped, newest first). A row is a state dot, the title, and a line with the state and time, cost, `verified` and `CLI`. A worktree is a branch chip on the thread, never a group. `⌘B` folds the sidebar to a 56 px rail of initials and state dots.
- **Tool panel and rail** on the right: Editor, Terminal, Review (badge: files waiting), Files, Browser, Git, Thread details, and `+` for more (usage, background tasks, MCP in phase C). A click toggles a tool; open tools stack in rail order with draggable dividers, and follow the selected thread (its worktree, its shell, its review). `⌘⌥1`–`⌘⌥7` toggle tools and `⌘⌥0` hides the panel. Any tool drags out into the main area as a full pane, which is how a long review is read.
- Settings is its own window (`⌘,`), as on any Mac app.
- Minimum window 1024 × 640.

### Agent pane

- Header: the title (16/24, 600), live cost with its rate while working, Stop (`⌘.`). Chips: worktree branch, model and role, a context meter (`76k/200k`), and the view mode: คำตอบ, + ความคิด, ทุก tool.
- Blocks: a session divider; the user's message as a right-aligned bubble with its delivery reactions under it; the agent's answer in the serif face, at most 680 px wide; thinking with a dashed border and its duration (hidden in คำตอบ); tool cards with a 30 px header (icon, tool, argument, then exit code, `+n −n` or duration), opened only for a failure's output or an edit's diff, tinted `working` while running, and folded into one chip (`3 tools · read, bash exit 101, edit +6 −1`) in the two narrower view modes.
- Receipt: a card with an `accent` border; a header with `verified` (or a dashed `unverified n`), the verdict, and turns, time and cost; checks with the thread that ran each, and changed files against the first snapshot; next steps (รีวิว diff, เปิด PR, รวมเข้า main) and 👍/👎 for `cedia threads rate`.
- Composer: queued messages sit above the text as chips (ส่งที่ turn ถัดไป, ส่งเลย, แก้, ลบ). Enter sends, Shift-Enter adds a line, `@` mentions a bot, `/` opens commands, and macOS dictation works.
- Thread details (a tool): environment, sessions in order with where each started, cost by model, outputs, and the context meter split into system, tools and messages.

### Diff review

- Header: what it compares against ("snapshot แรกของ thread", never git HEAD), totals, unified or split, progress (`3/7 hunk`), รับที่เหลือทั้งหมด, and Commit และเปิด PR.
- Files (280 px) with `+/−` and a tag: รอรีวิว, รับครบ, `stale n`.
- Each hunk has รับ (`Y`) and ปฏิเสธ (`N`). Reject asks why, then writes the file and sends file, hunk and reason with `cedia threads send`. An accepted hunk folds to a `รับแล้ว` badge with undo. A STALE hunk has the warn tint and says in words why it cannot be rejected.
- Evidence (280 px): for the open file, the checks that cover it after its last edit, and the ones a later edit outdated, with "ให้ thread รัน check ใหม่".
- `J`/`K` move between hunks, `⌘O` opens the file in the editor.

### Settings window

Sections down the left; Bots and Models are marked `cedia`, because those are cedia's keys and are edited through its schema. There is no Save button: each change replaces the file atomically and takes effect at once, and a column on the right shows the file as it will be written, with the changed line marked. Appearance holds the theme (มืด by default, สว่าง, ตามระบบ), the fonts, the size, the density (สบายตา, กะทัดรัด), the default view mode, and the sound.

### Notifications

Badges only, as in other Mac apps, for now; banners wait until the rest of the app is done (owner, 2026-10-22). The project tab badges, the sidebar's ต้องดู group, and the Dock icon's count (threads that need you, across projects), plus an optional sound on finish or failure. A thread needs you when it failed, was interrupted, finished and is not reviewed, or asks a question. Opening it clears its share of every badge.

Threads go on working and check their own work, and ask only when something truly matters (owner, 2026-10-22). cedia cannot ask yet; once it can (below), cedian shows the thread as ถามคุณ, counted in every badge, with the question above the composer.

### Color

Ink & Lime, from the app icon. Lime is the only accent and means verified or primary; nothing decorative uses it. Dark is the default theme.

| Token | Dark | Light |
| --- | --- | --- |
| frame (tab strip) | `#060709` | `#DDE1D8` |
| app | `#0B0D10` | `#F6F7F4` |
| side (sidebar, toolbar, active tab) | `#0E1114` | `#EFF1EC` |
| surface | `#111418` | `#FFFFFF` |
| raised | `#171B21` | `#FAFBF8` |
| hover | `#1E232B` | `#F0F2ED` |
| sel | `#222932` | `#E4E8DF` |
| border | `#242A33` | `#E3E5DF` |
| border.strong | `#313845` | `#CDD1C8` |
| code | `#0D1013` | `#FCFCFA` |
| text | `#E8EAED` | `#0B0D10` |
| text.muted | `#9BA3AF` (7.3:1) | `#545B66` (6.9:1) |
| text.faint | `#7D8592` (5.0:1) | `#667080` (4.5:1 on app) |
| accent fill | `#C6F432` | `#5E8A00` |
| accent text | `#C6F432` (14.4:1) | `#4F7500` (5.4:1) |
| primary button | `#C6F432`, ink text | `#0B0D10`, lime text |
| working | `#5AA9FF` | `#1F6FD1` |
| failed | `#FF6B57` | `#C93B22` |
| warn (interrupted, stale) | `#E8B931` | `#946000` |
| user.bubble | `#1A1F26` | `#F0F2ED` |

Ratios are against `surface` unless noted. Soft tints are the hue at 8–14% alpha, lines at 35–40%. Diff lines carry `+` and `−` as well as their tint, so color is never the only signal.

The tokens ship as two Zed theme files, "cedian Dark" and "cedian Light", loaded in place of `LoadThemes::JustBase`, so the window, Zed's `editor` and the terminal share them: `app` → `background`, `side` → `panel.background`, `surface` → `editor.background` and `elevated_surface.background`, `sel` → `element.selected`, `text.muted` → `text.muted`, `accent` → `text.accent` and `status.success`, `failed` → `status.error`, `warn` → `status.warning`, `working` → `status.info`; syntax keywords `working`, strings `accent`, comments `text.faint`. Tokens Zed has no key for (`frame`, `user.bubble`, the tints) live in one `cedian_window` file derived from the active theme.

### Thread states as drawn

| State | Dot | Word |
| --- | --- | --- |
| working | solid `working`, pulsing 1.4 s | กำลังทำ m:ss |
| done | hollow `text.faint` | time since |
| verified | done, plus the `verified` badge | |
| failed | solid `failed` | ล้มเหลว |
| stopped | solid `text.muted` | หยุดแล้ว |
| interrupted | hollow `warn` | ขาดตอน |
| asking (once cedia can) | `accent` ring | ถามคุณ |

### Type

The owner asked for type in the manner of the Claude desktop app: a plain sans for the interface and a serif for the agent's answers. Anthropic's own faces are not ours to ship, so cedian bundles open (OFL) ones and registers them with GPUI's text system at start, never reading the system's:

- Interface: **Inter**, with **Noto Sans Thai** for Thai.
- Agent answers: **Source Serif 4**, with **Noto Serif Thai** for Thai.
- Code, tool output and terminal: **Lilex** (Zed's), with Noto Sans Thai for Thai.

| Name | Size / line | Weight | Use |
| --- | --- | --- | --- |
| title | 22 / 30 | 600 | settings section title |
| pane title | 16 / 24 | 600 | thread title |
| answer | 15.5 / 26 serif | 400 | the agent's text |
| message | 14 / 24 | 400 | the person's text, composer |
| ui | 13 / 20 | 400 | lists, buttons, inputs |
| meta | 12 / 18 | 400 | chips, secondary lines |
| label | 11 / 16 | 600 | group headers, badges |
| code | 12.5 / 20 | 400 | editor, tool output, terminal |

Thai: line height at least 1.6 times the size for running text, no italics (use weight), capitals only on English labels, and line breaks by dictionary, not spaces. GPUI's line wrapper breaks at spaces and punctuation, so Thai would wrap mid-word; the first step of this work is word segmentation (ICU4X `icu_segmenter`) feeding the agent pane's wrap points, measured against the 10k-block scroll budget.

### Spacing, radius, motion, access

- Spacing 4, 8, 12, 16, 20, 24, 32. Radius 4 (badges, inline code), 6–7 (buttons, chips), 8 (cards, rows), 10 (tab tops), 12 (composer, receipt, window).
- Heights: tab 34, toolbar 44, pane tab 32, tool card header 30, buttons 28 in a pane and 30 in a header, status bar 26.
- Motion: 120 ms ease-out for hover and expand, the working pulse, the streaming caret; under Reduce Motion all of it stops and the working dot stays solid.
- Text meets 4.5:1 at every size. Focus ring: 2 px `accent`. States differ by shape and word as well as color. Every control has a key and a VoiceOver name.

### `desktop` keys

`theme` (`dark` by default, `light`, `system`), `font.ui`, `font.answer`, `font.code`, `ui_font_size` and `code_font_size`, `density` (`comfortable`, `compact`), `view_mode` (`answers`, `thinking`, `all`), `notifications.sound`, `keymap`, `layout` (per project: open tools, panes, sizes).

### Order of work

1. Thai word wrapping, the two Zed theme files, the bundled fonts (phase B, before review and settings land).
2. Agent pane: header, tool cards, receipt, composer with the queue (phase B).
3. Review and the settings window (phase B).
4. Project tabs, toolbar, sidebar groups, tool panel and rail, badges (phase B for one project, phase C for many).
5. Phase C, below: the project chat first (it changes what phase B built), then the browser, usage, background tasks, scheduled runs and MCP.

### Phase C

Decided 2026-10-22 with the owner over the canvas's C1–C4 boards.

- **The person does not start threads.** Each project has a project chat (`⌘0`, the first row of the sidebar), a conversation with the project's coordinator: the person asks there, and the coordinator opens threads with `open_thread`, each in its own worktree, and answers with cards for them (title, branch, a progress bar from the thread's todo list as `3/5`, the current step). `@` sends a message straight to one thread. Thread ใหม่ leaves the toolbar, and the omnibox's last row stops starting a thread; both were phase B's and change with this. Opening a thread from the sidebar still shows its agent pane, and the person can write to it there.
- **Search moves to the top of the thread list**, above the project chat row: the omnibox's filter as an input in the sidebar (`⌘K` focuses it). The toolbar keeps a breadcrumb (folder › thread or แชทโปรเจกต์) and the branch.
- **Questions** come to the project chat as cards: the thread that asks, the question, its answers as buttons with the recommended one filled. The thread's row shows ถามคุณ, counted in every badge, as before.
- **Auto-archive when nothing is owed** (owner's choice): a thread is archived by itself once it is done, its receipt is `verified`, and it has no hunk left to review. A thread that failed, was interrupted, is unverified, asks, or has hunks waiting stays in ต้องดู. The coordinator's card for it turns into a finished card (verified, its PR, `เก็บแล้ว`); the sidebar's foot counts `เก็บอัตโนมัติแล้ว n` and opens the archived list, with เอากลับ. The daemon archives it through `cedia threads archive` and marks the change seen.
- **New tools on the rail**: Outputs (files the project's threads produced, across threads) and Memory (the project's shared memory under `~/.cedia/projects/<slug>/`, read only until cedia offers a write) when the project chat is open; Usage, Background tasks and MCP under `+`.
- **Browser**: one Chromium per workspace that the daemon starts, shared by the person and the agent; cedia's browser tool drives it through its CDP endpoint, and cedian adds none of its own. The pane has back, reload, the URL (a worktree's preview first), mobile and desktop widths, and จับภาพและวาด, which captures the frame, lets the person mark it, and sends the capture with the marks to the thread. A banner says when the agent is using it, with the sites it may open. An agent browser that ships its own tools (ego-lite, looked at 2026-10-22) is not used: it would be a second browser tool beside cedia's, and it is an app, not something a pane can host.
- **Usage**, a full pane: 7 วัน, 30 วัน, เดือนนี้ and a project filter in one row; tiles for today, the range, the average per thread and the share verified; cost per day as one series of bars with a hover readout and a table view; and breakdowns by model, by project and the costliest threads. Figures come from the `costUsd` cedia logs.
- **Background tasks**, a full pane: running sub-threads and bash calls across the project's threads, with the thread each belongs to, the time, and เปิด or หยุด. Under it, scheduled runs (made by asking in the project chat; the daemon wakes the coordinator at the time; each with its next and last run and a switch), and the MCP servers cedia has connected, with their tools and state, configured in cedia's keys of `settings.json`.
- Needs from cedia: `open_thread` and a coordinator profile, per-project memory and files, a todo list event for progress, and schedules (item 4 below).

## What cedia needs to add

Done in cedia PR #19: project identity across worktrees, the `desktop` key with unknown keys failing loudly, the protocol schema and session fixtures, `permission_reply` removed. Done in cedia PR #28 (pinned at `4716402`): `cedia --version --json` with version, protocol hash and capabilities; `threads rate` and `threads unsend` as thread events; a Keychain fallback for API keys (service `cedia`, the env var name as account); each check in a receipt names the thread that ran it; fixture thread ids are stable across re-records. Done in cedia PR #31: a `result` session event at the end of a run's last writer session, carrying the receipt, so a thread rebuilt from cedia's files shows it. Done in cedia PR #35 (pinned at `8ecebb8`): `ask_user`, the `asked` result, the `question` thread event and the `asking` state. The schema's assistant blocks now include a `$ref` member (compaction), so cedian's build inlines `$ref` members of a union before typify, which otherwise gives up on the tag. Done in cedia PRs #33 to #48 (pinned at `f3bd455`): a `turn` thread event (the shadow-git trees before a run's first write and after its last, across every session it took), `threads diff|undo|redo <id>` on that turn, `cacheMisses` and `inputRepairs` on the result, and an `input_repaired` session event. typify made every object refuse unknown members, so a result carrying `cacheMisses` lost its receipt at the old pin; cedian's build now drops `additionalProperties: false` before typify, so a member cedia adds between pins is read past, and only a new event kind is skipped (logged) until the next pin. The build also hashes the pinned schema as cedia does (the first 16 hex digits of its sha256), and the status bar says when the cedia the daemon runs speaks another protocol. Done in cedia PRs #39 to #58 (pinned at `9ad8be7`, protocol `3bd775d0cd73d051`): hooks with a `hook` session event, `run` and `rewind` thread events and a `run` tag on what a run writes, worktree sub-threads, MCP, web, the advisor, a goal and the TUI. cedian folds a rewind as cedia does: the events from the run it goes back to up to the rewind, and anything a dropped run writes later, leave the fold, except messages, unsends, ratings and archiving; the dropped sessions are listed apart (`rewound`), so opening the thread replays only the conversation it went on with. A message taken back after a session was shown it stays, as in cedia. A rewind button waits with the rest of whole-turn review. cedian moves its pin when cedia lands a feature (a new capability), not at every commit. The thread list carries the `turn` as `last_turn`; undoing a whole turn is cedia's (`cedia threads undo`), and cedian's review only adds rejecting one hunk. Still to add, in the order cedian needs them:

1. For the agent pane's header: a running cost on each `assistant` event (today only the result carries `costUsd`, so the header shows finished runs only), the model's context window (the meter shows `76k`, not `76k/200k`), and when a thinking block started and ended (its duration).
2. `threads delete`, and a `title` event for rename (phase C).
3. `profiles` and `--profile` (phase C).
4. `open_thread` for a coordinator, per-project memory and files, a todo list event for a thread's progress, and scheduled runs the daemon can wake (phase C).
5. `cedia serve` with `host_tools`, `host_call` and `host_result`, once a host tool is needed (phase C).
5. `prompt.images` and the media route to Haiku 5.5 (phase D, deferred).
6. Media references (video frames, original file) in tool results (phase D).

## Projects

cedia's threads already follow Claude Projects: a thread is one line of work made of sessions joined by handoff and `recall`, and threads message each other. cedian adds the project layer above them: a project pane with a coordinator thread (a `coordinator` profile that opens threads with `open_thread`, each in its own worktree), pause for everything the coordinator opened, shared memory and files under `~/.cedia/projects/<slug>/`, an outputs card across threads, and each thread's todo list drawn as a checklist on its tab.

## Roadmap

| Phase | cedian | cedia | Exit gate |
| --- | --- | --- | --- |
| 0 Clean | Repo cleared | Contract (PR #19) | Both repos clean; schema and fixtures merged (done) |
| A Window | Spike first: GPUI window plus Zed `editor` and `terminal` compiling at one pinned rev, with Zed's `[patch]` entries copied and the toolchain matched; compiled cedia passing its sandbox tests on a Mac. Then the agent pane over the fixtures | Stage 2 | A fixture renders; replay test green; performance budgets measured |
| B One live thread | Daemon as a LaunchAgent, runs per prompt, reconcile and keep-awake; terminal (attach selection), review, settings UI; message queue with Send now; receipt card with next steps and the verified badge; reactions; view modes, context meter, thread details; window and layout restored after quit; worktree setup with progress and branch names from the first message; open in other apps | `--version --json`, rate, unsend, Keychain, which thread ran each check (done in cedia PR #28) | Delegate a task into a worktree, quit and reopen with the thread intact, review and accept |
| C Parallel | Many threads, tabs by state, notifications, search, pin and group; shared Chromium with capture and annotate; project pane with coordinator; bot profiles and @mention; side chat, background tasks pane, task chips, app preview with per-site approval, safe archive; file tree; usage page by day, model and project; storage view; export; scheduled runs; Open With from Finder; MCP card | Stage 5: sub-agents, MCP, web, host tools, profiles, `open_thread` | Three threads at once, memory measured |
| D Media, depth | Video and image panes, editor with LSP and line comments, git pane, surface protocol, pop-out panes | Stage 6, prompt images and the media route, video read | The agent fixes a bug shown in a screen recording |
| Later | Remote over QUIC, computer use via `cua-driver`, voice mode | Classifier, goal mode | |

## Phase A spike

Done on Linux (2026-10-22): `cedian_window` opens a GPUI window with Zed's `editor` above a login shell from Zed's `terminal` crate, drawn as plain text lines. It builds and runs under Xvfb with Mesa's software Vulkan, and writes no Zed data directories. What it showed:

- `editor` is not a small crate: it pulls in Zed's `workspace`, `project`, `client`, `rpc`, `dap` and `db`, about 1,050 crates in all. A clean debug build takes about 5 minutes on 4 cores, and a rebuild after editing cedian's own crate about 20 seconds, so the size costs disk (a 5 GB `target`) more than time.
- A release build takes about 16 minutes and gives a 119 MB stripped binary on Linux. Idle memory with the window open is about 180 MB under software Vulkan, which is not yet the macOS number the performance budget needs.
- `terminal` stands alone (26 direct dependencies). `terminal_view` needs `workspace`, so cedian draws the terminal's cells itself.
- Zed's crates read their assets from the Zed checkout at run time in debug builds. `util/debug-embed` turns that off, so no dev build depends on a checkout path.

The agent pane followed on the same day. `cedian_agent` folds cedia's stream-json lines into blocks (user, text, thinking, tool call with its permission and result, session, receipt). Streamed text shows until the turn's assistant event replaces it, and a sub-thread's events, which arrive in the parent's stream, go to a transcript of their own, keyed by thread. `cargo test -p cedian_agent` replays every pinned fixture against an outline in `tests/replay/`, and `cedian --replay <stream.jsonl>` plays one into the window a line at a time.

On the owner's Mac (Apple silicon, same day):

- A debug build of the window needs full Xcode for GPUI's Metal shaders; with Command Line Tools only it fails with `unable to find utility "metal"`. `cargo build -p cedian_window --features runtime_shaders` builds without Xcode (about 3 minutes clean) and compiles the shaders at launch instead. Builds that ship leave the feature off and need Xcode.
- The window opens and runs without a panic, at about 100 to 130 MB resident and under 2% CPU. With the replay of a fixture, screenshots show the agent pane (tool cards, receipt with the verified badge) beside Zed's editor and a live zsh. Cold start, from launch until the window is on screen, has a median of 285 ms over three runs of the debug build; the first launch after a build takes about 6 s while macOS checks the new binary.
- A sub-thread's receipt lists the same check twice, because cedia's evidence counts the sub-thread's run of it and the parent's. The fix is in cedia: say which thread ran each check.
- cedia at the pin compiles with `bun build --compile` in under a second to a 72 MB binary. Its sandbox tests pass (13 of 13), and the compiled binary runs sandboxed bash under `--sandbox on` while a write outside the working directory fails with `Operation not permitted`.

Performance budgets, first measurement (2026-10-22, release build with `runtime_shaders`, Apple silicon). Each number is taken from outside the app: keys and scroll events are posted as HID events, and the window's pixels are polled with `CGWindowListCreateImage`. That polling caps the resolution at about 14 ms a frame, so these are upper bounds that cannot tell 60 Hz from 120 Hz. ScreenCaptureKit is the next step.

| Budget | cedian | Claude desktop |
| --- | --- | --- |
| Cold start, launch to window on screen | median 194 ms; 268 ms with a 10k-block thread loaded | not measured: quitting it would end the session doing the measuring |
| Keystroke to pixels, editor | median 36 ms, p95 43 ms (30 keys) | not measured: it was in use |
| Scrolling a 10k-block thread | changed on 87% of captures at about 67 captures/s; longest gap 33 ms | not measured |
| Idle memory, 10k-block thread open | about 127 MB, under 1% CPU | about 1,020 MB across 14 processes |

The release binary is 117 MB and a clean release build takes about 10 minutes on the Mac. Typing works in the editor and the shell, including Thai: the shell takes text from the platform input handler, as Zed's terminal does, and draws combining marks with their base character.

## Phase B

The daemon came first (2026-10-22). Three crates carry it:

- `cedian_index` reads cedia's files: the project key (the main worktree's root, slugged as cedia does), the project's threads folded as cedia folds them, and a thread's session logs as stream-json lines.
- `cedian_wire` is what the window and the daemon say over the socket: one JSON object per line, requests (`threads`, `open`, `close`, `prompt`, `send`, `stop`, `review`, `decide`, `settings`, `set_desktop`) one way and notices (`welcome`, `threads`, `preparing`, `started`, `line`, `caught`, `status`, `review`, `settings`, `error`) the other.
- `cedian_daemon` (binary `cedian-daemon`) listens on `daemon.sock` in cedian's state directory (`~/Library/Application Support/cedian` on macOS), never under `~/.cedia`. It reads the login shell's environment once, as Zed does (the shell runs the daemon with `--printenv`), and asks cedia for `--version --json`. Each prompt is one `cedia --print=<text> --output-format stream-json --cwd <dir> [--thread <id>]`; the run's first session event names its thread. A prompt to a thread whose run is alive goes in with `cedia threads send` instead, and Stop sends SIGTERM. The daemon keeps each thread's latest run's lines, so a window that opens the thread mid-run, or after it ends, gets all of it; older sessions come from cedia's session logs.

The window connects to the daemon and starts it when nobody answers, in its own process group so quitting the window leaves it running. `cedian --cwd <dir>` shows that project's threads, opens the latest, and has a composer under the agent pane (Enter sends, Shift-Enter adds a line). Under Xvfb, with a fake cedia that plays the recorded fixtures (`crates/cedian_daemon/tests/fake/cedia`), a prompt typed in the composer runs and draws live, the thread is intact after the window quits and reopens, and after the daemon too is killed the thread rebuilds from cedia's files. On the owner's Mac the same four checks passed against the compiled cedia on its faux model (`CEDIA_HOME` and the state directory in a scratch directory), with the window on screen 217 ms after launch and the daemon's start (login environment and `cedia --version`) hidden inside the window's opening.

Then, the same day: the receipt survives a daemon restart, read from the `result` event cedia now logs. While any run works the daemon holds `caffeinate -i -w <daemon pid>`, so the Mac does not idle-sleep under a delegated thread. `cedian-daemon install` writes `~/Library/LaunchAgents/dev.cedian.daemon.plist` (run at login, restarted ten seconds after a crash, not after a clean exit, which means another daemon already serves) and loads it with `launchctl bootstrap`; `uninstall` undoes both. A bundled app will register the same agent with `SMAppService`. launchd starts agents without `SHELL`, so the daemon takes the account's shell from the password database. Reconcile followed. cedia runs in its own process group, so a run outlives a daemon that crashes or that launchd restarts. The daemon keeps the live runs (pid, thread, cwd, the thread's sessions before the run) in `runs.json` in its state directory; the next daemon checks each pid is still a stream-json cedia run, follows it by tailing the run's session logs (no streamed text, every logged event), and ends it when the pid goes. Stop still signals it. Worktrees followed. A new thread runs in its own worktree unless the window turns it off: `<main root>/.cedian/worktrees/<name>` on a new branch `cedian/<name>` from the main worktree's HEAD, the name taken from the ASCII words of the first message (at most 40 characters, `thread-<secs>` when there are none, `-2`, `-3` when taken). The daemon adds `/.cedian/` to the repository's `info/exclude`, runs git with hooks off, and sends `preparing` notices the window shows in the thread's title. What git ignores in the main worktree (dependencies, build output, `.env`) is cloned into the new worktree, so a thread starts ready to build and test: APFS `clonefile` on macOS, `cp --reflink=always` on Linux, which share blocks until one side writes and so cost no time or space. Where the filesystem cannot clone, nothing is copied rather than gigabytes of `target/` or `node_modules/`, and the thread installs its own. The idea is LayerFS's instant bootstrap without its Linux containers; cedia's own sub-thread worktrees link these directories read only, which is cedia's to change. A resumed thread runs in its latest session's cwd, so it stays in its worktree. Review followed, functional and minimal so the interface spec can restyle it. `cedian_review` reads cedia's shadow git for the thread's directory (`~/.cedia/snapshots/<slug of that directory>`, read only: cedian never adds to it) and diffs the first tree the thread's sessions there logged against the last, with three lines of context. A hunk is STALE when the file on disk no longer holds its new side (context included), so a hunk the person edited is never rejected. Reject puts the hunk's old lines back in the file (removes a file the agent added, restores one it deleted) and tells the thread with `cedia threads send`: the file, the hunk and the reason. Accept and undo only mark the hunk. Decisions live in the daemon's state directory (`review/<thread>.json`), keyed by a hash of the file and hunk text. The window's right column has a Review tab: J and K move, Y accepts, N opens the reason box, Enter rejects. The settings screen followed, opened from the thread list. The daemon reads `desktop` from `~/.cedia/settings.json` (`$CEDIA_HOME` when set) and sets one key at a time, rewriting the file through a rename with every other key kept in its order; each window is sent the new `desktop` and applies it at once. It has the theme (dark by default, light, or follow the system) and the interface and code text sizes, put into Zed's settings store in memory so Zed's own settings file is never touched. The daemon does not watch the file yet: a window reads it again when it connects or opens the screen. Then the layout: `desktop.layout` holds the window's bounds and state, the right column's tab (now the open tools), and the thread each project had open. The window reads `desktop` from the file once at launch, so the first frame has the saved theme, bounds (when a display still shows most of them) and tab; changes go back through the daemon half a second after the last one, so dragging the window writes the file once. Threads made or changed outside the daemon followed: once a second the daemon fingerprints the threads directory (file names, sizes and times) of each project a window shows, and sends that project's list again when it moved, so a `cedia -p` or `cedia threads archive` in a terminal shows up without a reload. Then type and color, the first step of the interface's order of work: the window bundles static instances of Inter and Noto Sans Thai (400, 500, 600, 700) and Source Serif 4 (optical size 16) and Noto Serif Thai (400, 600, 700), cut from Google Fonts' variable files by `script/fonts/instance.py` with plain family names so GPUI matches them by family and weight, each with its OFL; Lilex comes from Zed's assets. cedian's assets sit over Zed's (`assets.rs`), so the fonts and themes need no files beside the binary. `script/themes/cedian.py` writes "cedian Dark" and "cedian Light" from the Ink & Lime table (one Zed theme family, `assets/themes/cedian.json`); `tokens.rs` holds what Zed has no key for. `desktop.theme` picks between them, and the interface font, the code font and their Thai fallbacks go into Zed's settings store with the sizes. theme_settings starts its observer a cycle late, so the window reloads the theme itself after applying `desktop`; until then a launch showed the system's default. Thai wrapping followed. GPUI's wrapper treats every Thai letter as a break, so it split words and even a consonant from its vowel. Text with Thai in it is cut where ICU4X's dictionary line segmenter allows a break (a space after a Thai word stays with the word, as after an English one), filled into lines one whole piece at a time by measuring with GPUI's text system, and each line shaped and painted on its own (`line_breaks.rs`), so a line's Latin and Thai runs share one baseline. Text without Thai keeps GPUI's wrapping. The agent's answers, thinking, the person's messages and the receipt use it; the composer is Zed's editor, which wraps by itself. The sidebar's groups and badges followed. The daemon keeps, per thread, the `updated_at` a window last showed (`seen.json` in its state directory): opening a thread marks it, and so does any change while a window has it open. A thread needs you when it is done, failed, interrupted or asking and changed since it was seen; one never seen counts only when it failed, was interrupted or asks, so the threads from before cedian do not all light up. `ThreadInfo.needs_you` carries it. The sidebar (264 px) groups threads into กำลังทำ, ต้องดู (asking, failed, interrupted, and done but not seen) and เสร็จแล้ว, each row a state dot drawn as the table above (solid, hollow or a ring), the title (semibold while it needs you), the state word in Thai and the time since; a group header counts what needs you in a pill, `failed` when one of them failed, else `accent`. The window's title carries the same count until project tabs and the Dock badge land. Settings then moved to its own window, as the spec has it: `⌘,` (Zed's keymap binds it to `zed::OpenSettings`; `ctrl-,` on Linux), the app menu's Settings… or the sidebar's link open it or bring it forward. It shows the main window's settings view, so the daemon's `settings` notices reach it over the one connection. The main window opens no smaller than 1024 × 640. The project tab strip followed, for one project: on macOS the title bar is transparent and the strip (42 px, `frame`) is drawn under the traffic lights; Linux keeps its window manager's title bar above it. The tab (236 px, 34 px, 10 px top corners, in `side` so it joins what is under it) has the project's name, a count of working threads, and the badge of threads that need you. The strip moves the window when dragged and zooms it on a double click. More tabs followed (below). The Dock icon carries the same count (`NSDockTile.badgeLabel`, cleared at zero), so badges are complete: the tab, the sidebar group, each row's dot and the Dock. A thread in ต้องดู that the person has opened stays in the group (it still failed, or still asks) but leaves the counts. The settings window has the same dark bar on macOS in place of the system's light title bar, and the question card is drawn in the theme's text color with an `accent` border, the recommended answer filled. The agent pane followed, step 2 of the order of work. Its header has the title (16/24, 600), the state, the cost of the thread's finished runs, and Stop (`⌘.`, `ctrl-.` on Linux, bound in the composer too, where Zed's keymap gives the key to code actions); its chips are the branch when the thread runs in a worktree the daemon made, the latest session's model and role, the tokens the latest turn held, and the view mode, kept in `desktop.view_mode`. A session starts with a divider; the person's message is a right-aligned bubble; answers are in the serif at most 680 px wide, with a caret while they stream; thinking has a dashed border and no italics, hidden in คำตอบ. A tool call is a card with a 30 px header (icon, tool, argument, then `+n −n` for an edit or a write, or the exit code and the time from intent to result), tinted `working` while it runs, and opened only for a failure's output or an edit's diff (from the call's input). In คำตอบ and + ความคิด each run of calls folds into one chip (`3 tools · read, bash exit 101, edit +6 −1`) that opens on a click. The receipt has the `accent` border, `verified` or a dashed `unverified n`, the verdict in Thai, turns, time and cost, the checks with the thread that ran each when it is another, the changed files, รีวิว diff, and 👍/👎 on the thread's latest receipt: the daemon runs `cedia threads rate`, and the thread list carries the latest rating back. เปิด PR and รวมเข้า main wait for the git tool. The composer followed: a rounded field under the timeline, and above its text the person's messages the running thread has not seen yet (the thread's `pending`), each a chip with ส่งที่ turn ถัดไป, แก้ (takes it back with `cedia threads unsend` and puts its text in the composer) and ลบ (takes it back). ส่งเลย waits for cedia to deliver a message mid-turn; stopping the run to send it would throw away the turn's work. The review followed, step 3: its header says what it compares against (the thread's first snapshot, never git HEAD), the totals and `decided/total hunk`, with รับที่เหลือทั้งหมด; each file has `+n −n` and a tag (รอรีวิว, รับครบ, `stale n`); a hunk is a card with รับ (`Y`) and ปฏิเสธ (`N`), folds to a `รับแล้ว` badge with ย้อนกลับ once accepted, and a STALE hunk has the warn tint and says in words why it cannot be rejected. In the right column the files stack; widened into the main area (the tool column gives way) it has the files at 280 px, the open file's hunks, and the evidence at 280 px: the latest receipt's checks that ran after the file's last edit by the agent, and the ones an edit came after, with ให้ thread รัน check ใหม่, which sends the thread the command and the file. The window works this out from the transcript (`Transcript::file_checks`); a check the transcript never shows running, which the done gate ran at the end, counts as covering. Unified or split, Commit และเปิด PR and `⌘O` wait for the git tool and the editor's file opening. The settings window followed: sections down the left (รูปลักษณ์, and Bots and Models tagged `cedia`, which say they wait for cedia's schema and are edited in the file meanwhile), the open section's controls, and on the right the `desktop` key as written, with the line the last change set marked. Appearance has the theme (มืด first, the default; สว่าง; ตามระบบ), the interface and code sizes, the density (สบายตา, กะทัดรัด: the space between the agent pane's blocks, `desktop.density`), the default view mode, and the bundled faces, named; choosing other faces waits until there is more than one face to choose; the sound followed (below). The toolbar, the tool panel and the status bar followed, step 4 for one project. The toolbar (44 px, `side`) has the project's folder, the open thread's worktree branch (`main checkout` when it runs in the main worktree), the omnibox and Thread ใหม่. The omnibox (`⌘K`, `ctrl-k` on Linux, bound in the composer too) lists the project's threads whose title holds every word typed, at most six, and a last row that starts a thread with the text; up and down move, Enter opens or starts, Escape goes back to the composer. With several tabs it searches every tab's threads, the front tab's first, and names the project of a thread from another tab; opening one brings its tab forward. The rail (48 px) has Editor, Terminal, Review (its badge counts hunks waiting) and รายละเอียด thread; a click or `⌘⌥1`–`⌘⌥4` (`ctrl-alt` on Linux) toggles one and `⌘⌥0` closes them all. Open tools stack in the panel (456 px) in rail order, each under a 28 px label with a close ×, in equal heights; `desktop.layout.tools` keeps the list (Editor and Terminal by default), replacing the right column's tab, which is still read once. A widened review leaves the panel the other tools and keeps the rail; Review's button brings it back. Thread details has the thread's title and id, where it runs, its sessions in order with their role and model, the cost by model, the context of the latest turn and the files it changed. Browser and `+`, and dragging a tool into the main area wait; Files and Git followed (below). The status bar (26 px) has the daemon's dot and state, the cedia it runs, `n กำลังทำ`, `n ต้องดู` and Settings, which took the sidebar's link; the sidebar lost its header and footer to the toolbar and the status bar. Keep-awake, the cursor and today's cost wait for the daemon to report them. The terminal then followed the open thread: a shell per directory, started the first time the terminal shows it (the thread's worktree when it is on this machine, else the project), so going back to a thread finds its shell as left. Its grid is sized to the pane from the code font's cell width, so the shell wraps where the pane does, and a drag selects (Shift keeps it local when a program takes the mouse). `⌘L` (`ctrl-shift-l` on Linux, where `ctrl-l` clears the shell) puts the selection in the terminal or the editor into the composer as a fenced block. เปิดใน, beside the worktree chip, opens the same directory in the file manager or in an editor or terminal found on the machine (VS Code, Cursor, Zed, Ghostty, Terminal: an app bundle on macOS, a command on `PATH` elsewhere). The panel's dividers then became draggable: a 5 px strip over the line between two tools moves their shares (neither below 15% of the pair, each at least 60 px), and `desktop.layout.tool_sizes` keeps each tool's share by key. `⌘B` (`ctrl-shift-b` on Linux, where `ctrl-b` is the shell's) folds the sidebar to a 56 px rail: each thread as its first letter over its state dot, in the sidebar's order, the open one filled and the ones that need you ringed; `desktop.layout.sidebar` keeps it. Project tabs then went from one to many. Each tab is a project (`project.rs`): its directory, the threads the daemon last listed for it, the thread it shows, whether a new thread is starting, and its shells; the agent pane, composer, editor and review are the window's and show the front tab. A tab behind still gets its list, states and badges from the daemon (the window asks `threads` for every tab, and the daemon already watches each project a window asked for), but not its thread's lines: bringing a tab forward closes the thread it leaves and opens its own again, so the daemon replays it, and a thread that starts in a tab left behind is closed until its tab comes back. `+` and `⌘T` (`ctrl-shift-t` on Linux) open a folder from the system's picker, or bring forward the tab that already has it; × and `⌘W` (`ctrl-shift-w`) close a tab and its shells without stopping its threads, and the last tab stays. `⌘1`–`⌘8` pick a tab, `⌘9` the last, `⌃Tab` and `⌃⇧Tab` step round (`ctrl-1`… on Linux). `desktop.layout.projects` keeps the tabs in order and `desktop.layout.project` the front one; at launch the saved tabs whose folder is still there open, `--cwd` adds its folder (or brings its tab forward), and with neither the current directory opens alone. The window's title has the front tab's count; the Dock and the status bar add up every tab's. A tab drags to a new place: dropped on another tab it takes that tab's place, the front tab stays in front, and the new order is saved. The Files tool followed, between Review and Thread details on the rail (`⌘⌥4`; the rail's keys follow each tool's place in the list above, so Browser keeps `⌘⌥5`, Git is `⌘⌥6` and Thread details moves to `⌘⌥7`): the open thread's directory as a tree (its worktree when it has one, else the project), folders first, each opened or folded by a click. In a git repository it lists what git would, tracked files and untracked ones not ignored (`git ls-files --cached --others --exclude-standard`, without files deleted from the worktree); elsewhere it walks the directory past dot folders, at most 50,000 files. It is listed again when the directory changes and when a run of the open thread ends. A click on a file shows it in the Editor, read only and named in the Editor's label, opening the Editor if it is closed; a binary file or one over 2 MB is named, not shown. Saving waits for the editor to open files as buffers. The Git tool followed, read only: the directory's branch with its upstream and how far ahead and behind, the files changed against HEAD with git's two status letters (untracked files one by one), and the last ten commits; a click on a changed file shows its diff against HEAD in the Editor (an untracked file against nothing). It is read again with Files. Commit, push and pull requests stay off it: they are cedia's `/commit` and `/pr`, and a button for them waits for cedia to offer them to a client. Pinning followed: a pin on a thread's row (shown on hover, and always on a pinned one) puts the thread in ปักหมุด, between ต้องดู and เสร็จแล้ว, while it is neither working nor waiting on the person; a pinned thread that is working or needs you still shows in กำลังทำ or ต้องดู. `desktop.layout.pinned` keeps the pinned threads by project directory, since thread ids repeat across projects. The sound followed: `desktop.sound` (Settings, Appearance: ปิด by default, เปิด) plays the system's own sound when a run that was working ends, Glass when it is done or asks, Basso when it fails (`afplay`; `canberra-gtk-play` with `complete` and `dialog-error` on Linux when it is installed). A stop or an interrupted run is silent. Threads in a tab behind get no state notices, so the window also compares each project's new list with the last one, and a run rings once, on whichever comes first. Archiving followed: a box on a thread's row (shown on hover, not on a working thread) archives it through `cedia threads archive` (the daemon's `archive` request), the thread open in the pane giving way to a new one, and for ten seconds the sidebar offers เลิกทำ, which runs `cedia threads unarchive`. Archiving is the person's own change, so a thread that did not need them before it still does not after: the daemon marks that change seen. The archived threads followed, at the foot of the sidebar: `เก็บแล้ว n` (only when there are any) opens a list of the tab's archived top-level threads, last changed first, each with the time since and เอากลับ, which runs `cedia threads unarchive`. The window has them from the same list as the rest; the daemon sends archived threads too. Export followed: ส่งออกเป็น Markdown, under Thread details, writes the open thread to a file the person picks (the system's save panel, in Downloads with a name from the title) and shows it in the file manager. The text is the transcript the pane holds, as คำตอบ reads it: each session's model and role, the person's messages quoted, the answers, each run of tool calls as a list with its argument and outcome, and each receipt's verdict, turns, cost and files; thinking and sub-threads stay out, since cedia's files keep the whole record. The window writes this file itself, the one file it writes, because it is the person's copy and not state. Phase C began with search: the omnibox left the toolbar for an input at the top of the sidebar (`thread_search.rs`, `⌘K`, `ctrl-k` on Linux, unfolding a folded sidebar first). While it has text the sidebar lists the matching threads of every tab, the front tab's first and another tab's named beside its state, in place of the groups; Up, Down, Enter and Escape work as before, and it no longer starts a thread. The toolbar keeps a breadcrumb (folder › the open thread's title), the branch and เปิดใน. Thread ใหม่ stays until the project chat can open threads, since until then it is the only way to start one.

## Learned from the Claude desktop app

From Anthropic's public [desktop docs](https://code.claude.com/docs/en/desktop), the [Cowork changelog](https://claude.com/docs/cowork/changelog) and first-hand use, never from the app's code (Anthropic's Consumer Terms §3 forbid reverse engineering). Each maps onto something cedia has or the list above adds:

- Steering: text typed while a thread works goes in as a cedia `message`, delivered at the next turn boundary; queued messages can be edited, removed or sent now.
- View modes (answers, plus thinking, plus every tool call), a context meter from `usage`, and a thread details card (environment, context, outputs, changes, servers, background tasks).
- Side chat: cedia `fork` at the current event into a throwaway session. Editing the last message is a fork before it.
- Background tasks pane over sub-threads and running bash calls; task chips that open a new thread with its own worktree.
- App preview: the browser pane runs the project's preview command from `settings.json` and captures after writes, with a per-site allow list.
- Archive checks a worktree for uncommitted changes first; pop-out panes; keep-awake while working; recovery after a crash or sleep.

Where cedian aims to beat it rather than match it: proof of done from the log, cost per thread with fallback instead of waiting for limits, one timeline for runs started in a terminal or the window, a daemon that outlives the window, secrets redacted before the model sees them, and your ratings feeding cedia's evals.

## Ideas carried over from the earlier cedian

- Evidence is bound to code state and goes stale after a later edit.
- One browser per workspace, shared by person and agent, with captures bound to a frame.
- A normal turn has no ceremony; extra checks engage by risk.
