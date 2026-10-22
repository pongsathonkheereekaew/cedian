//! Keeps the Mac from idle sleep while any run works, so a delegated thread does not stall
//! when the person walks away. `caffeinate -i -w <daemon pid>` holds the assertion and ends
//! with the daemon if the daemon dies first.

use std::process::{Child, Command, Stdio};

#[derive(Default)]
pub struct KeepAwake {
    caffeinate: Option<Child>,
}

impl KeepAwake {
    pub fn set(&mut self, working: bool) {
        if working == self.caffeinate.is_some() || !cfg!(target_os = "macos") {
            return;
        }
        match self.caffeinate.take() {
            Some(mut child) => {
                if let Err(error) = child.kill() {
                    log::warn!("ending caffeinate: {error}");
                }
                child.wait().ok();
                log::info!("idle sleep allowed again");
            }
            None => {
                let spawned = Command::new("/usr/bin/caffeinate")
                    .args(["-i", "-w", &std::process::id().to_string()])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
                match spawned {
                    Ok(child) => {
                        log::info!("keeping the Mac awake while runs work");
                        self.caffeinate = Some(child);
                    }
                    Err(error) => log::warn!("starting caffeinate: {error}"),
                }
            }
        }
    }
}
