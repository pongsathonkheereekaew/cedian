//! The daemon as a per-user LaunchAgent: launchd starts it at login and restarts it when it
//! crashes. `cedian-daemon install` writes the agent's plist and loads it; `uninstall` undoes
//! both. A bundled app will register the same agent with `SMAppService` instead.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Context as _;

pub const LABEL: &str = "dev.cedian.daemon";

/// Seconds launchd waits before starting the daemon again after it exits with an error.
const THROTTLE_SECONDS: u32 = 10;

pub fn plist_path(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

/// The agent: run `daemon` at login, again after a crash (not after a clean exit, which
/// means another daemon already serves), logging to `log`.
pub fn plist(daemon: &Path, log: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{LABEL}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{daemon}</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
	<key>ThrottleInterval</key>
	<integer>{THROTTLE_SECONDS}</integer>
	<key>ProcessType</key>
	<string>Interactive</string>
	<key>StandardOutPath</key>
	<string>{log}</string>
	<key>StandardErrorPath</key>
	<string>{log}</string>
</dict>
</plist>
"#,
        daemon = xml_escape(&daemon.display().to_string()),
        log = xml_escape(&log.display().to_string()),
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn domain() -> String {
    // SAFETY: getuid cannot fail.
    format!("gui/{}", unsafe { libc::getuid() })
}

fn launchctl(args: &[&str]) -> anyhow::Result<std::process::Output> {
    Command::new("launchctl")
        .args(args)
        .output()
        .with_context(|| format!("running launchctl {}", args.join(" ")))
}

pub fn install(home: &Path, log: &Path) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(cfg!(target_os = "macos"), "LaunchAgents are macOS only");
    let daemon = std::env::current_exe()?.canonicalize()?;
    let path = plist_path(home);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // A loaded agent keeps its old plist until it is booted out.
    launchctl(&["bootout", &format!("{}/{LABEL}", domain())])?;
    std::fs::write(&path, plist(&daemon, log))
        .with_context(|| format!("writing {}", path.display()))?;
    let output = launchctl(&["bootstrap", &domain(), &path.display().to_string()])?;
    anyhow::ensure!(
        output.status.success(),
        "launchctl bootstrap: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(path)
}

pub fn uninstall(home: &Path) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(cfg!(target_os = "macos"), "LaunchAgents are macOS only");
    launchctl(&["bootout", &format!("{}/{LABEL}", domain())])?;
    let path = plist_path(home);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("removing {}", path.display())),
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plist_restarts_only_after_a_crash() {
        let text = plist(
            Path::new("/Applications/cedian & co/cedian-daemon"),
            Path::new("/Users/me/Library/Application Support/cedian/daemon.log"),
        );
        assert!(text.contains("<string>/Applications/cedian &amp; co/cedian-daemon</string>"));
        assert!(text.contains("<key>SuccessfulExit</key>\n\t\t<false/>"));
        assert!(text.contains("<key>RunAtLoad</key>\n\t<true/>"));
    }
}
