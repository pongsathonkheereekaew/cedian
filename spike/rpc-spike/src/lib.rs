//! Phase 0.5 spike: minimal host proving `omp --mode rpc-ui` interop.
//!
//! What this proves (see capability table in spike/CAPABILITY_TABLE.md):
//! spawn -> ready -> negotiate v2 -> chunk reassembly -> prompt -> stream ->
//! abort -> session restore -> images -> set_ask_dialog -> host tools/URIs.
//!
//! What this is NOT: the Phase 1 client. Real client lives in
//! `crates/cedian_omp` and MUST reuse upstream `sdk/rust/omp-rpc`
//! (blocking transport + generated `wire.rs`) instead of this hand-rolled
//! decoder. This file stays small on purpose; do not grow it.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value;

const MAX_REASSEMBLED: usize = 64 * 1024 * 1024;

pub struct Spike {
    child: Child,
    stdin_tx: Sender<String>,
    events: Receiver<Value>,
}

impl Spike {
    pub fn spawn(argv: &[&str]) -> Result<Self, String> {
        let mut child = Command::new(argv[0])
            .args(&argv[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let (stdin_tx, stdin_rx) = mpsc::channel::<String>();
        let (ev_tx, events) = mpsc::channel::<Value>();

        thread::spawn(move || {
            let mut stdin = stdin;
            for line in stdin_rx {
                if stdin.write_all(line.as_bytes()).is_err() {
                    break;
                }
                if stdin.write_all(b"\n").is_err() || stdin.flush().is_err() {
                    break;
                }
            }
        });

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            let mut pending: HashMap<String, PendingChunks> = HashMap::new();
            for line in reader.lines().map_while(Result::ok) {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let v: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => {
                        let _ = ev_tx.send(serde_json::json!({"type":"spike_parse_error"}));
                        continue;
                    }
                };
                if v.get("type").and_then(|t| t.as_str()) == Some("rpc_chunk") {
                    match reassemble(&mut pending, &v) {
                        Ok(Some(obj)) => {
                            if ev_tx.send(obj).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = ev_tx
                                .send(serde_json::json!({"type":"spike_chunk_error","error":e}));
                            break;
                        }
                    }
                } else if ev_tx.send(v).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            child,
            stdin_tx,
            events,
        })
    }

    pub fn send(&self, v: &Value) -> Result<(), String> {
        self.stdin_tx
            .send(v.to_string())
            .map_err(|e| format!("stdin closed: {e}"))
    }

    /// Next decoded frame, or None on timeout / EOF.
    pub fn next(&self, timeout: Duration) -> Option<Value> {
        self.events.recv_timeout(timeout).ok()
    }

    /// Wait for `response` with matching id.
    pub fn call(&self, id: &str, cmd: Value, timeout: Duration) -> Result<Value, String> {
        let mut obj = cmd;
        obj["id"] = Value::String(id.to_string());
        self.send(&obj)?;
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(format!("timeout waiting for response {id}"));
            }
            match self.next(left) {
                Some(v)
                    if v.get("type").and_then(|t| t.as_str()) == Some("response")
                        && v.get("id").and_then(|i| i.as_str()) == Some(id) =>
                {
                    return Ok(v);
                }
                Some(_) => continue, // stream frame; caller drains separately
                None => return Err(format!("eof waiting for response {id}")),
            }
        }
    }

    pub fn close(mut self) {
        drop(self.stdin_tx);
        let _ = self.child.wait();
    }
}

struct PendingChunks {
    count: usize,
    parts: Vec<Option<Vec<u8>>>,
    total: usize,
}

fn reassemble(
    pending: &mut HashMap<String, PendingChunks>,
    v: &Value,
) -> Result<Option<Value>, String> {
    let cid = v["chunkId"]
        .as_str()
        .ok_or("chunk without chunkId")?
        .to_string();
    let index = v["index"].as_u64().ok_or("chunk without index")? as usize;
    let count = v["count"].as_u64().ok_or("chunk without count")? as usize;
    let data = v["data"].as_str().ok_or("chunk without data")?;
    let bytes = STANDARD
        .decode(data)
        .map_err(|e| format!("bad base64: {e}"))?;
    let entry = pending.entry(cid).or_insert(PendingChunks {
        count,
        parts: vec![None; count],
        total: 0,
    });
    if entry.count != count || index >= count {
        return Err("interleaved/corrupt chunk sequence".to_string());
    }
    if entry.parts[index].is_none() {
        entry.total += bytes.len();
        if entry.total > MAX_REASSEMBLED {
            return Err("reassembly limit exceeded".to_string());
        }
        entry.parts[index] = Some(bytes);
    }
    if entry.parts.iter().all(|p| p.is_some()) {
        let raw: Vec<u8> = entry
            .parts
            .iter()
            .flat_map(|p| p.as_ref().unwrap().clone())
            .collect();
        let s = String::from_utf8(raw).map_err(|e| format!("bad utf8: {e}"))?;
        let obj: Value = serde_json::from_str(&s).map_err(|e| format!("bad json: {e}"))?;
        Ok(Some(obj))
    } else {
        Ok(None)
    }
}
