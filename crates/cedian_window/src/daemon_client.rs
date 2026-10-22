//! The window's end of the daemon's socket. The window starts the daemon when nobody answers,
//! then only talks to it: quitting the window leaves the daemon and its runs going.

use std::{
    io::BufReader,
    os::unix::{net::UnixStream, process::CommandExt as _},
    path::PathBuf,
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

use anyhow::Context as _;
use cedian_wire::{Notice, Request, read_message, socket_path, state_dir, write_message};
use futures::channel::mpsc::{UnboundedReceiver, unbounded};

/// How long a daemon this window started gets to read the login environment and bind.
const START_TIMEOUT: Duration = Duration::from_secs(15);

pub struct DaemonClient {
    writer: Mutex<UnixStream>,
}

impl DaemonClient {
    /// Connects, starting the daemon first if needed. Blocks; call it off the main thread.
    pub fn connect() -> anyhow::Result<(Self, UnboundedReceiver<Notice>)> {
        let socket = socket_path();
        let stream = match UnixStream::connect(&socket) {
            Ok(stream) => stream,
            Err(_) => {
                start_daemon()?;
                wait_for(&socket)?
            }
        };
        let reader = stream.try_clone()?;
        let (tx, rx) = unbounded();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(reader);
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

/// `$CEDIAN_DAEMON_BIN`, else `cedian-daemon` next to this binary, else on PATH.
fn daemon_bin() -> PathBuf {
    if let Some(bin) = std::env::var_os("CEDIAN_DAEMON_BIN") {
        return PathBuf::from(bin);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join("cedian-daemon")))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("cedian-daemon"))
}

/// Starts the daemon in its own process group, logging to `daemon.log` in the state directory.
fn start_daemon() -> anyhow::Result<()> {
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
        .with_context(|| format!("starting {}", bin.display()))?;
    Ok(())
}

fn wait_for(socket: &std::path::Path) -> anyhow::Result<UnixStream> {
    let start = Instant::now();
    loop {
        match UnixStream::connect(socket) {
            Ok(stream) => return Ok(stream),
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
