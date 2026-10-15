//! API keys for the agent's providers, kept in the agent config dir's `.env`
//! (ADR-0082). The agent loads that file at start, so a saved key reaches it
//! on the next start. cedian writes the file itself, mode 0600, and a key
//! never travels over RPC or into a log: [`ApiKey`] has no `Serialize` and
//! its `Debug` shows only the last four characters.

use std::fmt;
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

/// A provider the agent can use with a key of the person's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiKeyProvider {
    /// The agent's provider id.
    pub id: &'static str,
    pub name: &'static str,
    /// The variable the agent reads the key from (its provider catalog's
    /// `env`).
    pub var: &'static str,
}

pub const API_KEY_PROVIDERS: &[ApiKeyProvider] = &[
    ApiKeyProvider {
        id: "anthropic",
        name: "Anthropic",
        var: "ANTHROPIC_API_KEY",
    },
    ApiKeyProvider {
        id: "openai",
        name: "OpenAI",
        var: "OPENAI_API_KEY",
    },
    ApiKeyProvider {
        id: "google",
        name: "Google Gemini",
        var: "GEMINI_API_KEY",
    },
    ApiKeyProvider {
        id: "openrouter",
        name: "OpenRouter",
        var: "OPENROUTER_API_KEY",
    },
];

/// A key as typed, checked to fit one unquoted `.env` value.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn parse(text: &str) -> Result<Self, String> {
        let key = text.trim();
        if key.is_empty() {
            return Err("Enter a key.".to_string());
        }
        let fits = key
            .chars()
            .all(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\'' | '`' | '#' | '\\' | '$'));
        if !fits {
            return Err("A key has no spaces, quotes, #, $ or \\.".to_string());
        }
        Ok(Self(key.to_string()))
    }

    pub fn ending(&self) -> String {
        ending(&self.0)
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApiKey(•••• ends in {})", self.ending())
    }
}

fn ending(key: &str) -> String {
    let start = key
        .char_indices()
        .rev()
        .nth(3)
        .map_or(0, |(index, _)| index);
    key[start..].to_string()
}

/// The agent config dir's `.env`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvFile {
    path: PathBuf,
}

impl EnvFile {
    pub fn in_dir(agent_config_dir: &Path) -> Self {
        Self {
            path: agent_config_dir.join(".env"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The last four characters of the key saved for `var`, if one is.
    pub fn key_ending(&self, var: &str) -> Result<Option<String>, String> {
        Ok(self
            .lines()?
            .iter()
            .rev()
            .find_map(|line| value_of(line, var))
            .map(ending))
    }

    /// Save `key` for `var`, replacing any earlier value and keeping every
    /// other line.
    pub fn save(&self, var: &str, key: &ApiKey) -> Result<(), String> {
        let mut lines = self.lines()?;
        lines.retain(|line| value_of(line, var).is_none());
        lines.push(format!("{var}={}", key.0));
        self.write(&lines)
    }

    /// Remove the key saved for `var`; nothing to remove is not an error.
    pub fn remove(&self, var: &str) -> Result<(), String> {
        let mut lines = self.lines()?;
        let before = lines.len();
        lines.retain(|line| value_of(line, var).is_none());
        if lines.len() == before {
            return Ok(());
        }
        self.write(&lines)
    }

    fn lines(&self) -> Result<Vec<String>, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Ok(text.lines().map(str::to_string).collect()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(format!("cannot read {}: {error}", self.path.display())),
        }
    }

    /// Write through a 0600 sibling renamed over the file, so the keys are
    /// never readable by others, not even for a moment, nor half written.
    fn write(&self, lines: &[String]) -> Result<(), String> {
        let failed =
            |error: std::io::Error| format!("cannot write {}: {error}", self.path.display());
        let dir = self
            .path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", self.path.display()))?;
        std::fs::create_dir_all(dir).map_err(failed)?;
        let staged = dir.join(format!(".env.cedian-{}", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&staged)
            .map_err(failed)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(failed)?;
        let mut text = lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        file.write_all(text.as_bytes()).map_err(failed)?;
        file.sync_all().map_err(failed)?;
        std::fs::rename(&staged, &self.path).map_err(failed)
    }
}

/// The value `line` assigns to `var`, with or without `export`.
fn value_of<'a>(line: &'a str, var: &str) -> Option<&'a str> {
    let line = line.trim_start();
    let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
    let rest = line.strip_prefix(var)?.trim_start();
    Some(rest.strip_prefix('=')?.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("cedian-env-{tag}-{}", std::process::id()));
            std::fs::remove_dir_all(&dir).ok();
            Self(dir)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn a_saved_key_is_a_0600_file_the_agent_reads() {
        let dir = Dir::new("save");
        let env = EnvFile::in_dir(&dir.0.join("agent"));
        assert_eq!(env.key_ending("OPENAI_API_KEY").unwrap(), None);
        env.save(
            "OPENAI_API_KEY",
            &ApiKey::parse(" sk-test-4f2a \n").unwrap(),
        )
        .unwrap();
        assert_eq!(mode(env.path()), 0o600);
        assert_eq!(
            std::fs::read_to_string(env.path()).unwrap(),
            "OPENAI_API_KEY=sk-test-4f2a\n"
        );
        assert_eq!(
            env.key_ending("OPENAI_API_KEY").unwrap().as_deref(),
            Some("4f2a")
        );
    }

    #[test]
    fn replace_keeps_other_lines_and_tightens_a_loose_file() {
        let dir = Dir::new("replace");
        std::fs::create_dir_all(&dir.0).unwrap();
        let env = EnvFile::in_dir(&dir.0);
        std::fs::write(
            env.path(),
            "# mine\nexport ANTHROPIC_API_KEY=old-1111\nOTHER=keep\n",
        )
        .unwrap();
        std::fs::set_permissions(env.path(), std::fs::Permissions::from_mode(0o644)).unwrap();
        env.save("ANTHROPIC_API_KEY", &ApiKey::parse("new-2222").unwrap())
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(env.path()).unwrap(),
            "# mine\nOTHER=keep\nANTHROPIC_API_KEY=new-2222\n"
        );
        assert_eq!(mode(env.path()), 0o600);
        let leftovers: Vec<_> = std::fs::read_dir(&dir.0)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name())
            .collect();
        assert_eq!(leftovers, [std::ffi::OsString::from(".env")]);
    }

    #[test]
    fn remove_drops_only_that_key() {
        let dir = Dir::new("remove");
        let env = EnvFile::in_dir(&dir.0);
        env.save("GEMINI_API_KEY", &ApiKey::parse("g-0000").unwrap())
            .unwrap();
        env.save("OPENROUTER_API_KEY", &ApiKey::parse("o-9999").unwrap())
            .unwrap();
        env.remove("GEMINI_API_KEY").unwrap();
        assert_eq!(
            std::fs::read_to_string(env.path()).unwrap(),
            "OPENROUTER_API_KEY=o-9999\n"
        );
        assert_eq!(env.key_ending("GEMINI_API_KEY").unwrap(), None);
        env.remove("GEMINI_API_KEY").unwrap();
        assert_eq!(mode(env.path()), 0o600);
    }

    #[test]
    fn a_key_never_shows_in_debug_output() {
        let key = ApiKey::parse("sk-secret-value-4f2a").unwrap();
        let shown = format!("{key:?} {:?}", Some(&key));
        assert!(!shown.contains("secret"), "{shown}");
        assert!(shown.contains("4f2a"), "{shown}");
    }

    #[test]
    fn a_key_that_would_break_the_file_is_refused() {
        for bad in [
            "",
            "   ",
            "two words",
            "sk\nINJECTED=1",
            "a#b",
            "\"quoted\"",
            "$HOME",
        ] {
            assert!(ApiKey::parse(bad).is_err(), "{bad:?}");
        }
        assert_eq!(ApiKey::parse("abc").unwrap().ending(), "abc");
    }
}
