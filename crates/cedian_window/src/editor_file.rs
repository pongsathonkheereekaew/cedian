//! The file the Editor block holds (Review and editing): where it lives, the text the disk
//! had when it was last read or written, and how the disk has moved since. `⌘S` writes the
//! person's text; a change on disk is read in while the person has not edited, and while they
//! have it waits for Override (their text is written, and the thread told) or Discard.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    time::SystemTime,
};

use gpui::actions;

actions!(editor_file, [Save]);

/// The longest file the Editor opens for editing; a longer one is named, not shown.
pub const MAX_EDITED: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct EditorFile {
    pub path: PathBuf,
    /// What the disk held when it was last read or written here.
    pub disk: String,
    /// When and how long the file was then, so a check reads it again only after a change.
    stamp: Option<(SystemTime, u64)>,
}

/// How the disk compares with what the Editor last read or wrote.
#[derive(Clone, Debug, PartialEq)]
pub enum DiskChange {
    Same,
    /// The disk has new text and the person has not edited: show it.
    Reload(String),
    /// The disk has new text and so does the person: the disk's waits for their choice.
    Conflict(String),
    /// The file is no longer there.
    Gone,
}

/// What a file holds for the Editor: text to edit, or why it is only named.
#[derive(Clone, Debug, PartialEq)]
pub enum Opened {
    Text(String),
    Shown(String),
}

impl EditorFile {
    /// Reads `path` for the Editor: its text and the file to keep, or a line naming why it
    /// cannot be edited (too big, binary, unreadable).
    pub fn open(path: &Path) -> (Opened, Option<Self>) {
        match std::fs::read(path) {
            Ok(bytes) if bytes.len() > MAX_EDITED => (
                Opened::Shown(format!(
                    "{} is {} KB, too big to show here",
                    path.display(),
                    bytes.len() / 1024
                )),
                None,
            ),
            Ok(bytes) if bytes.contains(&0) => (
                Opened::Shown(format!("{} is a binary file", path.display())),
                None,
            ),
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => {
                    let file = Self {
                        path: path.to_owned(),
                        disk: text.clone(),
                        stamp: stamp(path),
                    };
                    (Opened::Text(text), Some(file))
                }
                Err(_) => (
                    Opened::Shown(format!("{} is not UTF-8 text", path.display())),
                    None,
                ),
            },
            Err(error) => (
                Opened::Shown(format!("reading {}: {error}", path.display())),
                None,
            ),
        }
    }

    /// Whether `buffer` holds edits the disk does not have.
    pub fn dirty(&self, buffer: &str) -> bool {
        buffer != self.disk
    }

    /// Writes `buffer` to the file and takes it as what the disk holds.
    pub fn save(&mut self, buffer: &str) -> std::io::Result<()> {
        std::fs::write(&self.path, buffer)?;
        self.disk = buffer.to_owned();
        self.stamp = stamp(&self.path);
        Ok(())
    }

    /// Takes `text`, read from the disk, as what it holds: a reload or a discard.
    pub fn take(&mut self, text: String) {
        self.disk = text;
        self.stamp = stamp(&self.path);
    }

    /// How the disk has moved since it was last read or written, given the Editor's text. The
    /// file is read again only when its time or length changed.
    pub fn check(&mut self, buffer: &str) -> DiskChange {
        let now = stamp(&self.path);
        if now.is_some() && now == self.stamp {
            return DiskChange::Same;
        }
        let text = match std::fs::read(&self.path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) if error.kind() == ErrorKind::NotFound => return DiskChange::Gone,
            Err(error) => {
                log::warn!("reading {}: {error}", self.path.display());
                return DiskChange::Same;
            }
        };
        if text == self.disk {
            self.stamp = now;
            DiskChange::Same
        } else if self.dirty(buffer) {
            DiskChange::Conflict(text)
        } else {
            DiskChange::Reload(text)
        }
    }
}

fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened(path: &Path) -> EditorFile {
        match EditorFile::open(path) {
            (Opened::Text(_), Some(file)) => file,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_save_writes_the_text_and_leaves_nothing_dirty() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.rs");
        std::fs::write(&path, "fn a() {}\n").expect("write");
        let mut file = opened(&path);
        assert!(!file.dirty("fn a() {}\n"));
        assert!(file.dirty("fn b() {}\n"));
        file.save("fn b() {}\n").expect("save");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "fn b() {}\n");
        assert!(!file.dirty("fn b() {}\n"));
        assert_eq!(file.check("fn b() {}\n"), DiskChange::Same);
    }

    #[test]
    fn a_change_on_disk_reloads_unless_the_person_edited() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "one\n").expect("write");
        let mut file = opened(&path);
        std::fs::write(&path, "one\ntwo from the agent\n").expect("agent");
        assert_eq!(
            file.check("one\n"),
            DiskChange::Reload("one\ntwo from the agent\n".into())
        );
        assert_eq!(
            file.check("one, edited\n"),
            DiskChange::Conflict("one\ntwo from the agent\n".into())
        );
        file.take("one\ntwo from the agent\n".into());
        assert_eq!(file.check("one\ntwo from the agent\n"), DiskChange::Same);
        std::fs::remove_file(&path).expect("remove");
        assert_eq!(file.check("anything"), DiskChange::Gone);
    }

    #[test]
    fn binary_and_large_files_are_only_named() {
        let dir = tempfile::tempdir().expect("dir");
        let binary = dir.path().join("b.bin");
        std::fs::write(&binary, [0u8, 1, 2]).expect("write");
        assert!(matches!(
            EditorFile::open(&binary),
            (Opened::Shown(_), None)
        ));
        let large = dir.path().join("big.txt");
        std::fs::write(&large, vec![b'a'; MAX_EDITED + 1]).expect("write");
        assert!(matches!(EditorFile::open(&large), (Opened::Shown(_), None)));
    }
}
