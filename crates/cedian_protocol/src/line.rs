//! One line of `cedia -p --output-format stream-json`, decoded strictly against the schema.

use serde::de::DeserializeOwned;

use crate::{HeadlessResult, SessionEvent, StreamEvent};

// A line lives only while it is folded into blocks, so its size does not add up.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Line {
    /// A session log event: what the model saw or did.
    Event(SessionEvent),
    /// A provider streaming event, shown while a turn is in flight and never logged.
    Stream(StreamEvent),
    /// The run's final result.
    Result(Box<HeadlessResult>),
}

#[derive(Debug)]
pub struct LineError(String);

impl std::fmt::Display for LineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LineError {}

impl Line {
    pub fn parse(text: &str) -> Result<Self, LineError> {
        let mut value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| LineError(format!("not JSON: {e}")))?;
        let kind = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| LineError("no \"type\" field".into()))?
            .to_owned();
        match kind.as_str() {
            "event" => Ok(Self::Event(decode(take(&mut value, "event")?, "event")?)),
            "stream" => Ok(Self::Stream(decode(take(&mut value, "event")?, "stream")?)),
            "result" => Ok(Self::Result(Box::new(decode(value, "result")?))),
            other => Err(LineError(format!("unknown line type {other:?}"))),
        }
    }
}

fn take(value: &mut serde_json::Value, field: &str) -> Result<serde_json::Value, LineError> {
    value
        .get_mut(field)
        .map(serde_json::Value::take)
        .ok_or_else(|| LineError(format!("no {field:?} field")))
}

fn decode<T: DeserializeOwned>(value: serde_json::Value, what: &str) -> Result<T, LineError> {
    serde_json::from_value(value).map_err(|e| LineError(format!("bad {what} line: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_result_naming_its_cache_misses_parses() {
        let text = r#"{"type":"result","subtype":"success","sessionId":"s","numTurns":2,"usage":{"uncached":10,"cacheRead":0,"cacheWrite":0,"output":5},"costUsd":0.01,"cacheHitRatio":0,"cacheMisses":[{"session":"s","request":2,"cause":"idle_over_ttl","tokens":4000}],"result":"done"}"#;
        assert!(
            matches!(Line::parse(text), Ok(Line::Result(_))),
            "{:?}",
            Line::parse(text).err()
        );
    }
}
