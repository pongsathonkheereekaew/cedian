//! `.cedian/audit.jsonl` (§64 mechanism 4 envelope): `kind: "tool"` rows for
//! each OMP tool execution start and end (ADR-0035 decision 4), and
//! `kind: "gate"` rows for each decision cedian's own gate makes (S3 gate
//! item 4): a host-tool call it serves (`allow`) and a dialog headless
//! refuses (`deny`, or `abstain` when nobody could answer). `decision_source`
//! says which profile let a tool run: `omp` under `policy = "omp"`, `cedian`
//! under the default profile. Append-only.

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
                Approvals::Cedian(_) | Approvals::Reviewer => "cedian",
            },
        })
    }

    /// Append the row for `event` if it is a tool start or end.
    pub fn record(&mut self, event: &RouterEvent) -> Result<(), String> {
        let item = match event {
            RouterEvent::ToolStart {
                tool_call_id,
                tool_name,
                args_preview,
            } => {
                let item = json!({"kind": "tool", "event": "start", "tool": tool_name, "tool_call_id": tool_call_id});
                self.append(item, None)?;
                if let Some((host_tool, true)) =
                    cedian_agent_ui::host_device(tool_name, args_preview)
                {
                    let gate = json!({
                        "kind": "gate", "tool": host_tool, "command": tool_call_id,
                        "decision": "allow", "scope": "once",
                    });
                    self.append(gate, None)?;
                }
                return Ok(());
            }
            RouterEvent::ToolEnd {
                tool_call_id,
                tool_name,
                is_error,
                ..
            } => json!({
                "kind": "tool", "event": "end", "tool": tool_name, "tool_call_id": tool_call_id, "is_error": is_error,
            }),
            _ => return Ok(()),
        };
        self.append(item, None)
    }

    /// The gate row for a dialog headless refused, stamped when it replied.
    pub fn refusal(&mut self, refusal: &cedian_omp::Refusal) -> Result<(), String> {
        let item = json!({
            "kind": "gate", "tool": refusal.tool, "command": refusal.label,
            "decision": refusal.decision.as_str(), "scope": "once",
        });
        self.append(item, Some(refusal.at_ms))
    }

    fn append(&mut self, mut item: Value, at_ms: Option<u64>) -> Result<(), String> {
        item["decision_source"] = json!(self.source);
        let timestamp_ms = at_ms.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        });
        let row = json!({"timestamp_ms": timestamp_ms, "ordinal": self.next_ordinal, "item": item});
        writeln!(self.file, "{row}").map_err(|e| format!("audit log append: {e}"))?;
        self.next_ordinal += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read the file back: every line parses and ordinals run 0, 1, 2, ….
    fn replay(dir: &Path) -> Vec<Value> {
        let rows: Vec<Value> = std::fs::read_to_string(dir.join(AUDIT_FILE))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row["ordinal"], i as u64, "ordinals are contiguous");
        }
        rows
    }

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

        let rows = replay(&dir);
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

    #[test]
    fn gate_rows_for_a_host_tool_call_and_a_refused_dialog() {
        let dir = std::env::temp_dir().join(format!("cedian-audit-gate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut log = AuditLog::open(&dir, Approvals::Cedian(Default::default())).unwrap();
        log.record(&RouterEvent::ToolStart {
            tool_call_id: "c1".into(),
            tool_name: "write".into(),
            args_preview: "xd://cedian_apply_edit".into(),
        })
        .unwrap();
        log.refusal(&cedian_omp::Refusal {
            label: "Allow tool: bash — Command: rm -rf x".into(),
            tool: Some("bash".into()),
            decision: cedian_omp::GateDecision::Deny,
            at_ms: 42,
        })
        .unwrap();
        log.refusal(&cedian_omp::Refusal {
            label: "Name?".into(),
            tool: None,
            decision: cedian_omp::GateDecision::Abstain,
            at_ms: 43,
        })
        .unwrap();
        drop(log);

        let rows = replay(&dir);
        let gates: Vec<&Value> = rows
            .iter()
            .filter(|r| r["item"]["kind"] == "gate")
            .collect();
        assert_eq!(rows[0]["item"]["kind"], "tool");
        assert_eq!(gates.len(), 3, "{rows:?}");
        assert_eq!(gates[0]["item"]["tool"], "cedian_apply_edit");
        assert_eq!(
            gates[0]["item"]["decision"], "allow",
            "cedian served its host tool"
        );
        assert_eq!(gates[0]["item"]["scope"], "once");
        assert_eq!(gates[1]["item"]["tool"], "bash");
        assert_eq!(
            gates[1]["item"]["command"],
            "Allow tool: bash — Command: rm -rf x"
        );
        assert_eq!(gates[1]["item"]["decision"], "deny");
        assert_eq!(
            gates[1]["timestamp_ms"], 42,
            "stamped when headless replied"
        );
        assert_eq!(gates[2]["item"]["decision"], "abstain");
        assert_eq!(gates[2]["item"]["tool"], Value::Null);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
