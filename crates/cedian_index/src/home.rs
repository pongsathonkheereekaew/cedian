use std::path::PathBuf;

/// cedia's home: `$CEDIA_HOME`, else `~/.cedia`, as cedia resolves it.
pub fn cedia_home() -> PathBuf {
    if let Some(home) = std::env::var_os("CEDIA_HOME") {
        return PathBuf::from(home);
    }
    let user = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    user.join(".cedia")
}
