//! The shared browser (Interface, Phase C: Browser): one headless Chromium the daemon starts the
//! first time a window opens a Browser block, with a page per project. The block draws the
//! page and the project's runs drive the same page through cedia's browser tool, so the person
//! and the agent see one page. Its profile lives in the daemon's state directory, so sign-ins
//! last; a daemon that restarts finds the Chromium still running there and keeps using it.
//! The Chromium is the headless shell cedian ships beside the daemon, else an installed one.

use std::{
    collections::HashMap,
    io::{Read as _, Write as _},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context as _, anyhow, bail};
use cedian_wire::BrowserPage;

/// How long Chromium has to say which port it listens on.
const START_TIMEOUT: Duration = Duration::from_secs(15);
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

/// The Chromium cedian ships (`script/fetch-chromium`), relative to the daemon's directory:
/// in the app bundle's Resources, else beside the build output in development. The directory
/// is not named after the binary: codesign takes `x/x` for an old-style bundle and fails.
const BUNDLED: [&str; 2] = [
    "../Resources/chromium/chrome-headless-shell",
    "../chromium/chrome-headless-shell",
];

/// Where an installed browser is looked for, after the bundled one.
#[cfg(target_os = "macos")]
const APPS: [&str; 5] = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
];
#[cfg(not(target_os = "macos"))]
const APPS: [&str; 0] = [];
const ON_PATH: [&str; 5] = [
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
    "microsoft-edge",
];

pub struct Browser {
    /// The profile directory, which also holds `DevToolsActivePort` while Chromium runs.
    profile: PathBuf,
    env: HashMap<String, String>,
    port: Option<u16>,
    /// Each project's page id, by cedia's project key.
    pages: HashMap<String, String>,
}

impl Browser {
    pub fn new(state_dir: &Path, env: HashMap<String, String>) -> Self {
        Self {
            profile: state_dir.join("browser"),
            env,
            port: None,
            pages: HashMap::new(),
        }
    }

    /// The project's page, starting Chromium first when it does not run.
    pub fn page(&mut self, project: &str) -> anyhow::Result<BrowserPage> {
        let port = match self.alive() {
            Some(port) => port,
            None => self.start()?,
        };
        self.page_on(port, project)
    }

    /// The project's page when Chromium already runs, for a run: a run never starts it.
    pub fn running_page(&mut self, project: &str) -> Option<BrowserPage> {
        let port = self.alive()?;
        self.page_on(port, project)
            .inspect_err(|e| log::warn!("browser page for {project}: {e:#}"))
            .ok()
    }

    /// The port of a Chromium that answers: the one known, else one a previous daemon started
    /// on the same profile.
    fn alive(&mut self) -> Option<u16> {
        let port = self.port.or_else(|| read_port(&self.profile))?;
        if http(port, "GET", "/json/version").is_ok() {
            self.port = Some(port);
            Some(port)
        } else {
            self.port = None;
            self.pages.clear();
            None
        }
    }

    fn start(&mut self) -> anyhow::Result<u16> {
        let exe = std::env::current_exe()
            .inspect_err(|e| log::warn!("the daemon's own path: {e}"))
            .ok();
        let bin = find_chromium(&self.env, exe.as_deref()).ok_or_else(|| {
            anyhow!(
                "ไม่พบ Chromium: รัน script/fetch-chromium, ติดตั้ง Google Chrome หรือตั้ง CEDIAN_CHROMIUM"
            )
        })?;
        std::fs::create_dir_all(&self.profile)
            .with_context(|| format!("creating {}", self.profile.display()))?;
        let port_file = self.profile.join("DevToolsActivePort");
        // A port file a Chromium that is gone left behind.
        let _ = std::fs::remove_file(&port_file);
        log::info!("starting {}", bin.display());
        Command::new(&bin)
            .args(chromium_args(&self.profile))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("starting {}", bin.display()))?;
        let since = Instant::now();
        while since.elapsed() < START_TIMEOUT {
            if let Some(port) = read_port(&self.profile)
                && http(port, "GET", "/json/version").is_ok()
            {
                self.port = Some(port);
                self.pages.clear();
                return Ok(port);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        bail!(
            "{} did not start in {}s",
            bin.display(),
            START_TIMEOUT.as_secs()
        )
    }

    /// The project's page if Chromium still has it, else a new one.
    fn page_on(&mut self, port: u16, project: &str) -> anyhow::Result<BrowserPage> {
        let targets: Vec<Target> = serde_json::from_str(&http(port, "GET", "/json/list")?)
            .context("reading the browser's pages")?;
        let known = self
            .pages
            .get(project)
            .filter(|id| targets.iter().any(|t| &t.id == *id && t.kind == "page"))
            .cloned();
        let id = match known {
            Some(id) => id,
            None => {
                let made: Target =
                    serde_json::from_str(&http(port, "PUT", "/json/new?about:blank")?)
                        .context("opening a page")?;
                self.pages.insert(project.to_owned(), made.id.clone());
                made.id
            }
        };
        Ok(BrowserPage {
            endpoint: format!("http://127.0.0.1:{port}"),
            socket: format!("ws://127.0.0.1:{port}/devtools/page/{id}"),
        })
    }
}

#[derive(serde::Deserialize)]
struct Target {
    id: String,
    #[serde(rename = "type")]
    kind: String,
}

/// Headless: the Browser block draws the page, so Chromium shows no window of its own.
fn chromium_args(profile: &Path) -> Vec<String> {
    // Chromium refuses its own sandbox as root, which only a container runs as.
    // SAFETY: geteuid has no preconditions.
    let root = cfg!(target_os = "linux") && unsafe { libc::geteuid() } == 0;
    root.then(|| "--no-sandbox".to_owned())
        .into_iter()
        .chain([
            "--headless=new".to_owned(),
            "--remote-debugging-port=0".to_owned(),
            "--remote-debugging-address=127.0.0.1".to_owned(),
            format!("--user-data-dir={}", profile.display()),
            "--no-first-run".to_owned(),
            "--no-default-browser-check".to_owned(),
            "--hide-scrollbars".to_owned(),
            // A headless screencast is drawn at the screen's scale, which is 1 whatever
            // `Emulation.setDeviceMetricsOverride` says; at 1 a Retina window stretches each
            // page pixel over four. The window's `maxWidth` scales frames down on a 1x screen.
            "--force-device-scale-factor=2".to_owned(),
            "about:blank".to_owned(),
        ])
        .collect()
}

/// `$CEDIAN_CHROMIUM`, else the bundled headless shell beside `exe`, else an installed
/// Chrome-family browser, else one on the login PATH.
pub fn find_chromium(env: &HashMap<String, String>, exe: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = env.get("CEDIAN_CHROMIUM") {
        return Some(PathBuf::from(path));
    }
    if let Some(dir) = exe.and_then(Path::parent)
        && let Some(bundled) = BUNDLED.iter().map(|p| dir.join(p)).find(|p| p.is_file())
    {
        return Some(bundled);
    }
    if let Some(app) = APPS.iter().map(PathBuf::from).find(|p| p.is_file()) {
        return Some(app);
    }
    let path = env.get("PATH")?;
    std::env::split_paths(path)
        .flat_map(|dir| ON_PATH.iter().map(move |name| dir.join(name)))
        .find(|p| p.is_file())
}

/// The port Chromium wrote on its first line of `DevToolsActivePort`.
fn read_port(profile: &Path) -> Option<u16> {
    let text = std::fs::read_to_string(profile.join("DevToolsActivePort")).ok()?;
    text.lines().next()?.trim().parse().ok()
}

/// One request to Chromium's DevTools HTTP side, which answers with a length and closes.
fn http(port: u16, method: &str, path: &str) -> anyhow::Result<String> {
    let mut stream = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), HTTP_TIMEOUT)?;
    stream.set_read_timeout(Some(HTTP_TIMEOUT))?;
    stream.set_write_timeout(Some(HTTP_TIMEOUT))?;
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    // Chromium keeps the connection open, so the body ends where its length says.
    let mut reply = Vec::new();
    let mut chunk = [0u8; 8192];
    let (head, body) = loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            bail!("the browser closed the connection at {path}");
        }
        reply.extend_from_slice(&chunk[..read]);
        let text = String::from_utf8_lossy(&reply);
        let Some((head, body)) = text.split_once("\r\n\r\n") else {
            continue;
        };
        let length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())?
            })
            .unwrap_or(0);
        if body.len() >= length {
            break (head.to_owned(), body.to_owned());
        }
    };
    if !head.starts_with("HTTP/1.1 200") {
        bail!(
            "the browser answered {path} with {}",
            head.lines().next().unwrap_or("")
        );
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chromium_comes_from_the_environment_first() {
        let env = HashMap::from([("CEDIAN_CHROMIUM".to_owned(), "/opt/chrome".to_owned())]);
        assert_eq!(
            find_chromium(&env, None),
            Some(PathBuf::from("/opt/chrome"))
        );
    }

    #[test]
    fn the_bundled_shell_comes_before_an_installed_browser() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir_all(dir.path().join("MacOS")).expect("mkdir");
        let exe = dir.path().join("MacOS/cedian-daemon");
        let shell = dir.path().join("Resources/chromium/chrome-headless-shell");
        std::fs::create_dir_all(shell.parent().expect("parent")).expect("mkdir");
        std::fs::write(&shell, "").expect("write");
        let env = HashMap::from([("PATH".to_owned(), "/nowhere".to_owned())]);
        let found = |exe: &Path| {
            find_chromium(&env, Some(exe)).map(|p| p.canonicalize().expect("canonical"))
        };
        assert_eq!(found(&exe), Some(shell.canonicalize().expect("canonical")));
        // In development the build output's directory holds it.
        let dev = tempfile::tempdir().expect("dir");
        let shell = dev.path().join("chromium/chrome-headless-shell");
        std::fs::create_dir_all(shell.parent().expect("parent")).expect("mkdir");
        std::fs::write(&shell, "").expect("write");
        std::fs::create_dir_all(dev.path().join("debug")).expect("mkdir");
        let exe = dev.path().join("debug/cedian-daemon");
        assert_eq!(found(&exe), Some(shell.canonicalize().expect("canonical")));
    }

    #[test]
    fn a_port_file_names_its_port_on_the_first_line() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join("DevToolsActivePort"),
            "40123\n/devtools/browser/x\n",
        )
        .expect("write");
        assert_eq!(read_port(dir.path()), Some(40123));
    }

    /// Against a real Chromium, when one is here (`CEDIAN_CHROMIUM`, or Playwright's).
    #[test]
    fn starts_chromium_and_keeps_a_page_per_project() {
        let bin = std::env::var("CEDIAN_CHROMIUM").ok().or_else(|| {
            Path::new("/opt/pw-browsers/chromium")
                .exists()
                .then(|| "/opt/pw-browsers/chromium".to_owned())
        });
        let Some(bin) = bin else { return };
        let dir = tempfile::tempdir().expect("dir");
        let env = HashMap::from([("CEDIAN_CHROMIUM".to_owned(), bin)]);
        let mut browser = Browser::new(dir.path(), env.clone());
        assert!(browser.running_page("a").is_none(), "a run never starts it");
        let a = browser.page("a").expect("page");
        assert_eq!(browser.page("a").expect("again"), a);
        let b = browser.page("b").expect("other project");
        assert_ne!(a.socket, b.socket);
        // A daemon that restarts finds the same Chromium and opens pages there.
        let mut again = Browser::new(dir.path(), env);
        let c = again.running_page("a").expect("still running");
        assert_eq!(c.endpoint, a.endpoint);
        let version = http(browser.port.expect("port"), "GET", "/json/version").expect("version");
        assert!(version.contains("webSocketDebuggerUrl"));
        let _ = Command::new("pkill")
            .arg("-f")
            .arg(dir.path().join("browser").display().to_string())
            .status();
    }
}
