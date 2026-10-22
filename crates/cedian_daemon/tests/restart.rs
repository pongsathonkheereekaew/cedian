//! A run outlives its daemon: kill the daemon binary mid-run, start another, and the new one
//! follows the run from cedia's files and can still stop it.

use std::{
    io::BufReader,
    os::unix::net::UnixStream,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use cedian_protocol::fixtures_dir;
use cedian_wire::{Notice, Request, Status, read_message, write_message};

struct Env {
    dir: tempfile::TempDir,
    cwd: std::path::PathBuf,
    threads: std::path::PathBuf,
}

impl Env {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let cwd = dir.path().join("repo");
        std::fs::create_dir_all(&cwd).expect("repo");
        let slug = cedian_index::project_key(&cwd);
        let threads = cedian_index::threads_dir(&dir.path().join("home"), &slug);
        std::fs::create_dir_all(&threads).expect("threads");
        // The fake cedia waits after its first line until SIGTERM.
        std::fs::write(threads.join("../hold"), "").expect("hold");
        Self { dir, cwd, threads }
    }

    fn daemon(&self) -> Child {
        let home = self.dir.path().join("home");
        let slug = cedian_index::project_key(&self.cwd);
        let child = Command::new(env!("CARGO_BIN_EXE_cedian-daemon"))
            .env("CEDIAN_NO_LOGIN_ENV", "1")
            .env("CEDIAN_STATE_DIR", self.dir.path().join("state"))
            .env(
                "CEDIAN_CEDIA_BIN",
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake/cedia"),
            )
            .env("CEDIA_HOME", &home)
            .env("FAKE_FIXTURE", fixtures_dir().join("edit-and-check"))
            .env("FAKE_THREADS", &self.threads)
            .env("FAKE_SESSIONS", home.join("sessions").join(slug))
            .stdin(Stdio::null())
            .stderr(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(self.dir.path().join("daemon.log"))
                    .expect("log"),
            )
            .spawn()
            .expect("daemon");
        let socket = self.dir.path().join("state/daemon.sock");
        let start = Instant::now();
        while UnixStream::connect(&socket).is_err() {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "daemon never bound"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        child
    }

    fn window(&self) -> (UnixStream, BufReader<UnixStream>) {
        let stream =
            UnixStream::connect(self.dir.path().join("state/daemon.sock")).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .expect("timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        assert!(matches!(next(&mut reader), Notice::Welcome { .. }));
        assert!(matches!(next(&mut reader), Notice::Choices { .. }));
        (stream, reader)
    }
}

fn next(reader: &mut BufReader<UnixStream>) -> Notice {
    read_message(reader).expect("read").expect("a notice")
}

fn until(reader: &mut BufReader<UnixStream>, done: impl Fn(&Notice) -> bool) -> Vec<Notice> {
    let mut seen = Vec::new();
    loop {
        let notice = next(reader);
        let stop = done(&notice);
        seen.push(notice);
        if stop {
            return seen;
        }
    }
}

#[test]
fn a_new_daemon_follows_a_run_the_old_one_started() {
    let env = Env::new();
    let mut first = Daemon(env.daemon());
    let (mut writer, mut reader) = env.window();
    write_message(
        &mut writer,
        &Request::Prompt {
            cwd: env.cwd.clone(),
            text: "make f.txt say good".into(),
            thread: None,
            worktree: false,
            profile: None,
            model: None,
            permission: None,
        },
    )
    .expect("prompt");
    until(&mut reader, |n| matches!(n, Notice::Line { .. }));
    first.0.kill().expect("kill the first daemon");
    first.0.wait().expect("reap");
    drop((writer, reader));

    let _second = Daemon(env.daemon());
    let (mut writer, mut reader) = env.window();
    write_message(
        &mut writer,
        &Request::Open {
            cwd: env.cwd.clone(),
            thread: "t-00000001".into(),
        },
    )
    .expect("open");
    // The followed run's lines come from its session log, read before or after the open.
    let mut seen = until(&mut reader, |n| matches!(n, Notice::Status { .. }));
    assert!(
        matches!(
            seen.last(),
            Some(Notice::Status {
                status: Status::Working,
                ..
            })
        ),
        "{seen:?}"
    );
    if !seen.iter().any(|n| matches!(n, Notice::Line { .. })) {
        seen = until(&mut reader, |n| matches!(n, Notice::Line { .. }));
    }
    assert!(seen.iter().any(|n| matches!(n, Notice::Line { .. })));

    write_message(
        &mut writer,
        &Request::Stop {
            thread: "t-00000001".into(),
        },
    )
    .expect("stop");
    until(&mut reader, |n| {
        matches!(
            n,
            Notice::Status {
                status: Status::Stopped,
                ..
            }
        )
    });
}

/// Kills the daemon when the test ends, passed or not.
struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}
