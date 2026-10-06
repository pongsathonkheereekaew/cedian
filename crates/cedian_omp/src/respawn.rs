//! Validated respawn argv (herdr discipline, plan §75 R3).
//!
//! The respawned `omp --mode rpc-ui` command line is constructed from this
//! struct, never string-concatenated: bare binary path, bounded args, no
//! control characters, cwd part of the dedupe identity so two workspaces
//! never share a runtime by accident.

use crate::OmpError;
use std::path::PathBuf;

/// Bounded, validated inputs for spawning (or respawning) the OMP sidecar.
#[derive(Debug, Clone)]
pub struct RespawnRequest {
    /// Absolute path to the bundled `omp` binary.
    pub binary_path: PathBuf,
    /// `--session-dir` for this workspace. Dedupe key with `cwd`.
    pub session_dir: PathBuf,
    /// Workspace root the sidecar runs in.
    pub cwd: PathBuf,
}

const MAX_ARGS: usize = 64;
const MAX_ARG_BYTES: usize = 8 * 1024;

/// Validation outcome: the exact argv to exec, plus the dedupe key.
#[derive(Debug, Clone)]
pub struct RespawnValidation {
    /// Exact argv: `[binary, --mode, rpc-ui, --session-dir, dir, --cwd, dir]`.
    pub argv: Vec<String>,
    /// Dedupe identity: `{binary_path, session_dir, cwd}`.
    pub dedupe_key: String,
}

/// Validate a respawn request into an exact argv, or reject it.
pub fn validate(req: &RespawnRequest) -> Result<RespawnValidation, OmpError> {
    for (label, path) in [
        ("binary_path", &req.binary_path),
        ("session_dir", &req.session_dir),
        ("cwd", &req.cwd),
    ] {
        if !path.is_absolute() {
            return Err(OmpError::InvalidRespawn(format!(
                "{label} must be absolute: {}",
                path.display()
            )));
        }
        let s = path.to_string_lossy();
        if s.len() > MAX_ARG_BYTES || s.chars().any(|c| c.is_control() || c == '\'') {
            return Err(OmpError::InvalidRespawn(format!(
                "{label} rejected: control chars or too long"
            )));
        }
    }
    let argv = vec![
        req.binary_path.to_string_lossy().into_owned(),
        "--mode".to_string(),
        "rpc-ui".to_string(),
        "--session-dir".to_string(),
        req.session_dir.to_string_lossy().into_owned(),
        "--cwd".to_string(),
        req.cwd.to_string_lossy().into_owned(),
    ];
    if argv.len() > MAX_ARGS {
        return Err(OmpError::InvalidRespawn(
            "argv exceeds MAX_ARGS".to_string(),
        ));
    }
    let dedupe_key = format!(
        "{}:{}:{}",
        req.binary_path.display(),
        req.session_dir.display(),
        req.cwd.display()
    );
    Ok(RespawnValidation { argv, dedupe_key })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> RespawnRequest {
        RespawnRequest {
            binary_path: PathBuf::from("/Applications/cedian.app/Contents/Resources/omp"),
            session_dir: PathBuf::from("/tmp/t1"),
            cwd: PathBuf::from("/Users/u/work"),
        }
    }

    #[test]
    fn valid_request_yields_exact_argv() {
        let v = validate(&req()).unwrap();
        assert_eq!(v.argv[0], "/Applications/cedian.app/Contents/Resources/omp");
        assert_eq!(&v.argv[1..3], &["--mode", "rpc-ui"]);
        assert!(v.dedupe_key.contains("/tmp/t1"));
    }

    #[test]
    fn relative_path_rejected() {
        let mut r = req();
        r.binary_path = PathBuf::from("omp");
        assert!(validate(&r).is_err());
    }

    #[test]
    fn control_chars_rejected() {
        let mut r = req();
        r.session_dir = PathBuf::from("/tmp/a\nb");
        assert!(validate(&r).is_err());
    }

    #[test]
    fn distinct_workspaces_distinct_keys() {
        let mut a = req();
        let mut b = req();
        b.cwd = PathBuf::from("/Users/u/other");
        assert_ne!(
            validate(&a).unwrap().dedupe_key,
            validate(&b).unwrap().dedupe_key
        );
        let _ = &mut a;
    }
}
