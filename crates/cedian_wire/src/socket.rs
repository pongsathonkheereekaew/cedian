use std::path::PathBuf;

/// Where the daemon keeps its socket and log: `$CEDIAN_STATE_DIR`, else
/// `~/Library/Application Support/cedian` on macOS and `$XDG_STATE_HOME/cedian` (or
/// `~/.local/state/cedian`) elsewhere. Never under `~/.cedia`, which is cedia's.
pub fn state_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CEDIAN_STATE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    if cfg!(target_os = "macos") {
        return home.join("Library/Application Support/cedian");
    }
    match std::env::var_os("XDG_STATE_HOME") {
        Some(dir) => PathBuf::from(dir).join("cedian"),
        None => home.join(".local/state/cedian"),
    }
}

pub fn socket_path() -> PathBuf {
    state_dir().join("daemon.sock")
}
