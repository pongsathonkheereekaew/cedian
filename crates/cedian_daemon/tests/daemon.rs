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
use cedian_wire::{Notice, Request, Status, read_message, write_message};

struct Harness {
    _dir: tempfile::TempDir,
    socket: PathBuf,
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
        home,
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
    });
    window.until(|n| matches!(n, Notice::Line { .. }));

    // A prompt to a working thread is a message for its next turn.
    window.ask(Request::Prompt {
        cwd: harness.cwd.clone(),
        text: "also say why".into(),
        thread: Some("t-00000001".into()),
    });
    window.ask(Request::Stop {
        thread: "t-00000001".into(),
    });
    window.until(status_is(Status::Stopped));
    let thread = std::fs::read_to_string(harness.threads.join("t-00000001.jsonl")).expect("thread");
    assert!(thread.contains("also say why"), "{thread}");
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
