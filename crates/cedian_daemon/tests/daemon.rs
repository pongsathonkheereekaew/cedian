//! Runs the daemon on a socket in a temporary directory against a fake cedia that plays
//! cedia's recorded fixtures, and talks to it as a window does.

use std::{
    collections::HashMap,
    io::BufReader,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::Duration,
};

use cedian_daemon::{Cedia, Config, serve};
use cedian_protocol::fixtures_dir;
use cedian_wire::{Decision, HunkState, Notice, Request, Status, read_message, write_message};

struct Harness {
    _dir: tempfile::TempDir,
    socket: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    threads: PathBuf,
    stream_lines: usize,
}

fn start(fixture: &str) -> Harness {
    start_with(fixture, "")
}

/// `caps`: capabilities the fake cedia adds to its version, as JSON strings with commas.
fn start_with(fixture: &str, caps: &str) -> Harness {
    let dir = tempfile::tempdir().expect("tempdir");
    let home = dir.path().join("home");
    // Canonical, as the window passes it: macOS's temp dir is under the /var -> /private/var
    // symlink, and a worktree's project key is taken from its real path.
    let cwd = dir.path().canonicalize().expect("tempdir").join("repo");
    std::fs::create_dir_all(&cwd).expect("repo");
    let slug = cedian_index::project_key(&cwd);
    let threads = cedian_index::threads_dir(&home, &slug);
    let fixture = fixtures_dir().join(fixture);
    let stream = std::fs::read_to_string(fixture.join("stream.jsonl")).expect("stream");
    let env = HashMap::from([
        ("PATH".to_owned(), std::env::var("PATH").unwrap_or_default()),
        ("CEDIA_HOME".to_owned(), home.display().to_string()),
        ("FAKE_FIXTURE".to_owned(), fixture.display().to_string()),
        ("FAKE_THREADS".to_owned(), threads.display().to_string()),
        ("FAKE_CAPS".to_owned(), caps.to_owned()),
        (
            "FAKE_SESSIONS".to_owned(),
            home.join("sessions").join(&slug).display().to_string(),
        ),
    ]);
    let socket = dir.path().join("state/daemon.sock");
    let config = Config {
        socket: socket.clone(),
        cedia: Cedia {
            bin: Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake/cedia"),
            env,
        },
        home: home.clone(),
    };
    std::thread::spawn(move || serve(config).expect("serve"));
    for _ in 0..100 {
        if UnixStream::connect(&socket).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Harness {
        _dir: dir,
        socket,
        home,
        cwd,
        threads,
        stream_lines: stream.lines().filter(|l| !l.trim().is_empty()).count(),
    }
}

struct Window {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    /// The models and modes the daemon sent after its welcome.
    choices: Notice,
}

impl Window {
    fn connect(harness: &Harness) -> Self {
        let stream = UnixStream::connect(&harness.socket).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .expect("timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut next = || read_message(&mut reader).expect("read").expect("a notice");
        match next() {
            Notice::Welcome { cedia, .. } => {
                assert_eq!(cedia.expect("fake version").version, "0.0.0-fake")
            }
            other => panic!("expected welcome, got {other:?}"),
        }
        let choices = next();
        Self {
            writer: stream,
            reader,
            choices,
        }
    }

    fn ask(&mut self, request: Request) {
        write_message(&mut self.writer, &request).expect("write");
    }

    fn next(&mut self) -> Notice {
        read_message(&mut self.reader)
            .expect("read")
            .expect("a notice")
    }

    /// Notices until one matches, with the lines on the way.
    fn until(&mut self, done: impl Fn(&Notice) -> bool) -> (Vec<String>, Vec<Notice>) {
        let mut lines = Vec::new();
        let mut seen = Vec::new();
        loop {
            let notice = self.next();
            if let Notice::Line { line, .. } = &notice {
                lines.push(line.clone());
            }
            let stop = done(&notice);
            seen.push(notice);
            if stop {
                return (lines, seen);
            }
        }
    }
}

fn status_is(want: Status) -> impl Fn(&Notice) -> bool {
    move |notice| matches!(notice, Notice::Status { status, .. } if *status == want)
}

#[test]
fn a_cedia_without_the_lists_sends_no_choices() {
    let harness = start("edit-and-check");
    let window = Window::connect(&harness);
    assert!(
        matches!(
            window.choices,
            Notice::Choices {
                models: None,
                permissions: None
            }
        ),
        "{:?}",
        window.choices
    );
}

#[test]
fn the_window_gets_cedias_models_and_modes() {
    let harness = start_with("edit-and-check", "\"models\",\"permissions\"");
    let window = Window::connect(&harness);
    let Notice::Choices {
        models: Some(models),
        permissions: Some(permissions),
    } = &window.choices
    else {
        panic!("{:?}", window.choices)
    };
    assert_eq!(models.default, "faux/a");
    assert_eq!(models.models.len(), 2);
    assert_eq!(models.models[1].reason.as_deref(), Some("no key"));
    assert_eq!(permissions.modes.len(), 2);
    assert_eq!(permissions.default.to_string(), "read-only");
}

#[test]
fn the_window_gets_the_projects_commands() {
    let harness = start_with("edit-and-check", "\"commands\"");
    let mut window = Window::connect(&harness);
    window.ask(Request::Commands {
        cwd: harness.cwd.clone(),
    });
    match window.next() {
        Notice::Commands { cwd, commands } => {
            assert_eq!(cwd, harness.cwd);
            assert_eq!(commands.len(), 1);
            assert_eq!(commands[0].name, "repo-review");
            assert_eq!(commands[0].argument_hint.as_deref(), Some("<pr>"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_window_gets_the_projects_memory_and_outputs() {
    let harness = start("edit-and-check");
    let mut window = Window::connect(&harness);
    let memory = harness.threads.with_file_name("memory.jsonl");
    std::fs::create_dir_all(&harness.threads).expect("project dir");
    std::fs::write(
        &memory,
        concat!(
            r#"{"kind":"note","id":"a1","text":"f.txt is the fixture","from":"user","at":"2026-10-22T09:00:00Z"}"#,
            "\n",
            r#"{"kind":"note","id":"b2","text":"stale","from":"s-1","at":"2026-10-22T09:01:00Z"}"#,
            "\n",
            r#"{"kind":"forget","id":"b2","at":"2026-10-22T09:02:00Z"}"#,
            "\n",
        ),
    )
    .expect("memory");
    window.ask(Request::Memory {
        cwd: harness.cwd.clone(),
    });
    match window.next() {
        Notice::Memory { cwd, notes } => {
            assert_eq!(cwd, harness.cwd);
            let ids: Vec<&str> = notes.iter().map(|n| n.id.as_str()).collect();
            assert_eq!(ids, ["a1"]);
        }
        other => panic!("{other:?}"),
    }
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "make f.txt say good".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    window.until(status_is(Status::Done));
    window.ask(Request::Outputs {
        cwd: harness.cwd.clone(),
    });
    let (_, seen) = window.until(|n| matches!(n, Notice::Outputs { .. }));
    let Some(Notice::Outputs { outputs, .. }) = seen.last() else {
        panic!("{seen:?}")
    };
    // The fixture ran in /fixture/repo, which is not here.
    let files: Vec<(&str, &str, bool)> = outputs
        .iter()
        .map(|o| (o.path.as_str(), o.thread.as_str(), o.exists))
        .collect();
    assert_eq!(files, [("f.txt", "t-00000001", false)]);
}

#[test]
fn the_window_gets_the_usage_of_one_project_or_all() {
    let harness = start_with("edit-and-check", "\"usage\"");
    let mut window = Window::connect(&harness);
    window.ask(Request::Usage {
        cwd: Some(harness.cwd.clone()),
        since: "7d".into(),
    });
    match window.next() {
        Notice::Usage { cwd, since, report } => {
            assert_eq!(cwd.as_ref(), Some(&harness.cwd));
            assert_eq!(since, "7d");
            assert_eq!(report.threads[0].title, "7d repo");
            assert_eq!(report.days.len(), 1);
        }
        other => panic!("{other:?}"),
    }
    window.ask(Request::Usage {
        cwd: None,
        since: "30d".into(),
    });
    match window.next() {
        Notice::Usage {
            cwd: None, report, ..
        } => assert_eq!(report.threads[0].title, "30d all"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_cedia_without_usage_says_so() {
    let harness = start("edit-and-check");
    let mut window = Window::connect(&harness);
    window.ask(Request::Usage {
        cwd: None,
        since: "7d".into(),
    });
    assert!(matches!(window.next(), Notice::Error { .. }));
}

#[test]
fn an_edited_message_forks_the_thread_and_runs_there() {
    let harness = start_with("edit-and-check", "\"threads.fork\"");
    let mut window = Window::connect(&harness);
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "make f.txt say good".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    window.until(status_is(Status::Done));
    window.ask(Request::Fork {
        cwd: harness.cwd.clone(),
        thread: "t-00000001".into(),
        run: "r-1".into(),
        text: "make f.txt say better".into(),
        model: Some("sonnet".into()),
        permission: None,
        effort: Some("high".into()),
    });
    let (_, seen) = window.until(|n| matches!(n, Notice::Forked { .. }));
    assert!(
        matches!(seen.last(), Some(Notice::Forked { from, thread, .. })
            if from == "t-00000001" && thread == "t-0000fork"),
        "{seen:?}"
    );
    // The fixture's run ends a thread that is done already, so wait on what it was given.
    let args_file = harness.threads.join("../args");
    for _ in 0..100 {
        if std::fs::read_to_string(&args_file).is_ok_and(|a| a.contains("better")) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let args = std::fs::read_to_string(&args_file).expect("args");
    let args: Vec<&str> = args.lines().collect();
    assert!(args.contains(&"--print=make f.txt say better"), "{args:?}");
    assert!(
        args.windows(2).any(|w| w == ["--thread", "t-0000fork"]),
        "{args:?}"
    );
    assert!(
        args.windows(2).any(|w| w == ["--model", "sonnet"]),
        "{args:?}"
    );
    assert!(
        args.windows(2).any(|w| w == ["--effort", "high"]),
        "{args:?}"
    );
}

#[test]
fn a_cedia_that_cannot_fork_says_so() {
    let harness = start("edit-and-check");
    let mut window = Window::connect(&harness);
    window.ask(Request::Fork {
        cwd: harness.cwd.clone(),
        thread: "t-00000001".into(),
        run: "r-1".into(),
        text: "again".into(),
        model: None,
        permission: None,
        effort: None,
    });
    assert!(matches!(window.next(), Notice::Error { .. }));
}

#[test]
fn a_cedia_without_commands_is_not_asked() {
    let harness = start("edit-and-check");
    let mut window = Window::connect(&harness);
    window.ask(Request::Commands {
        cwd: harness.cwd.clone(),
    });
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    assert!(matches!(window.next(), Notice::Threads { .. }));
}

#[test]
fn a_prompt_runs_and_a_second_window_rebuilds_it() {
    let harness = start("edit-and-check");
    let mut window = Window::connect(&harness);
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    assert!(matches!(window.next(), Notice::Threads { threads, .. } if threads.is_empty()));

    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "make f.txt say good".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    let (lines, seen) = window.until(status_is(Status::Done));
    assert!(
        matches!(&seen[0], Notice::Started { thread, .. } if thread == "t-00000001"),
        "{seen:?}"
    );
    assert_eq!(lines.len(), harness.stream_lines);
    assert!(
        seen.iter()
            .any(|n| matches!(n, Notice::Threads { threads, .. }
        if threads.len() == 1 && threads[0].status == Status::Working))
    );

    // The run's lines are kept; a window that opens the thread now gets all of them.
    let mut other = Window::connect(&harness);
    other.ask(Request::Open {
        cwd: harness.cwd.clone(),
        thread: "t-00000001".into(),
    });
    let (lines, seen) = other.until(|n| matches!(n, Notice::Caught { .. }));
    assert_eq!(lines.len(), harness.stream_lines, "{seen:?}");
    assert!(matches!(
        other.next(),
        Notice::Status {
            status: Status::Done,
            ..
        }
    ));
}

#[test]
fn stop_ends_a_run_and_send_reaches_the_thread() {
    let harness = start("edit-and-check");
    std::fs::create_dir_all(&harness.threads).expect("threads dir");
    std::fs::write(harness.threads.join("../hold"), "").expect("hold");
    let mut window = Window::connect(&harness);
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "make f.txt say good".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    window.until(|n| matches!(n, Notice::Line { .. }));

    // A prompt to a working thread is a message for its next turn.
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "also say why".into(),
        thread: Some("t-00000001".into()),
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    window.ask(Request::Stop {
        thread: "t-00000001".into(),
    });
    window.until(status_is(Status::Stopped));
    let thread = std::fs::read_to_string(harness.threads.join("t-00000001.jsonl")).expect("thread");
    assert!(thread.contains("also say why"), "{thread}");
}

#[test]
fn a_rating_reaches_the_thread_and_the_list() {
    let harness = start("edit-and-check");
    let mut window = Window::connect(&harness);
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "make f.txt say good".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    window.until(status_is(Status::Done));
    window.ask(Request::Rate {
        cwd: harness.cwd.clone(),
        thread: "t-00000001".into(),
        rating: cedian_protocol::Rating::Up,
    });
    // The thread watch sees the file change and sends the list again.
    window.until(|n| {
        matches!(n, Notice::Threads { threads, .. }
            if threads.first().is_some_and(|t| t.thread.rating == Some(cedian_protocol::Rating::Up)))
    });
}

#[test]
fn a_queued_message_can_be_taken_back() {
    let harness = start("edit-and-check");
    std::fs::create_dir_all(&harness.threads).expect("threads dir");
    std::fs::write(harness.threads.join("../hold"), "").expect("hold");
    let mut window = Window::connect(&harness);
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "make f.txt say good".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    window.until(|n| matches!(n, Notice::Line { .. }));
    window.ask(Request::Send {
        cwd: harness.cwd.clone(),
        thread: "t-00000001".into(),
        text: "also say why".into(),
    });
    let queued = |want: bool| {
        move |n: &Notice| {
            matches!(n, Notice::Threads { threads, .. }
                if threads.first().is_some_and(|t| t.thread.pending.is_empty() != want))
        }
    };
    window.until(queued(true));
    window.ask(Request::Unsend {
        cwd: harness.cwd.clone(),
        thread: "t-00000001".into(),
        ids: vec!["m-1".into()],
    });
    window.until(queued(false));
    window.ask(Request::Stop {
        thread: "t-00000001".into(),
    });
    window.until(status_is(Status::Stopped));
}

#[test]
fn a_thread_archives_and_comes_back() {
    let harness = start("edit-and-check");
    std::fs::create_dir_all(&harness.threads).expect("threads dir");
    std::fs::write(
        harness.threads.join("t-0000000c.jsonl"),
        "{\"at\":\"2026-10-22T03:00:00.000Z\",\"kind\":\"open\",\"title\":\"x\",\"task\":\"x\"}\n\
         {\"at\":\"2026-10-22T03:00:01.000Z\",\"kind\":\"state\",\"state\":\"done\"}\n",
    )
    .expect("thread");
    let mut window = Window::connect(&harness);
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    let archived = |want: bool| {
        move |n: &Notice| {
            matches!(n, Notice::Threads { threads, .. }
                if threads.first().is_some_and(|t| t.thread.archived == want && !t.needs_you))
        }
    };
    window.until(archived(false));
    // Seen once, so a change it did not make itself would make it need the person.
    window.ask(Request::Open {
        cwd: harness.cwd.clone(),
        thread: "t-0000000c".into(),
    });
    window.ask(Request::Close {
        thread: "t-0000000c".into(),
    });
    for want in [true, false] {
        window.ask(Request::Archive {
            cwd: harness.cwd.clone(),
            thread: "t-0000000c".into(),
            archived: want,
        });
        window.until(archived(want));
    }
}

#[test]
fn a_thread_left_working_reads_as_interrupted() {
    let harness = start("edit-and-check");
    std::fs::create_dir_all(&harness.threads).expect("threads dir");
    std::fs::write(
        harness.threads.join("t-0000000a.jsonl"),
        "{\"at\":\"2026-10-22T03:00:00.000Z\",\"kind\":\"open\",\"title\":\"x\",\"task\":\"x\"}\n",
    )
    .expect("thread");
    let mut window = Window::connect(&harness);
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    match window.next() {
        Notice::Threads { threads, .. } => assert_eq!(threads[0].status, Status::Interrupted),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_thread_made_outside_the_daemon_reaches_the_list() {
    let harness = start("edit-and-check");
    let mut window = Window::connect(&harness);
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    assert!(matches!(window.next(), Notice::Threads { threads, .. } if threads.is_empty()));

    // As `cedia -p` in a terminal would.
    std::fs::create_dir_all(&harness.threads).expect("threads dir");
    let path = harness.threads.join("t-0000000b.jsonl");
    std::fs::write(
        &path,
        "{\"at\":\"2026-10-22T03:00:00.000Z\",\"kind\":\"open\",\"title\":\"from a terminal\",\"task\":\"x\"}\n",
    )
    .expect("thread");
    let (_, seen) =
        window.until(|n| matches!(n, Notice::Threads { threads, .. } if !threads.is_empty()));
    match seen.last() {
        Some(Notice::Threads { threads, .. }) => {
            assert_eq!(threads[0].thread.title, "from a terminal")
        }
        other => panic!("{other:?}"),
    }

    // And it is archived from the terminal too.
    let mut text = std::fs::read_to_string(&path).expect("read");
    text.push_str(
        "{\"at\":\"2026-10-22T03:01:00.000Z\",\"kind\":\"archived\",\"archived\":true}\n",
    );
    std::fs::write(&path, text).expect("append");
    window.until(|n| matches!(n, Notice::Threads { threads, .. } if threads[0].thread.archived));
}

#[test]
fn a_failed_thread_needs_you_until_a_window_opens_it() {
    let harness = start("edit-and-check");
    std::fs::create_dir_all(&harness.threads).expect("threads dir");
    std::fs::write(
        harness.threads.join("t-0000000c.jsonl"),
        "{\"at\":\"2026-10-22T03:00:00.000Z\",\"kind\":\"open\",\"title\":\"x\",\"task\":\"x\"}\n\
         {\"at\":\"2026-10-22T03:01:00.000Z\",\"kind\":\"state\",\"state\":\"failed\"}\n",
    )
    .expect("thread");
    let mut window = Window::connect(&harness);
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    match window.next() {
        Notice::Threads { threads, .. } => assert!(threads[0].needs_you),
        other => panic!("{other:?}"),
    }
    window.ask(Request::Open {
        cwd: harness.cwd.clone(),
        thread: "t-0000000c".into(),
    });
    window.until(|n| matches!(n, Notice::Threads { threads, .. } if !threads[0].needs_you));

    // Seen stays seen for the next window.
    let mut other = Window::connect(&harness);
    other.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    match other.next() {
        Notice::Threads { threads, .. } => assert!(!threads[0].needs_you),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_socket_path_too_long_fails_with_the_reason() {
    let config = Config {
        socket: PathBuf::from(format!("/tmp/{}/daemon.sock", "x".repeat(120))),
        cedia: Cedia {
            bin: PathBuf::from("cedia"),
            env: HashMap::new(),
        },
        home: PathBuf::from("/nonexistent"),
    };
    let error = serve(config).expect_err("too long");
    assert!(
        format!("{error:#}").contains("CEDIAN_STATE_DIR"),
        "{error:#}"
    );
}

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .status()
        .expect("git");
    assert!(status.success(), "git {args:?}");
}

#[test]
fn a_new_thread_can_run_in_its_own_worktree() {
    let harness = start("edit-and-check");
    git(&harness.cwd, &["init", "-q"]);
    git(
        &harness.cwd,
        &["commit", "-q", "--allow-empty", "-m", "start"],
    );
    let mut window = Window::connect(&harness);
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "Make f.txt say good".into(),
        thread: None,
        worktree: true,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    let (_, seen) = window.until(status_is(Status::Done));
    assert!(
        seen.iter().any(|n| matches!(n, Notice::Preparing { step, .. } if step == "starting on cedian/make-f-txt-say-good")),
        "{seen:?}"
    );
    let ran_in = std::fs::read_to_string(harness.threads.join("../cwd")).expect("cwd");
    let worktree = harness
        .cwd
        .canonicalize()
        .expect("repo")
        .join(".cedian/worktrees/make-f-txt-say-good");
    assert_eq!(Path::new(ran_in.trim()), worktree);
    let exclude = std::fs::read_to_string(harness.cwd.join(".git/info/exclude")).expect("exclude");
    assert!(exclude.contains("/.cedian/"));
    git(
        &harness.cwd,
        &["rev-parse", "-q", "--verify", "cedian/make-f-txt-say-good"],
    );
}

/// A snapshot the way cedia takes one, into the shadow git cedia keeps for `work`.
fn snapshot(home: &Path, work: &Path) -> String {
    let shadow = home
        .join("snapshots")
        .join(cedian_index::project_slug(work));
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .env("GIT_DIR", &shadow)
            .env("GIT_WORK_TREE", work)
            .env("GIT_INDEX_FILE", shadow.join("cedia-index"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .current_dir(work)
            .output()
            .expect("git");
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).expect("utf8")
    };
    if !shadow.join("HEAD").exists() {
        std::fs::create_dir_all(&shadow).expect("shadow");
        git(&["init", "-q"]);
    }
    git(&["add", "-A", "."]);
    git(&["write-tree"]).trim().to_owned()
}

#[test]
fn rejecting_a_hunk_puts_the_file_back_and_tells_the_thread() {
    let harness = start("edit-and-check");
    let home = harness.home.clone();
    let mut window = Window::connect(&harness);
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "make f.txt say good".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    let (_, seen) = window.until(status_is(Status::Done));
    let thread = seen
        .iter()
        .find_map(|n| match n {
            Notice::Started { thread, .. } => Some(thread.clone()),
            _ => None,
        })
        .expect("started");

    // The fixture ran in /fixture/repo; move its log to this repo and rebuild its snapshots,
    // which are content-addressed, so the trees it logged are the trees git writes here.
    let sessions = home
        .join("sessions")
        .join(cedian_index::project_key(&harness.cwd));
    for entry in std::fs::read_dir(&sessions).expect("sessions") {
        let path = entry.expect("entry").path();
        let text = std::fs::read_to_string(&path).expect("log");
        let here = harness.cwd.display().to_string();
        std::fs::write(&path, text.replace("/fixture/repo", &here)).expect("log");
    }
    std::fs::write(harness.cwd.join("f.txt"), "bad\n").expect("write");
    assert_eq!(
        snapshot(&home, &harness.cwd),
        "5f443e10ae5374c7c54f6caa2d73aa39e2872d6f"
    );
    std::fs::write(harness.cwd.join("f.txt"), "good\n").expect("write");
    assert_eq!(
        snapshot(&home, &harness.cwd),
        "99f8c32e405deb9aaa1ecb9d26171f9c8936cfcf"
    );

    let review_of =
        |notice: &Notice| matches!(notice, Notice::Review { .. } | Notice::Error { .. });
    window.ask(Request::Review {
        cwd: harness.cwd.clone(),
        thread: thread.clone(),
    });
    let (_, seen) = window.until(review_of);
    let Some(Notice::Review { review, .. }) = seen.last() else {
        panic!("{seen:?}");
    };
    assert_eq!(review.files.len(), 1, "{review:?}");
    let hunk = review.files[0].hunks[0].clone();
    assert_eq!(
        (review.files[0].path.as_str(), hunk.state),
        ("f.txt", HunkState::Pending)
    );
    assert_eq!(hunk.lines, ["+good"]);

    window.ask(Request::Decide {
        cwd: harness.cwd.clone(),
        thread: thread.clone(),
        hunk: hunk.id.clone(),
        decision: Decision::Reject {
            reason: "keep the file out".into(),
        },
    });
    let (_, seen) = window.until(review_of);
    let Some(Notice::Review { review, .. }) = seen.last() else {
        panic!("{seen:?}");
    };
    assert_eq!(review.files[0].hunks[0].state, HunkState::Rejected);
    assert!(!harness.cwd.join("f.txt").exists());
    let sent = harness.threads.join("../sent");
    for _ in 0..100 {
        if sent.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let text = std::fs::read_to_string(sent).expect("sent");
    assert!(text.contains("rejected your change to f.txt"), "{text}");
    assert!(
        text.contains("Why: keep the file out") && text.contains("+good"),
        "{text}"
    );

    // Rejected once is final.
    window.ask(Request::Decide {
        cwd: harness.cwd.clone(),
        thread,
        hunk: hunk.id,
        decision: Decision::Accept,
    });
    let (_, seen) = window.until(review_of);
    assert!(
        matches!(seen.last(), Some(Notice::Error { .. })),
        "{seen:?}"
    );
}

#[test]
fn a_verified_thread_with_nothing_to_review_archives_itself() {
    // The denied-call run changes no file; given a verified receipt, nothing is owed on it.
    let fixture = tempfile::tempdir().expect("tempdir");
    let from = fixtures_dir().join("denied-call");
    for sub in ["threads", "sessions"] {
        std::fs::create_dir_all(fixture.path().join(sub)).expect("mkdir");
        for entry in std::fs::read_dir(from.join(sub)).expect("read dir") {
            let path = entry.expect("entry").path();
            let mut text = std::fs::read_to_string(&path).expect("read");
            if sub == "sessions" {
                text = text.replace(
                    "\"subtype\":\"success\",",
                    "\"subtype\":\"success\",\"evidence\":{\"changed\":[],\"checks\":[],\"flags\":[],\"unverified\":[]},",
                );
            }
            let name = path.file_name().expect("name");
            std::fs::write(fixture.path().join(sub).join(name), text).expect("write");
        }
    }
    std::fs::copy(
        from.join("stream.jsonl"),
        fixture.path().join("stream.jsonl"),
    )
    .expect("copy");
    let harness = start(fixture.path().to_str().expect("utf-8"));
    let mut window = Window::connect(&harness);
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "publish it".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    window.until(|n| {
        matches!(n, Notice::Threads { threads, .. }
            if threads.first().is_some_and(|t| t.thread.archived && !t.needs_you))
    });
}

#[test]
fn an_unverified_thread_stays() {
    // As recorded, denied-call's receipt has no evidence, so it is not verified.
    let harness = start("denied-call");
    let mut window = Window::connect(&harness);
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "publish it".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    window.until(status_is(Status::Done));
    // A list after the run ended, which an archive would have come before.
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    let (_, seen) =
        window.until(|n| matches!(n, Notice::Threads { threads, .. } if !threads.is_empty()));
    match seen.last() {
        Some(Notice::Threads { threads, .. }) => assert!(!threads[0].thread.archived),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_thread_the_coordinator_opened_starts_in_its_own_worktree() {
    let harness = start_with("edit-and-check", "\"todo\",\"open_thread\"");
    git(&harness.cwd, &["init", "-q"]);
    git(
        &harness.cwd,
        &["commit", "-q", "--allow-empty", "-m", "start"],
    );
    std::fs::create_dir_all(&harness.threads).expect("threads dir");
    std::fs::write(
        harness.threads.join("t-0000000q.jsonl"),
        "{\"at\":\"2026-10-22T03:00:00.000Z\",\"kind\":\"open\",\"title\":\"Fix the login\",\"task\":\"Fix the login form\",\"by\":\"t-0000000c\"}\n",
    )
    .expect("thread");
    let mut window = Window::connect(&harness);
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    let (_, seen) =
        window.until(|n| matches!(n, Notice::Threads { threads, .. } if !threads.is_empty()));
    match seen.last() {
        Some(Notice::Threads { threads, .. }) => assert_eq!(threads[0].status, Status::Queued),
        other => panic!("{other:?}"),
    }
    // The daemon starts it once it polls the project: `cedia -p <task> --thread <id>`.
    let args_file = harness.threads.join("../args");
    for _ in 0..100 {
        if std::fs::read_to_string(&args_file).is_ok_and(|a| a.contains("--thread")) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let args = std::fs::read_to_string(&args_file).expect("args");
    let args: Vec<&str> = args.lines().collect();
    assert!(args.contains(&"--print=Fix the login form"), "{args:?}");
    assert!(
        args.windows(2).any(|w| w == ["--thread", "t-0000000q"]),
        "{args:?}"
    );
    assert!(args.windows(2).any(|w| w == ["--todo", "on"]), "{args:?}");
    let ran_in = std::fs::read_to_string(harness.threads.join("../cwd")).expect("cwd");
    let worktree = harness
        .cwd
        .canonicalize()
        .expect("repo")
        .join(".cedian/worktrees/fix-the-login-form");
    assert_eq!(Path::new(ran_in.trim()), worktree);
}

#[test]
fn a_prompt_that_names_a_model_and_a_mode_runs_on_them() {
    let harness = start("edit-and-check");
    let mut window = Window::connect(&harness);
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "make f.txt say good".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: Some("sonnet".into()),
        permission: Some("plan".into()),
        effort: None,
    });
    window.until(status_is(Status::Done));
    let args = std::fs::read_to_string(harness.threads.join("../args")).expect("args");
    let args: Vec<&str> = args.lines().collect();
    assert!(
        args.windows(2).any(|w| w == ["--model", "sonnet"]),
        "{args:?}"
    );
    assert!(
        args.windows(2).any(|w| w == ["--permission", "plan"]),
        "{args:?}"
    );
}

#[test]
fn the_window_gets_the_schedules_and_a_due_one_wakes_its_thread() {
    let harness = start_with("edit-and-check", "\"schedules\"");
    let mut window = Window::connect(&harness);
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "make f.txt say good".into(),
        thread: None,
        worktree: false,
        profile: None,
        model: None,
        permission: None,
        effort: None,
    });
    window.until(status_is(Status::Done));
    let state = harness.threads.join("..");
    let schedule = r#"[{"id":"s-1","prompt":"summarise the night","thread":"t-00000001","every":1440,"enabled":true,"next":"2020-01-01T00:00:00.000Z","createdAt":"2019-12-31T00:00:00.000Z"}]"#;
    std::fs::write(state.join("schedules.json"), schedule).expect("list");
    window.ask(Request::Background {
        cwd: harness.cwd.clone(),
    });
    let (_, seen) = window.until(|n| matches!(n, Notice::Background { .. }));
    match seen.last() {
        Some(Notice::Background { cwd, schedules, .. }) => {
            assert_eq!(cwd, &harness.cwd);
            let schedules = schedules.as_ref().expect("this cedia has schedules");
            assert_eq!(schedules.len(), 1);
            assert_eq!(schedules[0].prompt, "summarise the night");
        }
        other => panic!("{other:?}"),
    }

    // Due once: the daemon fires it and runs its prompt on its thread.
    std::fs::write(state.join("due.json"), schedule).expect("due");
    window.ask(Request::Threads {
        cwd: harness.cwd.clone(),
    });
    let args_file = state.join("args");
    for _ in 0..100 {
        if std::fs::read_to_string(&args_file).is_ok_and(|a| a.contains("night")) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let args = std::fs::read_to_string(&args_file).expect("args");
    let args: Vec<&str> = args.lines().collect();
    assert!(args.contains(&"--print=summarise the night"), "{args:?}");
    assert!(
        args.windows(2).any(|w| w == ["--thread", "t-00000001"]),
        "{args:?}"
    );
    let calls = std::fs::read_to_string(state.join("schedule-calls")).expect("calls");
    assert!(
        calls.lines().any(|c| c.starts_with("schedules fire s-1")),
        "{calls}"
    );

    window.ask(Request::Schedule {
        cwd: harness.cwd.clone(),
        id: "s-1".into(),
        enabled: false,
    });
    // The pause is answered with the list again; one may already be on its way from the fire.
    let paused = || {
        std::fs::read_to_string(state.join("schedule-calls"))
            .is_ok_and(|calls| calls.lines().any(|c| c.starts_with("schedules pause s-1")))
    };
    window.until(|n| matches!(n, Notice::Background { .. }) && paused());
    assert!(paused());
}

#[test]
fn a_cedia_without_schedules_lists_none() {
    let harness = start("edit-and-check");
    let mut window = Window::connect(&harness);
    window.ask(Request::Background {
        cwd: harness.cwd.clone(),
    });
    assert!(matches!(
        window.next(),
        Notice::Background {
            schedules: None,
            ..
        }
    ));
}
