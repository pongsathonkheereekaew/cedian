//! The optional sound when a thread finishes or fails (Interface, Notifications), from
//! `desktop.sound`, off by default. It plays the system's own sounds: Glass and Basso on macOS,
//! the freedesktop `complete` and `dialog-error` through `canberra-gtk-play` elsewhere, when it
//! is installed.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sound {
    #[default]
    Off,
    On,
}

/// Which way a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    /// Done, or stopped on a question for the person.
    Finished,
    Failed,
}

impl Sound {
    /// Each choice, its `desktop.sound` value and its word in Settings.
    pub const ALL: [(Sound, &'static str, &'static str); 2] =
        [(Sound::Off, "off", "ปิด"), (Sound::On, "on", "เปิด")];

    pub fn from_key(key: Option<&str>) -> Self {
        Self::ALL
            .iter()
            .find(|(_, k, _)| Some(*k) == key)
            .map_or_else(Self::default, |(sound, _, _)| *sound)
    }

    pub fn play(self, ending: Ending) {
        if self == Sound::Off {
            return;
        }
        let Some(mut command) = command(ending) else {
            return;
        };
        match command.spawn() {
            // Reaped on its own thread, so no zombie is left behind.
            Ok(mut child) => {
                std::thread::spawn(move || child.wait());
            }
            Err(error) => log::warn!("playing the {ending:?} sound: {error}"),
        }
    }
}

#[cfg(target_os = "macos")]
fn command(ending: Ending) -> Option<std::process::Command> {
    let name = match ending {
        Ending::Finished => "Glass",
        Ending::Failed => "Basso",
    };
    let mut command = std::process::Command::new("/usr/bin/afplay");
    command.arg(format!("/System/Library/Sounds/{name}.aiff"));
    Some(command)
}

#[cfg(not(target_os = "macos"))]
fn command(ending: Ending) -> Option<std::process::Command> {
    let path = std::env::var_os("PATH")?;
    let player = std::env::split_paths(&path)
        .map(|dir| dir.join("canberra-gtk-play"))
        .find(|bin| bin.is_file())?;
    let id = match ending {
        Ending::Finished => "complete",
        Ending::Failed => "dialog-error",
    };
    let mut command = std::process::Command::new(player);
    command.args(["-i", id]);
    Some(command)
}

/// The sound a change of a thread's state from `before` to `after` calls for: only the end of
/// a run that was working.
pub fn ending(before: Option<cedian_wire::Status>, after: cedian_wire::Status) -> Option<Ending> {
    use cedian_wire::Status;
    if before != Some(Status::Working) {
        return None;
    }
    match after {
        Status::Done | Status::Asking => Some(Ending::Finished),
        Status::Failed => Some(Ending::Failed),
        Status::Queued | Status::Working | Status::Stopped | Status::Interrupted => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cedian_wire::Status;

    #[test]
    fn only_a_run_that_ends_rings() {
        assert_eq!(Sound::from_key(Some("on")), Sound::On);
        assert_eq!(Sound::from_key(None), Sound::Off);
        assert_eq!(
            ending(Some(Status::Working), Status::Done),
            Some(Ending::Finished)
        );
        assert_eq!(
            ending(Some(Status::Working), Status::Failed),
            Some(Ending::Failed)
        );
        assert_eq!(ending(Some(Status::Working), Status::Stopped), None);
        assert_eq!(ending(Some(Status::Done), Status::Done), None);
        assert_eq!(ending(None, Status::Failed), None);
    }
}
