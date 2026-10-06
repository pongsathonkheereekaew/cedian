//! Settings: permissions + reviewer allow-list + update channel.
//!
//! Rule (§2.5): the settings UI EDITS the same files CI gates — no second
//! source of truth. Headless format is JSON (`cedian.json`); the TOML switch
//! happens with the fork (one migration, same `Settings` shape). Unknown keys
//! fail closed — a typo'd policy must never silently become permissive.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Permission tier verdicts (canonical: Allow | Ask | Deny; Abstain is
/// system-generated, never persisted here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Allow,
    Ask,
    Deny,
}

/// `[permissions]` tiers (plan §64: safe / project_write / dangerous).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permissions {
    pub safe: Verdict,
    pub project_write: Verdict,
    pub dangerous: Verdict,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            safe: Verdict::Allow,
            project_write: Verdict::Allow,
            dangerous: Verdict::Ask,
        }
    }
}

/// Full settings document (`cedian.json` headless; `cedian.toml` with the fork).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub permissions: Permissions,
    /// Reviewer allow-list: shell commands reviewers may run (Phase 5+).
    #[serde(default)]
    pub reviewer_allow_list: Vec<String>,
    /// Update channel: `stable` | `beta` (auto-update reads this).
    #[serde(default = "default_channel")]
    pub update_channel: String,
}

fn default_channel() -> String {
    "stable".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            permissions: Permissions::default(),
            reviewer_allow_list: Vec::new(),
            update_channel: default_channel(),
        }
    }
}

/// Settings failures (caller-visible, rendered in the settings UI later).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    Parse(String),
    UnknownKey(String),
    BadChannel(String),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "settings parse error: {e}"),
            Self::UnknownKey(k) => write!(f, "unknown settings key: {k}"),
            Self::BadChannel(c) => write!(f, "bad update channel: {c} (want stable|beta)"),
        }
    }
}

impl std::error::Error for SettingsError {}

/// Known top-level keys — anything else fails closed.
const KNOWN_KEYS: &[&str] = &["permissions", "reviewer_allow_list", "update_channel"];

/// Parse + validate settings JSON. Unknown keys and bad channels fail closed.
pub fn load_settings(json: &str) -> Result<Settings, SettingsError> {
    let raw: HashMap<String, serde_json::Value> =
        serde_json::from_str(json).map_err(|e| SettingsError::Parse(e.to_string()))?;
    for key in raw.keys() {
        if !KNOWN_KEYS.contains(&key.as_str()) {
            return Err(SettingsError::UnknownKey(key.clone()));
        }
    }
    let settings: Settings =
        serde_json::from_str(json).map_err(|e| SettingsError::Parse(e.to_string()))?;
    if settings.update_channel != "stable" && settings.update_channel != "beta" {
        return Err(SettingsError::BadChannel(settings.update_channel.clone()));
    }
    Ok(settings)
}

/// Canonical default document (what onboarding writes on first run).
pub fn default_settings_toml() -> String {
    serde_json::to_string_pretty(&Settings::default()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe() {
        let s = load_settings("{}").unwrap();
        assert_eq!(s.permissions.dangerous, Verdict::Ask);
        assert_eq!(s.update_channel, "stable");
    }

    #[test]
    fn unknown_key_fails_closed() {
        assert!(matches!(
            load_settings(r#"{"dangerous_stuff": true}"#),
            Err(SettingsError::UnknownKey(_))
        ));
    }

    #[test]
    fn bad_channel_rejected() {
        assert!(matches!(
            load_settings(r#"{"update_channel": "nightly"}"#),
            Err(SettingsError::BadChannel(_))
        ));
    }
}
