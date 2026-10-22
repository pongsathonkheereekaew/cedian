//! cedian's part of `~/.cedia/settings.json`: the `desktop` key. cedia owns the file and the
//! rest of it; cedian reads `desktop` and replaces the file atomically when one of its keys
//! changes, leaving every other key and their order as they were.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use serde_json::{Map, Value};

pub fn settings_path(home: &Path) -> PathBuf {
    home.join("settings.json")
}

/// The whole file, or an empty object when there is none.
fn read_file(path: &Path) -> anyhow::Result<Map<String, Value>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    match serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))? {
        Value::Object(map) => Ok(map),
        _ => bail!("{} is not a JSON object", path.display()),
    }
}

/// The `desktop` object, empty when the file or the key is missing.
pub fn read_desktop(path: &Path) -> anyhow::Result<Value> {
    Ok(match read_file(path)?.remove("desktop") {
        Some(Value::Object(desktop)) => Value::Object(desktop),
        Some(_) => bail!("`desktop` in {} is not an object", path.display()),
        None => Value::Object(Map::new()),
    })
}

/// Sets one key of `desktop`; `null` removes it. Returns the new `desktop`.
pub fn set_desktop_key(path: &Path, key: &str, value: Value) -> anyhow::Result<Value> {
    let mut file = read_file(path)?;
    let desktop = file
        .entry("desktop")
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(desktop) = desktop else {
        bail!("`desktop` in {} is not an object", path.display());
    };
    if value.is_null() {
        desktop.remove(key);
    } else {
        desktop.insert(key.to_owned(), value);
    }
    let desktop = Value::Object(desktop.clone());
    let mut text = serde_json::to_string_pretty(&Value::Object(file))?;
    text.push('\n');
    let dir = path
        .parent()
        .context("the settings file has no directory")?;
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let temp = dir.join(format!(".settings.json.cedian-{}", std::process::id()));
    std::fs::write(&temp, text).with_context(|| format!("writing {}", temp.display()))?;
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&temp, meta.permissions());
    }
    std::fs::rename(&temp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(desktop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_desktop_changes_and_the_rest_keeps_its_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = settings_path(dir.path());
        assert_eq!(read_desktop(&path).expect("missing"), serde_json::json!({}));
        std::fs::write(
            &path,
            r#"{"zdr": true, "roles": {"code": "deepseek"}, "desktop": {"font": "Lilex"}, "allow": []}"#,
        )
        .expect("write");
        let desktop = set_desktop_key(&path, "theme", "light".into()).expect("set");
        assert_eq!(
            desktop,
            serde_json::json!({"font": "Lilex", "theme": "light"})
        );
        let desktop = set_desktop_key(&path, "font", Value::Null).expect("remove");
        assert_eq!(desktop, serde_json::json!({"theme": "light"}));
        let text = std::fs::read_to_string(&path).expect("read");
        let keys: Vec<_> = ["zdr", "roles", "desktop", "allow"]
            .iter()
            .map(|k| text.find(&format!("\"{k}\"")).expect(k))
            .collect();
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "{text}");
        assert!(text.contains("\"code\": \"deepseek\""));
    }
}
