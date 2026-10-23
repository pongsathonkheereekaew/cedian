use std::path::{Path, PathBuf};

/// The daemon's LaunchAgent, which `cedian-daemon install` writes; the window looks for it to
/// point it at the app's own daemon after a handover.
pub const LAUNCH_AGENT_LABEL: &str = "dev.cedian.daemon";

pub fn launch_agent_plist(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
        .join(format!("{LAUNCH_AGENT_LABEL}.plist"))
}
