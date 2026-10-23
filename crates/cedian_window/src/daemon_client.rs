//! The window's end of the daemon's socket. The window starts the daemon when nobody answers,
//! then only talks to it: quitting the window leaves the daemon and its runs going. A daemon
//! from another build than the one beside the window (an older app, a build replaced since)
//! is handed over: stopped, and the window's own started, which follows the runs the old one
//! left going.

use std::{
    io::BufReader,
    os::{
        fd::AsRawFd as _,
        unix::{net::UnixStream, process::CommandExt as _},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

use anyhow::Context as _;
use cedian_wire::{
    Build, Notice, Request, launch_agent_plist, read_message, socket_path, state_dir, write_message,
};
use futures::channel::mpsc::{UnboundedReceiver, unbounded};

/// How long a daemon this window started gets to read the login environment and bind.
const START_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a daemon being handed over gets to exit after SIGTERM.
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

pub struct DaemonClient {
    writer: Mutex<UnixStream>,
}

impl DaemonClient {
    /// Connects, starting the daemon first if needed. Blocks; call it off the main thread.
    pub fn connect() -> anyhow::Result<(Self, UnboundedReceiver<Notice>)> {
        let socket = socket_path();
        let (mut stream, mut reader, mut welcome) = open(&socket)?;
        let own = own_daemon().as_deref().and_then(Build::of);
        if let Some(own) = &own
            && is_other_build(&welcome, own)
        {
            log::info!(
                "the daemon on {} is another build; handing over to {}",
                socket.display(),
                own.exe.display()
            );
            match hand_over(&stream, &own.exe, &socket) {
                Ok(()) => {
                    (stream, reader, welcome) = open(&socket)?;
                    if is_other_build(&welcome, own) {
                        log::warn!("the daemon is still another build after the handover");
                    }
                }
                // Keep the old daemon rather than no daemon.
                Err(error) => log::warn!("handing the daemon over: {error:#}"),
            }
        }
        let (tx, rx) = unbounded();
        tx.unbounded_send(welcome).ok();
        std::thread::spawn(move || {
            loop {
                match read_message::<Notice>(&mut reader) {
                    Ok(Some(notice)) => {
                        if tx.unbounded_send(notice).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {
                        tx.unbounded_send(Notice::Error {
                            message: "the daemon closed the connection".into(),
                        })
                        .ok();
                        break;
                    }
                    Err(error) => {
                        log::error!("reading from the daemon: {error:#}");
                        break;
                    }
                }
            }
        });
        Ok((
            Self {
                writer: Mutex::new(stream),
            },
            rx,
        ))
    }

    pub fn request(&self, request: &Request) {
        let mut writer = self
            .writer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Err(error) = write_message(&mut *writer, request) {
            log::error!("writing to the daemon: {error:#}");
        }
    }
}

/// Connects, starting the daemon when nobody answers, and reads the daemon's first notice.
fn open(socket: &Path) -> anyhow::Result<(UnixStream, BufReader<UnixStream>, Notice)> {
    let stream = match UnixStream::connect(socket) {
        Ok(stream) => stream,
        Err(_) => {
            let daemon = start_daemon()?;
            wait_for(socket, daemon)?
        }
    };
    let mut reader = BufReader::new(stream.try_clone()?);
    let first = read_message::<Notice>(&mut reader)
        .context("reading the daemon's welcome")?
        .context("the daemon closed the connection at once")?;
    Ok((stream, reader, first))
}

/// Whether the daemon that sent `welcome` runs another build than `own`. A daemon too old to
/// say is another build.
fn is_other_build(welcome: &Notice, own: &Build) -> bool {
    match welcome {
        Notice::Welcome { build, .. } => build.as_ref() != Some(own),
        _ => false,
    }
}

/// Stops the daemon at the other end of `stream`, and, from an app bundle, points the
/// LaunchAgent (when there is one) at `own`, so login starts this build too. Runs and the
/// shared Chromium outlive the daemon, and the next one takes them over.
fn hand_over(stream: &UnixStream, own: &Path, socket: &Path) -> anyhow::Result<()> {
    let pid = peer_pid(stream).context("asking the socket which process serves it")?;
    // SAFETY: kill only sends a signal.
    if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("stopping the daemon (pid {pid})"));
    }
    let start = Instant::now();
    // SAFETY: signal 0 only checks that the process exists.
    while unsafe { libc::kill(pid, 0) } == 0 {
        anyhow::ensure!(
            start.elapsed() < STOP_TIMEOUT,
            "the daemon (pid {pid}) did not stop"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let in_bundle = own
        .parent()
        .is_some_and(|dir| dir.ends_with("Contents/MacOS"));
    let home = std::env::home_dir().unwrap_or_default();
    if in_bundle && launch_agent_plist(&home).is_file() {
        // `install` boots the old agent out and loads one for this binary, which starts it.
        let status = Command::new(own)
            .arg("install")
            .stdout(Stdio::null())
            .status()
            .with_context(|| format!("running {} install", own.display()))?;
        if !status.success() {
            log::warn!("{} install exited with {status}", own.display());
            return Ok(());
        }
        // launchd is starting the daemon; a second one started here would race it for the
        // socket, and launchd would restart a loser every few seconds.
        let start = Instant::now();
        while UnixStream::connect(socket).is_err() {
            if start.elapsed() > START_TIMEOUT {
                log::warn!("the LaunchAgent's daemon did not answer; starting one");
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Ok(())
}

/// The pid of the process at the other end of a Unix socket.
#[cfg(target_os = "macos")]
fn peer_pid(stream: &UnixStream) -> anyhow::Result<libc::pid_t> {
    // From <sys/un.h>: the socket level and option that name the peer's pid.
    const SOL_LOCAL: libc::c_int = 0;
    const LOCAL_PEERPID: libc::c_int = 2;
    let mut pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    // SAFETY: getsockopt writes at most `len` bytes into `pid`.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            SOL_LOCAL,
            LOCAL_PEERPID,
            (&raw mut pid).cast(),
            &mut len,
        )
    };
    anyhow::ensure!(result == 0, std::io::Error::last_os_error());
    Ok(pid)
}

#[cfg(not(target_os = "macos"))]
fn peer_pid(stream: &UnixStream) -> anyhow::Result<libc::pid_t> {
    // SAFETY: ucred is plain integers, for which zero is a value.
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: getsockopt writes at most `len` bytes into `cred`.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &mut len,
        )
    };
    anyhow::ensure!(result == 0, std::io::Error::last_os_error());
    Ok(cred.pid)
}

/// `$CEDIAN_DAEMON_BIN`, else `cedian-daemon` next to this binary: the build this window
/// expects to answer. None when the daemon is only found on PATH, which is not checked.
fn own_daemon() -> Option<PathBuf> {
    if let Some(bin) = std::env::var_os("CEDIAN_DAEMON_BIN") {
        return Some(PathBuf::from(bin));
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join("cedian-daemon")))
        .filter(|path| path.is_file())
}

/// The daemon to start: this window's own, else `cedian-daemon` on PATH.
fn daemon_bin() -> PathBuf {
    own_daemon().unwrap_or_else(|| PathBuf::from("cedian-daemon"))
}

/// Starts the daemon in its own process group, logging to `daemon.log` in the state directory.
fn start_daemon() -> anyhow::Result<Child> {
    let dir = state_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("daemon.log"))?;
    let bin = daemon_bin();
    Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .process_group(0)
        .spawn()
        .with_context(|| format!("starting {}", bin.display()))
}

/// The last lines of `daemon.log`, to say why a daemon this window started exited.
fn log_tail() -> String {
    let text = std::fs::read_to_string(state_dir().join("daemon.log")).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(5)..].join("\n")
}

fn wait_for(socket: &std::path::Path, mut daemon: Child) -> anyhow::Result<UnixStream> {
    let start = Instant::now();
    loop {
        match UnixStream::connect(socket) {
            Ok(stream) => {
                // The daemon outlives the window, but while the window lives it is the parent:
                // reap it when it exits, or it stays a zombie that still answers `kill -0`.
                std::thread::spawn(move || daemon.wait());
                return Ok(stream);
            }
            // Exit 0 means another daemon won the race to bind; keep connecting.
            Err(_) if daemon.try_wait()?.is_some_and(|status| !status.success()) => {
                anyhow::bail!("the daemon exited at start:\n{}", log_tail());
            }
            Err(error) if start.elapsed() > START_TIMEOUT => {
                return Err(error).with_context(|| {
                    format!(
                        "the daemon did not answer on {} (see daemon.log beside it)",
                        socket.display()
                    )
                });
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn welcome(build: Option<Build>) -> Notice {
        Notice::Welcome {
            daemon: "0.1.0".into(),
            cedia: Err("no cedia".into()),
            build,
        }
    }

    #[test]
    fn another_build_is_handed_over_and_the_same_is_kept() {
        let own = Build {
            exe: "/Applications/Cedian.app/Contents/MacOS/cedian-daemon".into(),
            modified: 2,
        };
        assert!(!is_other_build(&welcome(Some(own.clone())), &own));
        // Replaced in place since the daemon started.
        let older = Build {
            modified: 1,
            ..own.clone()
        };
        assert!(is_other_build(&welcome(Some(older)), &own));
        let elsewhere = Build {
            exe: "/Users/me/Library/Application Support/cedian/bin/cedian-daemon".into(),
            ..own.clone()
        };
        assert!(is_other_build(&welcome(Some(elsewhere)), &own));
        // Too old to say.
        assert!(is_other_build(&welcome(None), &own));
    }

    #[test]
    fn the_peer_is_the_process_across_the_socket() {
        let (a, _b) = UnixStream::pair().expect("pair");
        assert_eq!(
            peer_pid(&a).expect("peer pid"),
            std::process::id() as libc::pid_t
        );
    }
}
