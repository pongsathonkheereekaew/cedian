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
    let dir = tempfile::tempdir().expect("tempdir");
    let home = dir.path().join("home");
    let cwd = dir.path().join("repo");
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
}

impl Window {
    fn connect(harness: &Harness) -> Self {
        let stream = UnixStream::connect(&harness.socket).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .expect("timeout");
        let mut window = Self {
            writer: stream.try_clone().expect("clone"),
            reader: BufReader::new(stream),
        };
        match window.next() {
            Notice::Welcome { cedia, .. } => {
                assert_eq!(cedia.expect("fake version").version, "0.0.0-fake")
            }
            other => panic!("expected welcome, got {other:?}"),
        }
        window
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
    });
    window.until(|n| matches!(n, Notice::Line { .. }));

    // A prompt to a working thread is a message for its next turn.
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "also say why".into(),
        thread: Some("t-00000001".into()),
        worktree: false,
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
