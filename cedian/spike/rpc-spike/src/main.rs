//! rpc-spike binary: exercises every Phase 0.5 capability row through the
//! minimal host in lib.rs, serving one host tool + one host URI scheme.
//!
//! Usage:
//!   cargo run -p rpc-spike -- --non-model   # no model calls (safe, cheap)
//!   cargo run -p rpc-spike -- --full        # includes prompt/stream/abort/images
//!
//! Exit 0 + `SPIKE-RESULT: PASS` only when every selected row proves HAVE.

use rpc_spike::Spike;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

const T: Duration = Duration::from_secs(60);

fn main() {
    let full = env::args().any(|a| a == "--full");
    let sess = env::temp_dir().join(format!("spike-rust-{}", std::process::id()));
    let argv_ref: Vec<&str> = vec![
        "omp",
        "--mode",
        "rpc-ui",
        "--session-dir",
        sess.to_str().unwrap(),
    ];
    // NOTE: no --provider/--model: spike uses the ambient default auth
    // (openai-codex subscription here). Do NOT pin a provider that needs
    // an env API key the harness may not have.
    let mut fails: Vec<String> = Vec::new();
    let spike = Spike::spawn(&argv_ref).expect("spawn omp");
    let mut have = |row: &str, ok: bool, detail: &str| {
        println!("{} {row} :: {detail}", if ok { "HAVE" } else { "MISS" });
        if !ok {
            fails.push(row.to_string());
        }
    };

    // ready handshake
    let ready = spike.next(T).expect("ready frame");
    have(
        "ready-handshake",
        ready.get("type").and_then(|t| t.as_str()) == Some("ready"),
        &ready.to_string()[..120.min(ready.to_string().len())],
    );
    let v2 = ready
        .get("supportedProtocolVersions")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().any(|x| x.as_u64() == Some(2)))
        .unwrap_or(false);

    // negotiate v2
    let r = spike.call(
        "n1",
        json!({"type":"negotiate_protocol","protocolVersion":2}),
        T,
    );
    have(
        "protocol-negotiation",
        v2 && r.map(|v| v["success"] == true).unwrap_or(false),
        "v2",
    );

    // request correlation: two overlapping calls, match by id
    spike
        .send(&json!({"id":"c-a","type":"get_session_stats"}))
        .unwrap();
    spike.send(&json!({"id":"c-b","type":"get_state"})).unwrap();
    let mut seen_a = false;
    let mut seen_b = false;
    for _ in 0..200 {
        match spike.next(T) {
            Some(v) if v["type"] == "response" && v["id"] == "c-a" => seen_a = true,
            Some(v) if v["type"] == "response" && v["id"] == "c-b" => seen_b = true,
            Some(_) => continue,
            None => break,
        }
        if seen_a && seen_b {
            break;
        }
    }
    have(
        "request-correlation",
        seen_a && seen_b,
        "c-a + c-b matched by id",
    );

    // chunking: get_available_models overflows 1MiB -> rpc_chunk sequence
    let r = spike
        .call(
            "m1",
            json!({"type":"get_available_models"}),
            Duration::from_secs(120),
        )
        .expect("models");
    let n = r
        .get("data")
        .and_then(|d| d.get("models"))
        .and_then(|m| m.as_array())
        .map(|a| a.len());
    have(
        "frame-chunking",
        r["success"] == true && n.unwrap_or(0) > 0,
        &format!("{n:?} models reassembled"),
    );

    // ask dialog opt-in (MUST precede any ask UI)
    let r = spike
        .call("a1", json!({"type":"set_ask_dialog","enabled":true}), T)
        .expect("ask");
    have(
        "ui-request-response(opt-in)",
        r["success"] == true,
        &r.to_string()[..80.min(r.to_string().len())],
    );

    // host tools registration
    let r = spike
        .call(
            "h1",
            json!({"type":"set_host_tools","tools":[{
                "name":"echo_host","label":"Echo Host",
                "description":"Echo a value back from the host.",
                "parameters":{"type":"object","properties":{"message":{"type":"string"}},
                              "required":["message"],"additionalProperties":false}}]}),
            T,
        )
        .expect("host tools");
    have(
        "host-tools(register)",
        r["success"] == true,
        &r.to_string()[..120.min(r.to_string().len())],
    );

    // host URI schemes registration
    let r = spike
        .call(
            "u1",
            json!({"type":"set_host_uri_schemes","schemes":[{
                "scheme":"spike","description":"spike virtual files",
                "writable":true,"immutable":false}]}),
            T,
        )
        .expect("host uris");
    have(
        "host-uris(register)",
        r["success"] == true,
        &r.to_string()[..120.min(r.to_string().len())],
    );

    // subagent subscription levels accepted
    let r = spike
        .call(
            "s1",
            json!({"type":"set_subagent_subscription","level":"progress"}),
            T,
        )
        .expect("sub");
    have(
        "subagent-subscription",
        r["success"] == true,
        "progress accepted",
    );

    // session restore: fresh dir -> resumed=false; reopen same dir -> no-op
    let r = spike
        .call(
            "o1",
            json!({"type":"open_session","sessionDir":sess.to_str().unwrap()}),
            T,
        )
        .expect("open_session");
    let resumed = r
        .get("data")
        .and_then(|d| d.get("resumed"))
        .and_then(|v| v.as_bool());
    have(
        "session-restore(open)",
        r["success"] == true,
        &format!("resumed={resumed:?}"),
    );

    if full {
        run_model_part(&spike, &mut have);
    }

    spike.close();
    if fails.is_empty() {
        println!("SPIKE-RESULT: PASS");
    } else {
        println!("SPIKE-RESULT: FAIL :: {fails:?}");
        std::process::exit(1);
    }
}

/// Model-backed rows: prompt/stream, images, abort, host-tool + host-uri
/// roundtrips (served inline while waiting for prompt_result).
fn run_model_part(spike: &Spike, have: &mut dyn FnMut(&str, bool, &str)) {
    // P1: short prompt -> stream -> prompt_result + session_settled
    spike.send(&json!({"id":"p1","type":"prompt","message":"Reply with exactly this word and nothing else: spike-pong"})).unwrap();
    let ack = wait_response(spike, "p1");
    let ok_ack = ack.as_ref().map(|v| v["success"] == true).unwrap_or(false);
    let (result, saw_text) = drain_until_result(spike, "p1", Duration::from_secs(180), &|_| {});
    let status = result
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(|s| s.as_str())
        .unwrap_or("?");
    have(
        "prompt-stream-settle",
        ok_ack && status == "completed" && saw_text,
        &format!("status={status} text={saw_text}"),
    );

    // P2: host tool roundtrip (model must call echo_host; serve inline)
    spike
        .send(&json!({"id":"p2","type":"prompt",
            "message":"Call the echo_host tool with message 'ping123', then reply with only the tool result text."}))
        .unwrap();
    let _ = wait_response(spike, "p2");
    let served = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let served2 = served.clone();
    let (result2, _) = drain_until_result(spike, "p2", Duration::from_secs(180), &|o| {
        if o.get("type").and_then(|t| t.as_str()) == Some("host_tool_call") {
            let id = o["id"].as_str().unwrap_or("").to_string();
            spike
                .send(&json!({"type":"host_tool_result","id":id,
                    "result":{"content":[{"type":"text","text":"PONG-spike"}]}}))
                .unwrap();
            served2.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    });
    let status2 = result2
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(|s| s.as_str())
        .unwrap_or("?");
    spike
        .send(&json!({"id":"g2","type":"get_last_assistant_text"}))
        .unwrap();
    let g = wait_response(spike, "g2");
    let txt = g
        .as_ref()
        .and_then(|v| v.get("data"))
        .map(|d| d.to_string())
        .unwrap_or_default();
    let served_tool = served.load(std::sync::atomic::Ordering::SeqCst);
    let roundtrip = served_tool && txt.contains("PONG-spike");
    have(
        "host-tools(roundtrip)",
        roundtrip,
        &format!(
            "served={served_tool} status={status2} text={}",
            &txt[..120.min(txt.len())]
        ),
    );

    // P3: host URI roundtrip via read tool on spike://
    spike
        .send(&json!({"id":"p3","type":"prompt",
            "message":"Read the file at spike://notes/1 with the read tool and reply with only its exact content."}))
        .unwrap();
    let _ = wait_response(spike, "p3");
    let (result3, _) = drain_until_result(spike, "p3", Duration::from_secs(180), &|o| {
        if o.get("type").and_then(|t| t.as_str()) == Some("host_uri_request") {
            let id = o["id"].as_str().unwrap_or("").to_string();
            spike
                .send(&json!({"type":"host_uri_result","id":id,
                    "content":"URI-PONG-spike","contentType":"text/plain"}))
                .unwrap();
        }
    });
    spike
        .send(&json!({"id":"g3","type":"get_last_assistant_text"}))
        .unwrap();
    let g = wait_response(spike, "g3");
    let txt = g
        .as_ref()
        .and_then(|v| v.get("data"))
        .map(|d| d.to_string())
        .unwrap_or_default();
    let ok3 = result3
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(|s| s.as_str())
        == Some("completed")
        && txt.contains("URI-PONG-spike");
    have(
        "host-uris(roundtrip)",
        ok3,
        &format!("text={}", &txt[..120.min(txt.len())]),
    );

    // P4: images
    const PX: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
    spike
        .send(
            &json!({"id":"p4","type":"prompt","message":"Reply with exactly: img-ok",
            "images":[{"type":"image","mimeType":"image/png","data":PX}]}),
        )
        .unwrap();
    let _ = wait_response(spike, "p4");
    let (result4, _) = drain_until_result(spike, "p4", Duration::from_secs(180), &|_| {});
    let ok4 = result4
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(|s| s.as_str())
        == Some("completed");
    have("prompt-images", ok4, "img prompt completed");

    // P5: long bash then abort mid-stream
    spike
        .send(&json!({"id":"p5","type":"prompt",
            "message":"Run `sleep 25` with the bash tool, then reply done."}))
        .unwrap();
    let _ = wait_response(spike, "p5");
    std::thread::sleep(Duration::from_secs(8));
    spike.send(&json!({"id":"ab1","type":"abort"})).unwrap();
    let ab = wait_response(spike, "ab1");
    let (result5, _) = drain_until_result(spike, "p5", Duration::from_secs(120), &|_| {});
    let status5 = result5
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(|s| s.as_str())
        .unwrap_or("?");
    have(
        "cancellation(abort)",
        ab.map(|v| v["success"] == true).unwrap_or(false) && status5 == "aborted",
        &format!("status={status5}"),
    );
}

fn wait_response(spike: &Spike, id: &str) -> Option<Value> {
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return None;
        }
        match spike.next(left) {
            Some(v)
                if v.get("type").and_then(|t| t.as_str()) == Some("response")
                    && v.get("id").and_then(|i| i.as_str()) == Some(id) =>
            {
                return Some(v)
            }
            Some(_) => continue,
            None => return None,
        }
    }
}

/// Drain frames until prompt_result[id]; serve host frames via f.
/// Returns (prompt_result, saw_any_text_delta).
fn drain_until_result(
    spike: &Spike,
    id: &str,
    timeout: Duration,
    f: &dyn Fn(Value),
) -> (Option<Value>, bool) {
    let deadline = std::time::Instant::now() + timeout;
    let mut saw_text = false;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return (None, saw_text);
        }
        match spike.next(left) {
            Some(v) => {
                let t = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if t == "prompt_result" && v.get("id").and_then(|i| i.as_str()) == Some(id) {
                    return (Some(v), saw_text);
                }
                if t == "message_update" {
                    saw_text = true;
                }
                if t == "host_tool_call" || t == "host_uri_request" {
                    f(v);
                }
            }
            None => return (None, saw_text),
        }
    }
}
