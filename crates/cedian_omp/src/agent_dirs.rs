//! Where cedian's agent keeps its files (ADR-0078 decision 5): never the
//! person's `~/.omp`.
//!
//! - config (`config.yml`, `models.yml`, skills, rules, agents):
//!   `$XDG_CONFIG_HOME/cedian/agent`, else `~/.config/cedian/agent`, beside
//!   `cedian.toml`. The agent sees it as `PI_CODING_AGENT_DIR`.
//! - everything the agent writes (sessions, logs, tokens, locks, caches):
//!   `<state root>/agent`, where the state root is `$CEDIAN_STATE_DIR`, else
//!   `$XDG_STATE_HOME/cedian`, else `~/.local/state/cedian`. The agent sees
//!   it as `PI_STATE_DIR` (a cedian-agent variable; unset, the agent falls
//!   back to `~/.omp`).

use crate::OmpError;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The agent's config dir and state dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDirs {
    pub config: PathBuf,
    pub state: PathBuf,
}

impl AgentDirs {
    /// The dirs for this process's environment.
    pub fn from_env() -> Result<Self, OmpError> {
        Self::from_vars(&std::env::vars().collect())
    }

    /// The dirs for `vars`. Refused when neither the XDG variables nor HOME
    /// name a place: without them the agent would fall back to `~/.omp`.
    pub fn from_vars(vars: &BTreeMap<String, String>) -> Result<Self, OmpError> {
        let set = |name: &str| {
            vars.get(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
        };
        let home = set("HOME");
        let missing = |what: &str| {
            OmpError::InvalidSpawnProfile(format!(
                "HOME is not set, so the agent has no {what} dir"
            ))
        };
        let config = set("XDG_CONFIG_HOME")
            .or_else(|| home.as_ref().map(|h| h.join(".config")))
            .ok_or_else(|| missing("config"))?
            .join("cedian/agent");
        let state = state_root(vars)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| missing("state"))?
            .join("agent");
        Ok(Self { config, state })
    }

    /// The variables that point the agent at these dirs.
    pub fn env(&self) -> [(String, String); 2] {
        [
            ("PI_CODING_AGENT_DIR", &self.config),
            ("PI_STATE_DIR", &self.state),
        ]
        .map(|(name, dir)| (name.to_string(), dir.to_string_lossy().into_owned()))
    }
}

/// cedian's state root for `vars`: `$CEDIAN_STATE_DIR`, else
/// `$XDG_STATE_HOME/cedian`, else `$HOME/.local/state/cedian`.
pub fn state_root(vars: &BTreeMap<String, String>) -> Option<PathBuf> {
    let set = |name: &str| vars.get(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    set("CEDIAN_STATE_DIR")
        .or_else(|| set("XDG_STATE_HOME").map(|d| d.join("cedian")))
        .or_else(|| set("HOME").map(|h| h.join(".local/state/cedian")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn home_defaults() {
        let dirs = AgentDirs::from_vars(&vars(&[("HOME", "/Users/u")])).unwrap();
        assert_eq!(dirs.config, PathBuf::from("/Users/u/.config/cedian/agent"));
        assert_eq!(
            dirs.state,
            PathBuf::from("/Users/u/.local/state/cedian/agent")
        );
    }

    #[test]
    fn xdg_and_cedian_state_dir_win() {
        let dirs = AgentDirs::from_vars(&vars(&[
            ("HOME", "/Users/u"),
            ("XDG_CONFIG_HOME", "/c"),
            ("XDG_STATE_HOME", "/s"),
        ]))
        .unwrap();
        assert_eq!(dirs.config, PathBuf::from("/c/cedian/agent"));
        assert_eq!(dirs.state, PathBuf::from("/s/cedian/agent"));
        let dirs = AgentDirs::from_vars(&vars(&[
            ("HOME", "/Users/u"),
            ("XDG_STATE_HOME", "/s"),
            ("CEDIAN_STATE_DIR", "/st"),
        ]))
        .unwrap();
        assert_eq!(dirs.state, PathBuf::from("/st/agent"));
    }

    #[test]
    fn no_home_is_refused() {
        assert!(AgentDirs::from_vars(&vars(&[("PATH", "/usr/bin")])).is_err());
    }
}
