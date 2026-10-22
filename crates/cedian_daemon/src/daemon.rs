//! The daemon's state: connected windows, runs, and which window reads which thread. Every
//! piece can be rebuilt from cedia's files except the lines of a run in progress.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, mpsc::Sender},
};

use cedian_index::{
    ThreadSummary, list_threads, project_key, read_thread, session_cwd, session_lines,
    session_path, threads_dir,
};
use cedian_protocol::{Line, SessionEvent, VersionInfo};
use cedian_wire::{Notice, Request, Status, ThreadInfo};

use crate::{
    Cedia,
    desktop_settings::{read_desktop, set_desktop_key, settings_path},
    follow::{Offsets, follow},
    keep_awake::KeepAwake,
    ledger::{Ledger, RunRecord, still_running},
    review::{Changes, DecisionStore, decide},
    run::{Run, RunId, watch},
};

pub type Shared = Arc<Mutex<Daemon>>;
pub type ClientId = u64;

/// The state stays usable after a panic in another thread; each change to it is one step.
pub fn lock(shared: &Shared) -> MutexGuard<'_, Daemon> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct Client {
    tx: Sender<Notice>,
    /// Threads whose lines this client reads.
    open: HashSet<String>,
    /// Projects whose thread lists this client shows: cwd to project key.
    watching: HashMap<PathBuf, String>,
}

pub struct Daemon {
    cedia: Cedia,
    home: PathBuf,
    version: Result<VersionInfo, String>,
    clients: HashMap<ClientId, Client>,
    runs: HashMap<RunId, Run>,
    /// Each thread's latest run.
    by_thread: HashMap<String, RunId>,
    keep_awake: KeepAwake,
    ledger: Ledger,
    decisions: DecisionStore,
    next_id: u64,
}

impl Daemon {
    pub fn new(cedia: Cedia, home: PathBuf, state_dir: &Path) -> Self {
        let version = cedia.version();
        match &version {
            Ok(version) => log::info!("cedia {} at {}", version.version, cedia.bin.display()),
            Err(error) => log::warn!("{error}"),
        }
        Self {
            cedia,
            home,
            version,
            clients: HashMap::new(),
            runs: HashMap::new(),
            by_thread: HashMap::new(),
            keep_awake: KeepAwake::default(),
            ledger: Ledger::new(state_dir),
            decisions: DecisionStore::new(state_dir),
            next_id: 1,
        }
    }

    /// Takes over the runs a previous daemon left going, following each through its session
    /// logs until its process ends.
    pub fn adopt(shared: &Shared) {
        let mut daemon = lock(shared);
        let mut adopted = Vec::new();
        for record in daemon.ledger.load() {
            if !still_running(record.pid) {
                continue;
            }
            let id = daemon.id();
            log::info!(
                "run {id}: following pid {} of {}",
                record.pid,
                record.thread
            );
            daemon.runs.insert(
                id,
                Run {
                    asked_in: record.cwd.clone(),
                    cwd: record.cwd,
                    slug: record.slug,
                    thread: Some(record.thread.clone()),
                    pid: record.pid,
                    alive: true,
                    lines: Vec::new(),
                    before: record.before.into_iter().collect(),
                    requester: None,
                    stderr: Default::default(),
                    followed: Some(Offsets::default()),
                },
            );
            daemon.by_thread.insert(record.thread, id);
            adopted.push(id);
        }
        daemon.save_ledger();
        let working = !adopted.is_empty();
        daemon.keep_awake.set(working);
        drop(daemon);
        for id in adopted {
            follow(shared.clone(), id);
        }
    }

    /// Writes the live runs with known threads, for the next daemon to adopt.
    fn save_ledger(&self) {
        let records: Vec<RunRecord> = self
            .runs
            .values()
            .filter(|run| run.alive)
            .filter_map(|run| {
                Some(RunRecord {
                    pid: run.pid,
                    thread: run.thread.clone()?,
                    cwd: run.cwd.clone(),
                    slug: run.slug.clone(),
                    before: run.before.iter().cloned().collect(),
                })
            })
            .collect();
        if let Err(error) = self.ledger.save(&records) {
            log::warn!("{error:#}");
        }
    }

    /// One poll of a followed run: new session log lines go to its readers, and a process that
    /// is gone ends the run. Returns whether to poll again.
    pub fn poll_followed(&mut self, id: RunId) -> bool {
        let Some(run) = self.runs.get(&id) else {
            return false;
        };
        let (Some(thread), pid) = (run.thread.clone(), run.pid) else {
            return false;
        };
        let running = still_running(pid);
        let sessions = match read_thread(&threads_dir(&self.home, &run.slug), &thread) {
            Ok(summary) => summary.sessions,
            Err(error) => {
                log::warn!("{error:#}");
                Vec::new()
            }
        };
        let paths: Vec<PathBuf> = sessions
            .iter()
            .filter(|s| !run.before.contains(&s.session_id))
            .map(|s| session_path(&self.home, &run.slug, &s.session_id, s.path.as_deref()))
            .collect();
        let mut lines = Vec::new();
        if let Some(Run {
            followed: Some(offsets),
            ..
        }) = self.runs.get_mut(&id)
        {
            for path in &paths {
                lines.extend(offsets.read_new(path));
            }
        }
        for line in lines {
            self.on_line(id, line);
        }
        if !running {
            self.on_exit(id, "ended while another daemon had it".to_owned());
        }
        running
    }

    fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn connect(&mut self, tx: Sender<Notice>) -> ClientId {
        let id = self.id();
        self.clients.insert(
            id,
            Client {
                tx,
                open: HashSet::new(),
                watching: HashMap::new(),
            },
        );
        self.send(
            id,
            Notice::Welcome {
                daemon: env!("CARGO_PKG_VERSION").to_owned(),
                cedia: self.version.clone(),
            },
        );
        id
    }

    pub fn disconnect(&mut self, id: ClientId) {
        self.clients.remove(&id);
        for run in self.runs.values_mut() {
            if run.requester == Some(id) {
                run.requester = None;
            }
        }
    }

    fn send(&mut self, id: ClientId, notice: Notice) {
        let gone = match self.clients.get(&id) {
            Some(client) => client.tx.send(notice).is_err(),
            None => false,
        };
        if gone {
            self.disconnect(id);
        }
    }

    fn error(&mut self, id: ClientId, message: String) {
        log::warn!("client {id}: {message}");
        self.send(id, Notice::Error { message });
    }

    /// Sends a notice to every client that has the thread open.
    fn tell_readers(&mut self, thread: &str, notice: &Notice) {
        let readers: Vec<ClientId> = self
            .clients
            .iter()
            .filter(|(_, client)| client.open.contains(thread))
            .map(|(id, _)| *id)
            .collect();
        for id in readers {
            self.send(id, notice.clone());
        }
    }

    fn running(&self, thread: &str) -> bool {
        self.by_thread
            .get(thread)
            .and_then(|id| self.runs.get(id))
            .is_some_and(|run| run.alive)
    }

    fn threads(&self, slug: &str) -> anyhow::Result<Vec<ThreadInfo>> {
        Ok(list_threads(&threads_dir(&self.home, slug))?
            .into_iter()
            .map(|thread| ThreadInfo {
                status: Status::of(thread.state, self.running(&thread.id)),
                cwd: self.thread_cwd(slug, &thread),
                thread,
            })
            .collect())
    }

    /// The directory the thread's latest session ran in, if it is still there.
    fn thread_cwd(&self, slug: &str, thread: &ThreadSummary) -> Option<PathBuf> {
        let session = thread.sessions.last()?;
        let path = session_path(
            &self.home,
            slug,
            &session.session_id,
            session.path.as_deref(),
        );
        session_cwd(&path).filter(|cwd| cwd.is_dir())
    }

    fn status(&self, slug: &str, thread: &str) -> Status {
        match read_thread(&threads_dir(&self.home, slug), thread) {
            Ok(summary) => Status::of(summary.state, self.running(thread)),
            Err(_) if self.running(thread) => Status::Working,
            Err(error) => {
                log::warn!("{error:#}");
                Status::Failed
            }
        }
    }

    /// Sends the project's thread list to every client showing it.
    fn refresh(&mut self, slug: &str) {
        let watchers: Vec<(ClientId, PathBuf)> = self
            .clients
            .iter()
            .flat_map(|(id, client)| {
                client
                    .watching
                    .iter()
                    .filter(|(_, key)| key.as_str() == slug)
                    .map(|(cwd, _)| (*id, cwd.clone()))
            })
            .collect();
        if watchers.is_empty() {
            return;
        }
        match self.threads(slug) {
            Ok(threads) => {
                for (id, cwd) in watchers {
                    self.send(
                        id,
                        Notice::Threads {
                            cwd,
                            threads: threads.clone(),
                        },
                    );
                }
            }
            Err(error) => log::warn!("{error:#}"),
        }
    }

    pub fn handle(shared: &Shared, client: ClientId, request: Request) {
        match request {
            Request::Threads { cwd } => {
                let mut daemon = lock(shared);
                let slug = project_key(&cwd);
                match daemon.threads(&slug) {
                    Ok(threads) => {
                        if let Some(c) = daemon.clients.get_mut(&client) {
                            c.watching.insert(cwd.clone(), slug);
                        }
                        daemon.send(client, Notice::Threads { cwd, threads });
                    }
                    Err(error) => daemon.error(client, format!("{error:#}")),
                }
            }
            Request::Open { cwd, thread } => lock(shared).open(client, &cwd, &thread),
            Request::Close { thread } => {
                if let Some(c) = lock(shared).clients.get_mut(&client) {
                    c.open.remove(&thread);
                }
            }
            Request::Prompt {
                cwd,
                text,
                thread,
                worktree,
            } => {
                let running = thread.as_deref().is_some_and(|t| lock(shared).running(t));
                match thread {
                    // A run is already working on it: the text joins at its next turn.
                    Some(thread) if running => Self::message(shared, client, &cwd, &thread, &text),
                    None if worktree => Self::prompt_in_worktree(shared, client, cwd, &text),
                    thread => Self::prompt(shared, client, cwd.clone(), cwd, &text, thread),
                }
            }
            Request::Send { cwd, thread, text } => {
                Self::message(shared, client, &cwd, &thread, &text)
            }
            Request::Stop { thread } => lock(shared).stop(client, &thread),
            Request::Settings => lock(shared).settings(client),
            Request::SetDesktop { key, value } => lock(shared).set_desktop(client, &key, value),
            Request::Review { cwd, thread } => {
                lock(shared).review(client, &cwd, &thread, None);
            }
            Request::Decide {
                cwd,
                thread,
                hunk,
                decision,
            } => {
                let told = lock(shared).review(client, &cwd, &thread, Some((&hunk, &decision)));
                if let Some((work_tree, text)) = told {
                    Self::message(shared, client, &work_tree, &thread, &text);
                }
            }
        }
    }

    fn settings(&mut self, client: ClientId) {
        let path = settings_path(&self.home);
        match read_desktop(&path) {
            Ok(desktop) => self.send(client, Notice::Settings { path, desktop }),
            Err(error) => self.error(client, format!("{error:#}")),
        }
    }

    fn set_desktop(&mut self, client: ClientId, key: &str, value: serde_json::Value) {
        let path = settings_path(&self.home);
        match set_desktop_key(&path, key, value) {
            Ok(desktop) => {
                let clients: Vec<ClientId> = self.clients.keys().copied().collect();
                for id in clients {
                    let notice = Notice::Settings {
                        path: path.clone(),
                        desktop: desktop.clone(),
                    };
                    self.send(id, notice);
                }
            }
            Err(error) => self.error(client, format!("{error:#}")),
        }
    }

    /// Sends the thread's review, after applying a decision if there is one. A reject returns
    /// the worktree and the message for the thread.
    fn review(
        &mut self,
        client: ClientId,
        cwd: &Path,
        thread: &str,
        decision: Option<(&str, &cedian_wire::Decision)>,
    ) -> Option<(PathBuf, String)> {
        let slug = project_key(cwd);
        let result = (|| {
            let summary = read_thread(&threads_dir(&self.home, &slug), thread)?;
            let changes = Changes::of(&self.home, &slug, &summary)?;
            let mut decisions = self.decisions.load(thread);
            let mut review = changes.review(&self.home, &decisions)?;
            let mut told = None;
            if let Some((hunk, decision)) = decision {
                let text = decide(&review, &changes.work_tree, &mut decisions, hunk, decision)?;
                self.decisions.save(thread, &decisions)?;
                told = text.map(|text| (changes.work_tree.clone(), text));
                review = changes.review(&self.home, &decisions)?;
            }
            anyhow::Ok((review, told))
        })();
        match result {
            Ok((review, told)) => {
                self.send(
                    client,
                    Notice::Review {
                        thread: thread.to_owned(),
                        review,
                    },
                );
                told
            }
            Err(error) => {
                self.error(client, format!("{error:#}"));
                None
            }
        }
    }

    fn open(&mut self, client: ClientId, cwd: &Path, thread: &str) {
        let slug = project_key(cwd);
        let summary = match read_thread(&threads_dir(&self.home, &slug), thread) {
            Ok(summary) => summary,
            Err(error) => return self.error(client, format!("{error:#}")),
        };
        let run = self.by_thread.get(thread).and_then(|id| self.runs.get(id));
        let mut lines = Vec::new();
        for session in &summary.sessions {
            if run.is_some_and(|run| !run.before.contains(&session.session_id)) {
                continue;
            }
            let path = session_path(
                &self.home,
                &slug,
                &session.session_id,
                session.path.as_deref(),
            );
            match session_lines(&path) {
                Ok(more) => lines.extend(more),
                Err(error) => log::warn!("{error:#}"),
            }
        }
        if let Some(run) = run {
            lines.extend(run.lines.iter().cloned());
        }
        for line in lines {
            self.send(
                client,
                Notice::Line {
                    thread: thread.to_owned(),
                    line,
                },
            );
        }
        let status = Status::of(summary.state, self.running(thread));
        self.send(
            client,
            Notice::Caught {
                thread: thread.to_owned(),
            },
        );
        self.send(
            client,
            Notice::Status {
                thread: thread.to_owned(),
                status,
            },
        );
        if let Some(c) = self.clients.get_mut(&client) {
            c.open.insert(thread.to_owned());
        }
    }

    /// A new thread in a new worktree. Creating it runs git, so the daemon is not locked
    /// meanwhile.
    fn prompt_in_worktree(shared: &Shared, client: ClientId, asked_in: PathBuf, text: &str) {
        lock(shared).send(
            client,
            Notice::Preparing {
                cwd: asked_in.clone(),
                step: "creating a worktree".into(),
            },
        );
        match crate::worktree::create(&asked_in, text) {
            Ok((dir, branch)) => {
                log::info!("worktree {} on {branch}", dir.display());
                lock(shared).send(
                    client,
                    Notice::Preparing {
                        cwd: asked_in.clone(),
                        step: format!("starting on {branch}"),
                    },
                );
                Self::prompt(shared, client, asked_in, dir, text, None);
            }
            Err(error) => lock(shared).error(client, format!("{error:#}")),
        }
    }

    /// One run. `asked_in` is the window's project directory, which notices name; the run
    /// itself goes in `cwd`, or in a resumed thread's own directory.
    fn prompt(
        shared: &Shared,
        client: ClientId,
        asked_in: PathBuf,
        cwd: PathBuf,
        text: &str,
        thread: Option<String>,
    ) {
        let mut daemon = lock(shared);
        let slug = project_key(&cwd);
        let (before, cwd) = match &thread {
            Some(thread) => match read_thread(&threads_dir(&daemon.home, &slug), thread) {
                Ok(summary) => {
                    let own = daemon.thread_cwd(&slug, &summary).unwrap_or(cwd);
                    let before = summary.sessions.into_iter().map(|s| s.session_id).collect();
                    (before, own)
                }
                Err(error) => return daemon.error(client, format!("{error:#}")),
            },
            None => (HashSet::new(), cwd),
        };
        let ask = daemon
            .version
            .as_ref()
            .is_ok_and(|v| v.capabilities.contains(&cedian_protocol::Capability::Ask));
        let child = match daemon.cedia.prompt(&cwd, text, thread.as_deref(), ask) {
            Ok(child) => child,
            Err(error) => return daemon.error(client, format!("{error:#}")),
        };
        let id = daemon.id();
        log::info!("run {id}: pid {} in {}", child.id(), cwd.display());
        daemon.runs.insert(
            id,
            Run {
                asked_in,
                cwd,
                slug,
                thread: None,
                pid: child.id(),
                alive: true,
                lines: Vec::new(),
                before,
                requester: Some(client),
                stderr: Default::default(),
                followed: None,
            },
        );
        if let Some(thread) = thread {
            daemon.claim(id, thread);
        }
        daemon.keep_awake.set(true);
        drop(daemon);
        watch(shared.clone(), id, child);
    }

    /// Makes the run its thread's latest, dropping the thread's finished run before it.
    fn claim(&mut self, id: RunId, thread: String) {
        if let Some(old) = self.by_thread.insert(thread.clone(), id)
            && old != id
            && self.runs.get(&old).is_some_and(|run| !run.alive)
        {
            self.runs.remove(&old);
        }
        let Some(run) = self.runs.get_mut(&id) else {
            return;
        };
        run.thread = Some(thread.clone());
        let requester = run.requester;
        let cwd = run.asked_in.clone();
        let slug = run.slug.clone();
        self.save_ledger();
        if let Some(requester) = requester
            && let Some(client) = self.clients.get_mut(&requester)
            && client.open.insert(thread.clone())
        {
            self.send(
                requester,
                Notice::Started {
                    cwd,
                    thread: thread.clone(),
                },
            );
        }
        self.tell_readers(
            &thread,
            &Notice::Status {
                thread: thread.clone(),
                status: Status::Working,
            },
        );
        self.refresh(&slug);
    }

    fn message(shared: &Shared, client: ClientId, cwd: &Path, thread: &str, text: &str) {
        let cedia = lock(shared).cedia.clone();
        if let Err(error) = cedia.send(cwd, thread, text) {
            lock(shared).error(client, format!("{error:#}"));
        }
    }

    fn stop(&mut self, client: ClientId, thread: &str) {
        let Some(run) = self.by_thread.get(thread).and_then(|id| self.runs.get(id)) else {
            return self.error(client, format!("{thread} has no run"));
        };
        if !run.alive {
            return;
        }
        let Ok(pid) = libc::pid_t::try_from(run.pid) else {
            return;
        };
        // SAFETY: kill only sends a signal; the run is alive, so the pid is still its child.
        if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
            let error = std::io::Error::last_os_error();
            self.error(client, format!("stopping {thread}: {error}"));
        }
    }

    pub fn on_line(&mut self, id: RunId, line: String) {
        let Some(run) = self.runs.get_mut(&id) else {
            return;
        };
        run.lines.push(line.clone());
        let thread = match &run.thread {
            Some(thread) => thread.clone(),
            None => match Line::parse(&line) {
                Ok(Line::Event(SessionEvent::Session {
                    thread: Some(thread),
                    ..
                })) => {
                    self.claim(id, thread.clone());
                    thread
                }
                Ok(_) => return,
                Err(error) => {
                    log::warn!("run {id}: {error}");
                    return;
                }
            },
        };
        self.tell_readers(&thread.clone(), &Notice::Line { thread, line });
    }

    pub fn on_stderr(&mut self, id: RunId, line: String) {
        log::debug!("run {id}: {line}");
        if let Some(run) = self.runs.get_mut(&id) {
            run.push_stderr(line);
        }
    }

    pub fn on_exit(&mut self, id: RunId, how: String) {
        let Some(run) = self.runs.get_mut(&id) else {
            return;
        };
        run.alive = false;
        log::info!("run {id}: {how}");
        self.save_ledger();
        let working = self.runs.values().any(|run| run.alive);
        self.keep_awake.set(working);
        let Some(run) = self.runs.get(&id) else {
            return;
        };
        let (thread, slug, requester) = (run.thread.clone(), run.slug.clone(), run.requester);
        let stderr = run.stderr.iter().cloned().collect::<Vec<_>>().join("\n");
        match thread {
            Some(thread) => {
                let status = self.status(&slug, &thread);
                self.tell_readers(&thread.clone(), &Notice::Status { thread, status });
                self.refresh(&slug);
            }
            None => {
                self.runs.remove(&id);
                if let Some(requester) = requester {
                    self.error(
                        requester,
                        format!("cedia {how} before it started a thread:\n{stderr}"),
                    );
                }
            }
        }
    }
}
