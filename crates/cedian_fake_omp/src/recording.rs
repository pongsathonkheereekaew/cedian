//! Record lanes (ADR-0058 decision 5): which OMP a lane may record against,
//! what a recording may not carry into a committed fixture, and how many
//! model turns it took.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// The env var naming the OMP a record lane proxies to.
pub const RECORD_OMP_ENV: &str = "CEDIAN_RECORD_OMP";

/// The pinned OMP itself, not the pass's wrapper: what a sandboxed reviewer
/// records against (ADR-0066), since its profile cannot exec the wrapper.
pub const RECORD_OMP_REAL_ENV: &str = "CEDIAN_RECORD_OMP_REAL";

/// The env var naming the app's record lane to arm (one per run).
pub const APP_RECORD_ENV: &str = "CEDIAN_U12_RECORD";

/// Whether the app test `lane` records this run.
pub fn armed(lane: &str) -> bool {
    std::env::var(APP_RECORD_ENV).as_deref() == Ok(lane)
}

/// How long a test waits for OMP: `replay`, or minutes while a real model
/// answers.
pub fn wait_limit(replay: std::time::Duration) -> std::time::Duration {
    if std::env::var_os(APP_RECORD_ENV).is_some() || std::env::var_os("CEDIAN_U4_RECORD").is_some()
    {
        std::time::Duration::from_secs(300)
    } else {
        replay
    }
}

/// Arm an app recording in `sessions`: the pinned OMP behind a
/// [`session_wrapper`] written in `scratch`.
pub fn arm_app(sessions: &Path, scratch: &Path) {
    let omp = pinned_omp().unwrap_or_else(|e| panic!("{e}"));
    let wrapper = session_wrapper(&omp, sessions, &scratch.join("omp-in-temp-store"))
        .unwrap_or_else(|e| panic!("the session wrapper: {e}"));
    crate::arm_record(sessions, &wrapper).unwrap_or_else(|e| panic!("arm record: {e}"));
    // A review spawns the reviewer's own OMP from the same binary, which in a
    // record lane is the test's: it proxies only where a marker says so. Its
    // session dir is `<state>/reviewer/run/session` (one implementer session's
    // reviewer dir), beside the app's session dir this call is given.
    if let Some(state) = sessions.parent() {
        let reviewer = state.join("reviewer/run/session");
        let _ = crate::arm_record(&reviewer, &wrapper);
    }
}

/// The pinned OMP version (`vendor/omp-revision.json`).
pub fn pinned_version() -> String {
    let pin: Value = serde_json::from_str(include_str!("../../../vendor/omp-revision.json"))
        .expect("vendor/omp-revision.json is JSON");
    pin["spikeVerified"]["ompVersion"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The OMP named by [`RECORD_OMP_ENV`], once `--version` shows it is the
/// pin. A recording made on another version would replay as the pin's.
pub fn pinned_omp() -> Result<PathBuf, String> {
    let omp = std::env::var_os(RECORD_OMP_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{RECORD_OMP_ENV}: the pinned omp to record against"))?;
    check_pinned(&omp)?;
    Ok(omp)
}

fn check_pinned(omp: &Path) -> Result<(), String> {
    let out = std::process::Command::new(omp)
        .arg("--version")
        .output()
        .map_err(|e| format!("{}: {e}", omp.display()))?;
    let printed = String::from_utf8_lossy(&out.stdout);
    let found = printed.split_whitespace().next().unwrap_or_default();
    let want = format!("omp/{}", pinned_version());
    if found == want {
        Ok(())
    } else {
        Err(format!(
            "{} is {found:?}, not the pinned {want}: record only against the pin",
            omp.display()
        ))
    }
}

/// What a tool reaching outside the workspace returned, and the arguments
/// that named the place, become in a fixture.
pub const REDACTED_OUTSIDE: &str = "${REDACTED: outside the workspace}";

/// The recorder writes the workspace root as `${CEDIAN_CWD}`, so a tool's
/// arguments name a place outside the workspace when they hold an absolute
/// path, any `~` form or a `..` segment. A harmless system path (`ls /usr`)
/// is redacted too; that costs a fixture nothing. So `2>/dev/null` marks a
/// call outside and its result is redacted on purpose. A Windows drive path
/// (`C:\\x`) counts. `cedian://` URIs pass.
fn names_outside(text: &str, home: Option<&str>) -> bool {
    let home = home
        .map(|h| h.trim_end_matches('/'))
        .filter(|h| h.len() > 1);
    regex::Regex::new(OUTSIDE_PATH)
        .expect("an outside path shape")
        .is_match(text)
        || regex::Regex::new(HOME_PATH)
            .expect("a home path shape")
            .is_match(text)
        || text.contains("/Users/")
        || home.is_some_and(|h| text.contains(h))
}

const OUTSIDE_PATH: &str =
    r#"(?-u)(^|[\s"'=(,;&|<>])(/|[A-Za-z]:\\)|(^|[\s"'=(,;&|<>/])\.\.($|[/\s"',;&|)<>])"#;

const HOME_PATH: &str =
    r#"(?-u)\$\{HOME\}|\$HOME\b|(^|[\s"'=:(])~($|[/\s"';&|)]|[A-Za-z_][\w-]*/)"#;

fn outside_calls(records: &[Value], home: Option<&str>) -> Vec<String> {
    fn walk(v: &Value, home: Option<&str>, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                let call = match (map.get("type"), map.get("toolCallId"), map.get("id")) {
                    (Some(t), Some(Value::String(id)), _) if t == "tool_execution_start" => {
                        Some((id, map.get("args")))
                    }
                    (Some(t), _, Some(Value::String(id))) if t == "toolCall" => {
                        Some((id, map.get("arguments")))
                    }
                    _ => None,
                };
                if let Some((id, Some(args))) = call {
                    if any_string(args, &|s| names_outside(s, home)) && !out.contains(id) {
                        out.push(id.clone());
                    }
                }
                map.values().for_each(|v| walk(v, home, out));
            }
            Value::Array(items) => items.iter().for_each(|v| walk(v, home, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    records.iter().for_each(|r| walk(r, home, &mut out));
    out
}

/// Tests each raw string, so an escaped `\n` or `\t` is real whitespace.
fn any_string(v: &Value, test: &dyn Fn(&str) -> bool) -> bool {
    match v {
        Value::String(s) => test(s),
        Value::Object(map) => map.values().any(|v| any_string(v, test)),
        Value::Array(items) => items.iter().any(|v| any_string(v, test)),
        _ => false,
    }
}

fn redacted_result() -> Value {
    serde_json::json!({"content": [{"type": "text", "text": REDACTED_OUTSIDE}]})
}

fn redact_strings(v: &mut Value, home: Option<&str>) {
    match v {
        Value::String(s) if names_outside(s, home) => *s = REDACTED_OUTSIDE.into(),
        Value::Object(map) => map.values_mut().for_each(|v| redact_strings(v, home)),
        Value::Array(items) => items.iter_mut().for_each(|v| redact_strings(v, home)),
        _ => {}
    }
}

/// Any other frame of an outside call (`tool_stream_update` streams its
/// output) keeps its shape and loses every string.
fn blank_strings(v: &mut Value) {
    match v {
        Value::String(s) => *s = REDACTED_OUTSIDE.into(),
        Value::Object(map) => map.values_mut().for_each(blank_strings),
        Value::Array(items) => items.iter_mut().for_each(blank_strings),
        _ => {}
    }
}

fn redact_outside(v: &mut Value, outside: &[String], home: Option<&str>) {
    let id_of = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
    let is_outside = |id: Option<String>| id.is_some_and(|id| outside.contains(&id));
    match v {
        Value::Object(_) => {
            let kind = v.get("type").and_then(Value::as_str).unwrap_or_default();
            if kind.starts_with("tool_execution_") && is_outside(id_of(v, "toolCallId")) {
                for (key, value) in v.as_object_mut().into_iter().flatten() {
                    match key.as_str() {
                        "args" => redact_strings(value, home),
                        "result" | "partialResult" => *value = redacted_result(),
                        _ => {}
                    }
                }
            } else if kind == "toolCall" && is_outside(id_of(v, "id")) {
                if let Some(args) = v.get_mut("arguments") {
                    redact_strings(args, home);
                }
            } else if v.get("role").and_then(Value::as_str) == Some("toolResult")
                && is_outside(id_of(v, "toolCallId"))
            {
                v["content"] = redacted_result()["content"].clone();
                if let Some(map) = v.as_object_mut() {
                    map.remove("details");
                }
            } else if is_outside(id_of(v, "toolCallId")) {
                for (key, value) in v.as_object_mut().into_iter().flatten() {
                    if !matches!(key.as_str(), "type" | "toolCallId" | "toolName") {
                        blank_strings(value);
                    }
                }
            }
            if let Some(map) = v.as_object_mut() {
                map.values_mut()
                    .for_each(|v| redact_outside(v, outside, home));
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|v| redact_outside(v, outside, home)),
        _ => {}
    }
}

/// An approval prompt names the call it asks about; the app reads only its
/// first line (`Allow tool: <name>`), so the rest goes when it names a place
/// outside the workspace.
fn redact_prompt(v: &mut Value, home: Option<&str>) {
    match v {
        Value::String(s) if names_outside(s, home) => {
            *s = match s.split_once('\n') {
                Some((first, _)) if !names_outside(first, home) => {
                    format!("{first}\n{REDACTED_OUTSIDE}")
                }
                _ => REDACTED_OUTSIDE.into(),
            }
        }
        Value::Object(map) => map.values_mut().for_each(|v| redact_prompt(v, home)),
        Value::Array(items) => items.iter_mut().for_each(|v| redact_prompt(v, home)),
        _ => {}
    }
}

/// What the recorder's path redaction leaves but a fixture must not carry:
/// `get_state`'s system prompt (the person's mandate, prefs and memory) and
/// tool dump, the installed skills in OMP's command lists, what tools that
/// reached outside the workspace read or ran there ([`REDACTED_OUTSIDE`]),
/// and what their approval prompts showed of it,
/// the accumulated message `message_update` repeats beside its delta (the
/// app renders the delta, as OMP's own `delta` filter sends it), and any
/// spelling of the person's home the redaction missed. The person's user
/// name, as a whole word (`ls -la`'s owner column), becomes `${USER}`.
pub fn scrub(recorded: &str, home: Option<&str>, user: Option<&str>) -> String {
    let records: Vec<Option<Value>> = recorded
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).ok())
        .collect();
    let outside = outside_calls(&records.iter().flatten().cloned().collect::<Vec<_>>(), home);
    let scrubbed: String = recorded
        .lines()
        .zip(records)
        .map(|(line, record)| {
            let Some(mut record) = record else {
                return format!("{line}\n");
            };
            let frame = &mut record["frame"];
            redact_outside(frame, &outside, home);
            if frame["type"] == "extension_ui_request" {
                redact_prompt(frame, home);
            }
            if frame["type"] == "message_update" {
                if let Some(message) = frame.get_mut("message").and_then(Value::as_object_mut) {
                    message.insert("content".into(), Value::Array(Vec::new()));
                }
                if let Some(event) = frame
                    .get_mut("assistantMessageEvent")
                    .and_then(Value::as_object_mut)
                {
                    event.remove("partial");
                }
            }
            if frame["command"] == "get_state" {
                if let Some(data) = frame.get_mut("data").and_then(Value::as_object_mut) {
                    data.insert("systemPrompt".into(), "${SCRUBBED}".into());
                    data.insert("dumpTools".into(), Value::Array(Vec::new()));
                }
            }
            if frame["type"] == "available_commands_update" {
                frame["commands"] = Value::Array(Vec::new());
            }
            format!("{record}\n")
        })
        .collect();
    let scrubbed = match home {
        Some(home) if home.len() > 1 => scrubbed.replace(home.trim_end_matches('/'), "${HOME}"),
        _ => scrubbed,
    };
    match user.map(str::trim).filter(|u| u.len() > 1) {
        Some(user) => regex::Regex::new(&format!(r"(?i-u)\b{}\b", regex::escape(user)))
            .expect("an escaped word")
            .replace_all(&scrubbed, "$${USER}")
            .into_owned(),
        None => scrubbed,
    }
}

/// The env var naming the model a recording must have run on: the
/// benchmark's (`script/cedian-bench/run.sh record` sets it).
pub const RECORD_MODEL_ENV: &str = "CEDIAN_RECORD_MODEL";

/// Who recorded: what a fixture must never carry about them.
pub struct Owner {
    pub home: Option<String>,
    pub user: Option<String>,
    pub email: Option<String>,
}

impl Owner {
    /// The person running this process: HOME, USER and `git config
    /// user.email`.
    pub fn current() -> Self {
        let email = std::process::Command::new("git")
            .args(["config", "user.email"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        Owner {
            home: std::env::var("HOME").ok(),
            user: std::env::var("USER").ok(),
            email,
        }
    }
}

/// What in `text` a committed fixture must not carry, named by kind, never
/// by value. Runs on the whole text, so a line that is not JSON, a raw
/// `rpc_chunk` or a tool's output is checked like any frame, and on each JSON
/// line's raw strings, where an escaped `\n` or `\t` is real whitespace.
pub fn leaks(text: &str, owner: &Owner) -> Vec<String> {
    let shapes = [
        (
            r"(?-u)[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+",
            "an email address",
        ),
        (r"(?i-u)\bbearer\s+[A-Za-z0-9._~+/=-]{8,}", "a Bearer token"),
        (r"(?-u)\bsk-[A-Za-z0-9_-]{16,}", "an sk- key"),
        (
            r#"(?i-u)api[_-]?key["']?\s*[:=]\s*["']?[A-Za-z0-9_-]{12,}|\bgh[pousr]_[A-Za-z0-9]{20,}|\bAKIA[0-9A-Z]{16}\b"#,
            "an API key",
        ),
        (
            r"(?-u)\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
            "a JWT",
        ),
        (
            r"(?i-u)(/|\\/|%2f)users(/|\\/|%2f)[a-z0-9._-]",
            "a /Users/<name> path",
        ),
        (HOME_PATH, "a path under HOME outside the workspace"),
    ];
    let mut hits: Vec<String> = shapes
        .iter()
        .filter(|(re, _)| {
            let re = regex::Regex::new(re).expect("a leak shape");
            re.is_match(text)
                || text.lines().any(|line| {
                    serde_json::from_str::<Value>(line)
                        .is_ok_and(|v| any_string(&v, &|s| re.is_match(s)))
                })
        })
        .map(|(_, kind)| kind.to_string())
        .collect();
    let lower = text.to_lowercase();
    let mut has = |value: &Option<String>, kind: &str, word: bool| {
        let Some(v) = value.as_deref().map(str::trim).filter(|v| v.len() > 1) else {
            return;
        };
        let v = v.trim_end_matches('/').to_lowercase();
        let found = if word {
            regex::Regex::new(&format!(r"(?-u)\b{}\b", regex::escape(&v)))
                .is_ok_and(|re| re.is_match(&lower))
        } else {
            lower.contains(&v)
        };
        if found {
            hits.push(kind.to_string());
        }
    };
    has(&owner.email, "the owner's git email", false);
    has(&owner.home, "the HOME value", false);
    has(&owner.user, "the USER value", true);
    hits
}

/// The models OMP reported in a recording, as `provider/id`, in order:
/// `get_state`'s model and every message that names its model.
pub fn observed_models(recorded: &str) -> Vec<String> {
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                let named = match (map.get("provider"), map.get("model"), map.get("id")) {
                    (Some(Value::String(p)), Some(Value::String(m)), _) => Some(format!("{p}/{m}")),
                    _ => None,
                };
                if let Some(Value::Object(model)) = map.get("model") {
                    if let (Some(Value::String(p)), Some(Value::String(id))) =
                        (model.get("provider"), model.get("id"))
                    {
                        let m = format!("{p}/{id}");
                        if !out.contains(&m) {
                            out.push(m);
                        }
                    }
                }
                if let Some(m) = named {
                    if !out.contains(&m) {
                        out.push(m);
                    }
                }
                map.values().for_each(|v| walk(v, out));
            }
            Value::Array(items) => items.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for line in recorded.lines() {
        if let Ok(record) = serde_json::from_str::<Value>(line) {
            walk(&record, &mut out);
        }
    }
    out
}

/// Scrub `recorded` into `fixture` and return its model turns, or refuse
/// without writing: a recording on another model than `model`, or one whose
/// scrubbed text still carries a [`leaks`] hit.
pub fn try_write_fixture(
    recorded: &Path,
    fixture: &Path,
    owner: &Owner,
    model: &str,
) -> Result<usize, String> {
    let text =
        std::fs::read_to_string(recorded).map_err(|e| format!("{}: {e}", recorded.display()))?;
    let observed = observed_models(&text);
    if observed.is_empty() || observed.iter().any(|m| m != model) {
        return Err(format!(
            "{}: OMP ran {observed:?}, the benchmark's model is {model}; not written",
            fixture.display()
        ));
    }
    let scrubbed = scrub(&text, owner.home.as_deref(), owner.user.as_deref());
    let hits = leaks(&scrubbed, owner);
    if !hits.is_empty() {
        return Err(format!(
            "{}: the scrubbed recording still carries {}; not written",
            fixture.display(),
            hits.join(", ")
        ));
    }
    std::fs::write(fixture, scrubbed).map_err(|e| format!("{}: {e}", fixture.display()))?;
    Ok(model_turns(&text))
}

/// Model turns in a recording: the runs OMP started (`agent_start`).
pub fn model_turns(recorded: &str) -> usize {
    recorded
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|r| r["dir"] == "out" && r["frame"]["type"] == "agent_start")
        .count()
}

/// Scrub the recording at `recorded` into `fixture` ([`try_write_fixture`]
/// for the person running this, on [`RECORD_MODEL_ENV`]'s model), and
/// print the model turns it used, the line `script/cedian-bench/run.sh
/// record` counts. A refused recording panics and writes nothing.
pub fn write_fixture(recorded: &Path, fixture: &Path) {
    let model = std::env::var(RECORD_MODEL_ENV)
        .unwrap_or_else(|_| panic!("{RECORD_MODEL_ENV}: the model this recording must run on"));
    write_fixture_on(recorded, fixture, &model);
}

/// [`write_fixture`] for a recording that runs on `model` rather than the
/// pass's: the independent reviewer's (ADR-0066).
pub fn write_fixture_on(recorded: &Path, fixture: &Path, model: &str) {
    let text = std::fs::read_to_string(recorded).unwrap_or_default();
    match try_write_fixture(recorded, fixture, &Owner::current(), model) {
        Ok(turns) => println!(
            "fixture written: {} (model turns: {turns})",
            fixture.display()
        ),
        Err(refused) => {
            println!("fixture refused (model turns: {})", model_turns(&text));
            panic!("{refused}")
        }
    }
}

/// A wrapper that runs `omp` with its sessions in `sessions`: the app opens
/// OMP's own store (`Sessions::OmpDefault`), which a recording must keep
/// out of the person's.
pub fn session_wrapper(omp: &Path, sessions: &Path, at: &Path) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::write(
        at,
        format!(
            "#!/bin/sh\nexec {} \"$@\" --session-dir {}\n",
            sh_quote(omp),
            sh_quote(sessions)
        ),
    )?;
    std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o755))?;
    Ok(at.to_path_buf())
}

fn sh_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_removes_the_persons_setup() {
        let recorded = concat!(
            r#"{"dir":"out","frame":{"type":"response","command":"get_state","data":{"systemPrompt":"my memory","dumpTools":[{"name":"x"}],"sessionId":"s"}}}"#,
            "\n",
            r#"{"dir":"out","frame":{"type":"available_commands_update","commands":[{"name":"my-skill"}]}}"#,
            "\n",
            r#"{"dir":"out","frame":{"type":"tool_execution_end","result":"/Users/me/notes and /Users/me/"}}"#,
            "\n"
        );
        let scrubbed = scrub(recorded, Some("/Users/me/"), None);
        for secret in ["my memory", "\"x\"", "my-skill", "/Users/me"] {
            assert!(!scrubbed.contains(secret), "{secret} in {scrubbed}");
        }
        assert!(scrubbed.contains("\"sessionId\":\"s\""));
        assert!(scrubbed.contains("${HOME}/notes"), "{scrubbed}");
    }

    #[test]
    fn scrub_replaces_the_owners_user_as_a_word() {
        let recorded = concat!(
            r#"{"dir":"out","frame":{"type":"tool_execution_end","result":"drwxr-xr-x 8 me staff 256 main.c\n-rw-r--r-- 1 me staff 9 meme"}}"#,
            "\n"
        );
        let scrubbed = scrub(recorded, None, Some("me"));
        assert!(scrubbed.contains("8 ${USER} staff"), "{scrubbed}");
        assert!(scrubbed.contains("meme"), "{scrubbed}");
        let owner = Owner {
            home: None,
            user: Some("me".into()),
            email: None,
        };
        assert!(leaks(&scrubbed, &owner).is_empty(), "{scrubbed}");
    }

    #[test]
    fn scrub_redacts_tools_that_reach_outside_the_workspace() {
        let lines = [
            r#"{"dir":"out","frame":{"type":"tool_execution_start","toolCallId":"o1","toolName":"read","args":{"path":"${HOME}/.cache/run/brief.md"}}}"#,
            r#"{"dir":"out","frame":{"type":"tool_execution_update","toolCallId":"o1","toolName":"read","args":{"path":"${HOME}/.cache/run/brief.md"},"partialResult":{"content":[{"type":"text","text":"secret plan"}]}}}"#,
            r#"{"dir":"out","frame":{"type":"tool_execution_end","toolCallId":"o1","toolName":"read","result":{"content":[{"type":"text","text":"secret plan"}],"details":{"lines":3}},"isError":false}}"#,
            r#"{"dir":"out","frame":{"type":"tool_execution_start","toolCallId":"o2","toolName":"bash","args":{"command":"cat ~/notes.txt"}}}"#,
            r#"{"dir":"out","frame":{"type":"tool_execution_end","toolCallId":"o2","toolName":"bash","result":{"content":[{"type":"text","text":"secret notes"}]}}}"#,
            r#"{"dir":"out","frame":{"type":"tool_execution_start","toolCallId":"w1","toolName":"read","args":{"path":"${CEDIAN_CWD}/main.c"}}}"#,
            r#"{"dir":"out","frame":{"type":"tool_execution_end","toolCallId":"w1","toolName":"read","result":{"content":[{"type":"text","text":"int x = 42;"}]}}}"#,
            r#"{"dir":"out","frame":{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"reading"},{"type":"toolCall","id":"o1","name":"read","arguments":{"path":"${HOME}/.cache/run/brief.md"}}]}}}"#,
            r#"{"dir":"out","frame":{"type":"message_end","message":{"role":"toolResult","toolCallId":"o1","toolName":"read","content":[{"type":"text","text":"secret plan"}],"details":{"lines":3},"isError":false}}}"#,
            r#"{"dir":"out","frame":{"type":"agent_end","messages":[{"role":"toolResult","toolCallId":"o2","toolName":"bash","content":[{"type":"text","text":"secret notes"}]}]}}"#,
            r#"{"dir":"out","frame":{"type":"extension_ui_request","id":"u1","method":"select","title":"Allow tool: bash\nCommand: cat ~/notes.txt","options":["Approve","Deny"]}}"#,
        ];
        let scrubbed = scrub(&format!("{}\n", lines.join("\n")), None, None);
        for secret in ["secret", "brief.md", "notes.txt", "/.cache/run"] {
            assert!(!scrubbed.contains(secret), "{secret} in {scrubbed}");
        }
        assert!(scrubbed.contains("int x = 42;"), "{scrubbed}");
        assert!(scrubbed.contains("${CEDIAN_CWD}/main.c"), "{scrubbed}");
        assert!(scrubbed.contains(REDACTED_OUTSIDE), "{scrubbed}");
        assert!(
            scrubbed.contains(&format!(r#""Allow tool: bash\n{REDACTED_OUTSIDE}""#)),
            "{scrubbed}"
        );
        assert_eq!(scrubbed.lines().count(), lines.len());
        assert!(leaks(&scrubbed, &owner()).is_empty(), "{scrubbed}");
    }

    #[test]
    fn a_stream_update_of_an_outside_call_is_redacted() {
        let lines = [
            r#"{"dir":"out","frame":{"type":"tool_execution_start","toolCallId":"o1","toolName":"bash","args":{"command":"cat ${HOME}/x"}}}"#,
            r#"{"dir":"out","frame":{"type":"tool_stream_update","toolCallId":"o1","toolName":"bash","update":{"output":"secret plan"},"output":"secret plan"}}"#,
            r#"{"dir":"out","frame":{"type":"tool_execution_start","toolCallId":"w1","toolName":"edit","args":{"path":"notes.txt"}}}"#,
            r#"{"dir":"out","frame":{"type":"tool_stream_update","toolCallId":"w1","toolName":"edit","update":{"files":[{"path":"notes.txt","diff":"+beta"}]}}}"#,
        ];
        let scrubbed = scrub(&format!("{}\n", lines.join("\n")), None, None);
        assert!(!scrubbed.contains("secret"), "{scrubbed}");
        assert!(scrubbed.contains("+beta"), "{scrubbed}");
        assert!(scrubbed.contains(r#""toolCallId":"o1""#), "{scrubbed}");
    }

    #[test]
    fn any_path_outside_the_workspace_marks_the_call() {
        let outside = [
            "cd ~ && cat secret.txt",
            "cd ~",
            "cat ~/secret.txt",
            "cat ~pond/secret.txt",
            "read ${CEDIAN_CWD}/../../.cache/x/secret.md",
            "cd .. && cat .cache/secret.txt",
            "cat ../secret.txt",
            "cat /private/tmp/claude-501/secret.txt",
            "ls /usr/lib",
            "cat \"$HOME\"/secret.txt",
            "echo start\n/private/tmp/x/secret",
            "echo start\n~/bin/x",
            "ls\n../x",
            "echo a;\t/private/tmp/x",
            "type C:\\Users\\name\\x",
        ];
        let inside = [
            "cat notes.txt",
            "${CEDIAN_CWD}/src/main.c",
            "cedian://file/notes.txt",
            "git diff a..b",
            "echo ~5 items",
        ];
        for (n, arg) in outside.iter().chain(&inside).enumerate() {
            let arg = serde_json::to_string(arg).unwrap();
            let recorded = format!(
                "{{\"dir\":\"out\",\"frame\":{{\"type\":\"tool_execution_start\",\"toolCallId\":\"c{n}\",\"toolName\":\"bash\",\"args\":{{\"command\":{arg}}}}}}}\n{{\"dir\":\"out\",\"frame\":{{\"type\":\"tool_execution_end\",\"toolCallId\":\"c{n}\",\"toolName\":\"bash\",\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"result text\"}}]}}}}}}\n"
            );
            let scrubbed = scrub(&recorded, None, None);
            let redacted = !scrubbed.contains("result text");
            assert_eq!(redacted, n < outside.len(), "{arg}: {scrubbed}");
        }
    }

    #[test]
    fn scrub_keeps_only_the_delta_of_a_message_update() {
        let recorded = concat!(
            r#"{"dir":"out","frame":{"type":"message_update","messageId":"m1","message":{"role":"assistant","content":[{"type":"text","text":"the whole long message so far"}],"provider":"p","model":"m","timestamp":0},"assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":" far","partial":{"role":"assistant","content":[{"type":"text","text":"the whole long message so far"}],"timestamp":0}}}}"#,
            "\n"
        );
        let scrubbed = scrub(recorded, None, None);
        assert!(!scrubbed.contains("whole long"), "{scrubbed}");
        let record: Value = serde_json::from_str(scrubbed.trim()).unwrap();
        let event = &record["frame"]["assistantMessageEvent"];
        assert_eq!(event["delta"], " far");
        assert_eq!(record["frame"]["messageId"], "m1");
        assert_eq!(observed_models(&scrubbed), vec!["p/m"]);
        let parsed: omp_rpc::wire::RpcAgentEvent =
            serde_json::from_value(record["frame"].clone()).expect("still an OMP event");
        assert!(matches!(
            parsed,
            omp_rpc::wire::RpcAgentEvent::MessageUpdate(_)
        ));
    }

    #[test]
    fn a_home_path_outside_the_workspace_is_a_leak() {
        let nobody = Owner {
            home: None,
            user: None,
            email: None,
        };
        let kind = "a path under HOME outside the workspace";
        for text in [
            "grep x ${HOME}/.cache",
            "cat $HOME/x",
            "cat ~/notes",
            r#"{"path":"~/x"}"#,
            "cd ~ && cat x",
            r#"{"command":"cd ~"}"#,
            "cat ~pond/x",
            r#"{"command":"echo start\n~/bin/x"}"#,
            r#"{"command":"a\t~"}"#,
        ] {
            assert_eq!(leaks(text, &nobody), vec![kind], "{text}");
        }
        assert!(leaks("read ${CEDIAN_CWD}/main.c and x~/y, ~5 items", &nobody).is_empty());
    }

    fn owner() -> Owner {
        Owner {
            home: Some("/Users/pond".into()),
            user: Some("pond".into()),
            email: Some("owner@example.org".into()),
        }
    }

    #[test]
    fn every_secret_shape_is_named_not_shown() {
        let cases = [
            ("mail me at a.b+c@host.io", "an email address"),
            ("author owner@example.org", "the owner's git email"),
            (
                "Authorization: Bearer abcDEF123456789.xyz",
                "a Bearer token",
            ),
            ("key sk-proj1234567890abcdefXYZ", "an sk- key"),
            (r#""api_key": "A1b2C3d4E5f6G7h8""#, "an API key"),
            ("ghp_abcdefghijklmnopqrstuvwxyz0123", "an API key"),
            (
                "eyJhbGciOiJIUzI1.eyJzdWIiOiIxMjM0.SflKxwRJSMeKKF2Q",
                "a JWT",
            ),
            ("cat /Users/somebody/x", "a /Users/<name> path"),
            (r#"cat \/Users\/somebody"#, "a /Users/<name> path"),
            ("cat %2FUsers%2Fsomebody", "a /Users/<name> path"),
            ("cat /USERS/POND", "a /Users/<name> path"),
            ("in /home-less/Users/pond", "a /Users/<name> path"),
            ("logged in as pond today", "the USER value"),
        ];
        for (text, kind) in cases {
            let hits = leaks(text, &owner());
            assert!(hits.iter().any(|h| h == kind), "{text}: {hits:?}");
            assert!(
                hits.iter().all(|h| !h.contains("pond") && !h.contains('@')),
                "{hits:?}"
            );
        }
        let home_only = Owner {
            home: Some("/var/home/me".into()),
            user: None,
            email: None,
        };
        assert_eq!(
            leaks("at /var/home/me/x", &home_only),
            vec!["the HOME value"]
        );
        assert!(leaks("respond with ${CEDIAN_CWD}/notes and t@t", &owner()).is_empty());
    }

    #[test]
    fn a_leaky_recording_is_never_written() {
        let dir = std::env::temp_dir().join(format!("cedian-record-leak-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let recorded = dir.join("recorded.jsonl");
        let fixture = dir.join("fixture.jsonl");
        let model = r#"{"dir":"out","frame":{"type":"response","command":"get_state","data":{"model":{"id":"m","provider":"p"}}}}"#;
        std::fs::write(
            &recorded,
            format!("{model}\nrpc_chunk raw: Bearer abcdefghijklmnop\n"),
        )
        .unwrap();
        let refused = try_write_fixture(&recorded, &fixture, &owner(), "p/m").unwrap_err();
        assert!(refused.contains("a Bearer token"), "{refused}");
        assert!(!refused.contains("abcdefghijklmnop"), "{refused}");
        assert!(!fixture.exists());

        std::fs::write(&recorded, format!("{model}\n")).unwrap();
        let wrong = try_write_fixture(&recorded, &fixture, &owner(), "p/other").unwrap_err();
        assert!(
            wrong.contains("p/m") && wrong.contains("p/other"),
            "{wrong}"
        );
        assert!(!fixture.exists());
        assert_eq!(
            try_write_fixture(&recorded, &fixture, &owner(), "p/m"),
            Ok(0)
        );
        assert!(fixture.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn observed_models_come_from_state_and_messages() {
        let recorded = concat!(
            r#"{"dir":"out","frame":{"type":"response","command":"get_state","data":{"model":{"id":"m","provider":"p"}}}}"#,
            "\n",
            r#"{"dir":"out","frame":{"type":"message_end","message":{"role":"assistant","provider":"q","model":"n"}}}"#,
            "\n"
        );
        assert_eq!(observed_models(recorded), vec!["p/m", "q/n"]);
        assert!(observed_models("{}").is_empty());
    }

    #[test]
    fn a_wrapper_path_may_hold_a_quote() {
        let dir = std::env::temp_dir().join(format!("cedian-wrap-it's-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let omp = dir.join("o'mp");
        std::fs::write(&omp, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        std::fs::set_permissions(&omp, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        let w = session_wrapper(&omp, &dir.join("s'ess"), &dir.join("w")).unwrap();
        let out = std::process::Command::new(&w)
            .arg("--mode")
            .output()
            .unwrap();
        let printed = String::from_utf8_lossy(&out.stdout);
        assert!(
            printed.contains("--mode") && printed.contains("s'ess"),
            "{printed}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn model_turns_are_omps_runs() {
        let recorded = concat!(
            r#"{"dir":"in","frame":{"type":"prompt","message":"agent_start"}}"#,
            "\n",
            r#"{"dir":"out","frame":{"type":"agent_start"}}"#,
            "\n",
            r#"{"dir":"out","frame":{"type":"turn_start"}}"#,
            "\n",
            r#"{"dir":"out","frame":{"type":"agent_start"}}"#,
            "\n"
        );
        assert_eq!(model_turns(recorded), 2);
    }

    #[test]
    fn only_the_pin_records() {
        let dir = std::env::temp_dir().join(format!("cedian-record-pin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = |name: &str, version: &str| {
            let path = dir.join(name);
            std::fs::write(&path, format!("#!/bin/sh\necho omp/{version}\n")).unwrap();
            std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
            path
        };
        let pinned = fake("pinned", &pinned_version());
        // Not a version at all, only "not the pin": a pin bump cannot make
        // this pass by accident.
        let off_pin = fake("other", &format!("{}-next", pinned_version()));
        assert_eq!(check_pinned(&pinned), Ok(()));
        let refused = check_pinned(&off_pin).unwrap_err();
        assert!(
            refused.contains(&pinned_version()) && refused.contains("-next"),
            "{refused}"
        );
        assert!(check_pinned(&dir.join("missing")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
