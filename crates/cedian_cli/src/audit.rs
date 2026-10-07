//! `.cedian/audit.jsonl`: one row per OMP tool execution start and end
//! (§64 mechanism 4 envelope, ADR-0035 decision 4). `decision_source` says
//! which profile let the call run: `omp` under `policy = "omp"`, `cedian`
//! under the default profile. Append-only; the S3 cedian-gate rows join
//! this file later.

use cedian_omp::{Approvals, RouterEvent};
use serde_json::{json, Value};
use std::io::Write as _;
use std::path::Path;

pub const AUDIT_FILE: &str = ".cedian/audit.jsonl";

pub struct AuditLog {
    file: std::fs::File,
    next_ordinal: u64,
    source: &'static str,
}

impl AuditLog {
    pub fn open(workdir: &Path, approvals: Approvals) -> Result<Self, String> {
        let path = workdir.join(AUDIT_FILE);
        let complete_rows = match std::fs::read_to_string(&path) {
            Ok(text) => text.lines().filter(|l| !l.trim().is_empty()).count() as u64,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => return Err(format!("audit log {}: {e}", path.display())),
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("audit log: {e}"))?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("audit log {}: {e}", path.display()))?;
        Ok(Self {
            file,
            next_ordinal: complete_rows,
            source: match approvals {
                Approvals::Omp => "omp",
                Approvals::Cedian(_) => "cedian",
            },
        })
    }

    /// Append the row for `event` if it is a tool start or end.
    pub fn record(&mut self, event: &RouterEvent) -> Result<(), String> {
        let item = match event {
            RouterEvent::ToolStart {
                tool_call_id,
                tool_name,
                ..
            } => json!({"event": "start", "tool": tool_name, "tool_call_id": tool_call_id}),
            RouterEvent::ToolEnd {
                tool_call_id,
                tool_name,
                is_error,
                ..
            } => json!({
                "event": "end", "tool": tool_name, "tool_call_id": tool_call_id, "is_error": is_error,
            }),
            _ => return Ok(()),
        };
        self.append(item)
    }

    fn append(&mut self, mut item: Value) -> Result<(), String> {
        item["decision_source"] = json!(self.source);
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let row = json!({"timestamp_ms": timestamp_ms, "ordinal": self.next_ordinal, "item": item});
        writeln!(self.file, "{row}").map_err(|e| format!("audit log append: {e}"))?;
        self.next_ordinal += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_carry_envelope_source_and_continue_ordinals() {
        let dir = std::env::temp_dir().join(format!("cedian-audit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let start = RouterEvent::ToolStart {
            tool_call_id: "c1".into(),
            tool_name: "bash".into(),
            args_preview: "touch probe".into(),
        };
        let end = RouterEvent::ToolEnd {
            tool_call_id: "c1".into(),
            tool_name: "bash".into(),
            result_summary: String::new(),
            is_error: false,
        };
        let mut log = AuditLog::open(&dir, Approvals::Omp).unwrap();
        log.record(&start).unwrap();
        log.record(&RouterEvent::Settled).unwrap();
        log.record(&end).unwrap();
        drop(log);
        let mut log = AuditLog::open(&dir, Approvals::Cedian(Default::default())).unwrap();
        log.record(&start).unwrap();
        drop(log);

        let rows: Vec<Value> = std::fs::read_to_string(dir.join(AUDIT_FILE))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(rows.len(), 3, "non-tool events write nothing");
        let ordinals: Vec<_> = rows
            .iter()
            .map(|r| r["ordinal"].as_u64().unwrap())
            .collect();
        assert_eq!(ordinals, [0, 1, 2]);
        assert_eq!(rows[0]["item"]["decision_source"], "omp");
        assert_eq!(rows[1]["item"]["event"], "end");
        assert_eq!(rows[1]["item"]["is_error"], false);
        assert_eq!(rows[2]["item"]["decision_source"], "cedian");
        assert!(rows[0]["timestamp_ms"].as_u64().unwrap() > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
