//! Opening the thread's directory in another app (Interface, Layout: toolbar): the file
//! manager always, and each editor or terminal found on this machine.

use std::path::{Path, PathBuf};

/// An app the toolbar's เปิดใน menu offers.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenIn {
    pub label: &'static str,
    how: How,
}

#[derive(Clone, Debug, PartialEq)]
enum How {
    /// The platform's file manager.
    Files,
    /// A command that takes the directory as its argument.
    Command(PathBuf),
    /// A macOS app bundle, opened with `open -a`.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Bundle(PathBuf),
}

/// Editors and terminals cedian knows: the label, the command on `PATH`, and the macOS app.
const KNOWN: [(&str, &str, &str); 5] = [
    ("VS Code", "code", "Visual Studio Code.app"),
    ("Cursor", "cursor", "Cursor.app"),
    ("Zed", "zed", "Zed.app"),
    ("Ghostty", "ghostty", "Ghostty.app"),
    ("Terminal", "", "Utilities/Terminal.app"),
];

#[cfg(target_os = "macos")]
const FILES: &str = "Finder";
#[cfg(not(target_os = "macos"))]
const FILES: &str = "ตัวจัดการไฟล์";

/// What this machine can open a directory in, the file manager first.
pub fn available() -> Vec<OpenIn> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    let mut apps = vec![OpenIn {
        label: FILES,
        how: How::Files,
    }];
    for (label, command, bundle) in KNOWN {
        if let Some(how) = find(&dirs, command, bundle) {
            apps.push(OpenIn { label, how });
        }
    }
    apps
}

fn find(dirs: &[PathBuf], command: &str, bundle: &str) -> Option<How> {
    if cfg!(target_os = "macos") {
        // An app launched from the Dock has a short PATH, so the bundle is the surer sign.
        let app = Path::new("/Applications").join(bundle);
        if app.exists() {
            return Some(How::Bundle(app));
        }
    }
    if command.is_empty() {
        return None;
    }
    dirs.iter()
        .map(|dir| dir.join(command))
        .find(|bin| bin.is_file())
        .map(How::Command)
}

impl OpenIn {
    pub fn open(&self, dir: &Path, cx: &gpui::App) {
        let spawned = match &self.how {
            How::Files => {
                cx.open_with_system(dir);
                return;
            }
            How::Command(bin) => std::process::Command::new(bin).arg(dir).spawn(),
            How::Bundle(app) => std::process::Command::new("open")
                .arg("-a")
                .arg(app)
                .arg(dir)
                .spawn(),
        };
        match spawned {
            // Reaped on its own thread, so a closed editor leaves no zombie behind.
            Ok(mut child) => {
                std::thread::spawn(move || child.wait());
            }
            Err(error) => log::warn!("opening {} in {}: {error}", dir.display(), self.label),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_on_the_path_is_offered() {
        let dir = std::env::temp_dir().join(format!("cedian-open-in-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("zed"), "").expect("bin");
        let found = find(std::slice::from_ref(&dir), "zed", "No Such App.app");
        assert_eq!(found, Some(How::Command(dir.join("zed"))));
        assert_eq!(
            find(std::slice::from_ref(&dir), "code", "No Such App.app"),
            None
        );
        assert_eq!(
            find(std::slice::from_ref(&dir), "", "No Such App.app"),
            None
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
