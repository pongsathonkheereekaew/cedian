//! `WorkspaceHost`: the editor boundary (plan §12 trait shape).
//!
//! Headless implementation over [`BufferStore`]: selection/active-file are
//! caller-provided (no editor yet — the Zed binding feeds real cursor state),
//! `apply_edit` is the transaction path OMP host tools call, Agent Sync
//! (§13) saves dirty buffers before the turn.
//!
//! Threading: the store lives behind `parking_lot::Mutex`; host-tool handler
//! threads (vendored client) lock briefly per call — no await, no UI thread.

use crate::buffer::{ApplyEditResult, BufferError, BufferStore, TextEdit, Version};
use omp_rpc::{HostTool, HostUri};
use parking_lot::Mutex;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Editor boundary: active file, selection, versioned edits, save, diagnostics.
/// Mirrors plan §12 (`buffer_version` + `apply_edit(path, expected, edit)`);
/// the Zed binding implements this same trait over `clock::Global` + `History`.
pub trait WorkspaceHost: Send + Sync {
    /// File the user is editing, if any.
    fn active_file(&self) -> Option<PathBuf>;
    /// Current selection as `(path, start, end)` byte offsets, if any.
    fn selection(&self) -> Option<(PathBuf, usize, usize)>;
    /// Current buffer version (optimistic-concurrency token).
    fn buffer_version(&self, path: &Path) -> Option<Version>;
    /// Apply one edit transactionally. Version mismatch fails closed.
    fn apply_edit(
        &self,
        path: &Path,
        expected: Version,
        edit: &TextEdit,
    ) -> Result<ApplyEditResult, BufferError>;
    /// Undo the newest edit on one buffer (no-op when clean).
    fn undo(&self, path: &Path) -> Option<Version>;
    /// Read buffer text (agents see buffers, not just the filesystem — §13).
    fn read_buffer(&self, path: &Path) -> Option<String>;
}

/// Headless host: in-memory buffers + caller-set editor chrome.
#[derive(Debug, Default)]
pub struct HostTools {
    store: Mutex<BufferStore>,
    active_file: Mutex<Option<PathBuf>>,
    selection: Mutex<Option<(PathBuf, usize, usize)>>,
}

impl HostTools {
    /// Empty host.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shared handle (host-tool/URI handlers capture this).
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Open a buffer with initial text (test setup / workspace scan later).
    pub fn open(&self, path: &Path, text: &str) {
        self.store.lock().open(path, text);
    }

    /// Set the active file (Zed binding: cursor moves).
    pub fn set_active_file(&self, path: Option<PathBuf>) {
        *self.active_file.lock() = path;
    }

    /// Set the selection (Zed binding: selection changes).
    pub fn set_selection(&self, sel: Option<(PathBuf, usize, usize)>) {
        *self.selection.lock() = sel;
    }

    /// Agent Sync (§13): save every dirty buffer, return saved paths.
    /// Idempotent — a crash between save and prompt loses nothing (saves are
    /// plain version stamps; re-running saves the same content).
    pub fn agent_sync(&self) -> Vec<PathBuf> {
        let mut store = self.store.lock();
        let dirty = store.dirty_buffers();
        for path in &dirty {
            store.mark_saved(path);
        }
        dirty
    }

    /// Build the `cedian_apply_edit` host tool: the OMP-side route calls this
    /// for cedian-relevant paths (plan §10: host tools, never `cedian_edit`).
    /// Args: `{path, expected_version, start, end, replacement}`.
    pub fn apply_edit_tool(self: &Arc<Self>) -> HostTool {
        let host = Arc::clone(self);
        let params: Map<String, Value> = serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "expected_version": {"type": "integer"},
                "start": {"type": "integer"},
                "end": {"type": "integer"},
                "replacement": {"type": "string"}
            },
            "required": ["path", "expected_version", "start", "end", "replacement"],
            "additionalProperties": false
        })
        .as_object()
        .unwrap()
        .clone();
        HostTool::new(
            "cedian_apply_edit",
            "Apply one edit to a cedian workspace buffer transactionally (preferred over filesystem writes for project files).",
            params,
            move |args, _ctx| {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let edit = TextEdit {
                    start: args.get("start").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                    end: args.get("end").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                    replacement: args.get("replacement").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                };
                let expected = Version(args.get("expected_version").and_then(|v| v.as_u64()).unwrap_or(0));
                match host.apply_edit(Path::new(path), expected, &edit) {
                    Ok(result) => Ok(format!("applied, now at version {}", result.new_version.0).into()),
                    Err(e) => Err(format!("{e}").into()),
                }
            },
        )
    }

    /// Build the `cedian` URI scheme: `cedian://buffer/<path>` reads serve
    /// buffer text (writes route through the host tool — OMP `edit` never
    /// targets host URIs, plan §11).
    pub fn cedian_uri_scheme(self: &Arc<Self>) -> HostUri {
        let host = Arc::clone(self);
        HostUri::new("cedian", move |url, _ctx| {
            let uri = crate::parse_cedian_uri(url)
                .map_err(|e| -> omp_rpc::HostUriError { format!("{e}").into() })?;
            match uri.kind.as_str() {
                "buffer" => host
                    .read_buffer(Path::new(&uri.path))
                    .map(|text| text.into())
                    .ok_or_else(|| -> omp_rpc::HostUriError {
                        format!("buffer not open: {}", uri.path).into()
                    }),
                other => Err(format!("unknown cedian:// kind: {other}").into()),
            }
        })
        .expect("cedian scheme is valid")
    }
}

impl WorkspaceHost for HostTools {
    fn active_file(&self) -> Option<PathBuf> {
        self.active_file.lock().clone()
    }

    fn selection(&self) -> Option<(PathBuf, usize, usize)> {
        self.selection.lock().clone()
    }

    fn buffer_version(&self, path: &Path) -> Option<Version> {
        self.store.lock().version(path)
    }

    fn apply_edit(
        &self,
        path: &Path,
        expected: Version,
        edit: &TextEdit,
    ) -> Result<ApplyEditResult, BufferError> {
        self.store.lock().apply_edit(path, expected, edit)
    }

    fn undo(&self, path: &Path) -> Option<Version> {
        self.store.lock().undo(path)
    }

    fn read_buffer(&self, path: &Path) -> Option<String> {
        self.store.lock().read(path).map(|(text, _)| text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_saves_dirty() {
        let h = HostTools::new();
        h.open(Path::new("/a.rs"), "hi");
        h.apply_edit(
            Path::new("/a.rs"),
            Version(0),
            &TextEdit {
                start: 0,
                end: 2,
                replacement: "yo".into(),
            },
        )
        .unwrap();
        let saved = h.agent_sync();
        assert_eq!(saved, vec![PathBuf::from("/a.rs")]);
        assert!(h.agent_sync().is_empty());
    }
}
