//! The CLI's view of the workspace state dir (ADR-0044); the rules live in
//! `cedian_shell::state`. Unit tests use a per-process temp root, never the
//! user's real state.

use std::path::{Path, PathBuf};

fn root() -> Result<PathBuf, String> {
    #[cfg(test)]
    if std::env::var_os("CEDIAN_STATE_DIR").is_none() {
        return Ok(test_state_root());
    }
    cedian_shell::state::root()
}

#[cfg(test)]
fn test_state_root() -> PathBuf {
    use std::sync::{Once, OnceLock};
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    static ATEXIT: Once = Once::new();
    let path = ROOT
        .get_or_init(|| {
            let dir =
                std::env::temp_dir().join(format!("cedian-test-state-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            dir
        })
        .clone();
    ATEXIT.call_once(|| {
        extern "C" fn scrub() {
            if let Some(path) = ROOT.get() {
                let _ = std::fs::remove_dir_all(path);
            }
        }
        unsafe extern "C" {
            fn atexit(cb: extern "C" fn()) -> i32;
        }
        unsafe {
            atexit(scrub);
        }
    });
    path
}

/// The state directory for `workdir`, created on first use.
pub fn dir(workdir: &Path) -> Result<PathBuf, String> {
    cedian_shell::state::dir_in(&root()?, workdir)
}

/// `name` inside the state directory for `workdir`.
pub fn file(workdir: &Path, name: &str) -> Result<PathBuf, String> {
    Ok(dir(workdir)?.join(name))
}
