use std::collections::HashMap;

/// The login shell's environment, read once so cedia finds `git`, `gh` and `claude` as it does
/// in a terminal. The shell runs this binary with `--printenv`, as Zed does. Falls back to the
/// daemon's own environment.
pub fn login_environment() -> HashMap<String, String> {
    let own: HashMap<String, String> = std::env::vars().collect();
    if std::env::var_os("CEDIAN_NO_LOGIN_ENV").is_some() {
        return own;
    }
    let shell = own
        .get("SHELL")
        .cloned()
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
