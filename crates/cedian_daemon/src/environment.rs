use std::collections::HashMap;

/// The login shell's environment, read once so cedia finds `git`, `gh` and `claude` as it does
/// in a terminal. The shell runs this binary with `--printenv`, as Zed does. Falls back to the
/// daemon's own environment.
pub fn login_environment() -> HashMap<String, String> {
    let own: HashMap<String, String> = std::env::vars().collect();
    if std::env::var_os("CEDIAN_NO_LOGIN_ENV").is_some() {
        return own;
    }
    // launchd starts agents without SHELL; the account's shell is in the password database.
    let shell = own
        .get("SHELL")
        .cloned()
        .or_else(account_shell)
        .unwrap_or_else(|| "/bin/sh".to_owned());
    let home = own.get("HOME").cloned().unwrap_or_else(|| "/".to_owned());
    match futures::executor::block_on(util::shell_env::capture(&shell, &[], &home)) {
        Ok(env) => env.into_iter().collect(),
        Err(error) => {
            log::warn!("reading {shell}'s login environment: {error:#}");
            own
        }
    }
}

fn account_shell() -> Option<String> {
    // SAFETY: getpwuid returns a pointer into static storage or null, read at once before
    // anything else on this thread calls getpw*.
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() || (*entry).pw_shell.is_null() {
            return None;
        }
        let shell = std::ffi::CStr::from_ptr((*entry).pw_shell)
            .to_string_lossy()
            .into_owned();
        (!shell.is_empty()).then_some(shell)
    }
}
