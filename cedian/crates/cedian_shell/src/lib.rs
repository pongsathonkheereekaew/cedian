//! `cedian_shell`: headless app-shell models (Phase 2.5 without GPUI).
//!
//! Owns what the plan's App Shell owns minus rendering: settings TOML
//! (permissions + reviewer allow-list + update channel — the SAME files CI
//! gates, no second source of truth), palette registry (every agent action
//! discoverable), session manager (task lifecycle over OMP sessions).
//! Onboarding wizard + auto-update + keybinding UI bind with the Zed fork.

pub mod palette;
pub mod session_manager;
pub mod settings;

pub use palette::{Palette, PaletteAction};
pub use session_manager::{SessionEntry, SessionManager, SESSIONS_SNAPSHOT_VERSION};
pub use settings::{
    default_settings_toml, load_settings, Settings, SettingsError, Verdict, SETTINGS_SCHEMA,
};
