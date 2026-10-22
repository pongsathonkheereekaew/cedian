//! A daemon started by a process whose threads block SIGTERM, as the window's do on the Mac,
//! still stops on SIGTERM: a blocked signal is inherited through exec.

use std::{
    os::unix::{net::UnixStream, process::CommandExt as _, process::ExitStatusExt as _},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn sigterm_stops_a_daemon_whose_parent_blocked_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut command = Command::new(env!("CARGO_BIN_EXE_cedian-daemon"));
    command
        .env("CEDIAN_NO_LOGIN_ENV", "1")
        .env("CEDIAN_STATE_DIR", dir.path().join("state"))
        .env("CEDIA_HOME", dir.path().join("home"))
        .env("CEDIAN_CEDIA_BIN", "/nonexistent/cedia")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: only async-signal-safe calls between fork and exec.
    unsafe {
        command.pre_exec(|| {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut set);
            libc::sigaddset(&mut set, libc::SIGTERM);
            libc::sigaddset(&mut set, libc::SIGINT);
            libc::sigaddset(&mut set, libc::SIGHUP);
            libc::sigprocmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
            Ok(())
        });
    }
    let mut daemon = command.spawn().expect("daemon");
    let socket = dir.path().join("state/daemon.sock");
    let start = Instant::now();
    while UnixStream::connect(&socket).is_err() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "daemon never bound"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid = libc::pid_t::try_from(daemon.id()).expect("pid");
    // SAFETY: kill only sends a signal to our own child.
    unsafe { libc::kill(pid, libc::SIGTERM) };
    let start = Instant::now();
    let status = loop {
        if let Some(status) = daemon.try_wait().expect("wait") {
            break status;
        }
        if start.elapsed() > Duration::from_secs(5) {
            daemon.kill().ok();
            daemon.wait().ok();
            panic!("the daemon did not stop on SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.signal(), Some(libc::SIGTERM));
}
