//! EventRouter: the single fan-out point for OMP session events.
//!
//! Plan §§5, 71–73: `OMP stdout → background reader → frame decode →
//! EventRouter → channel → batched GPUI updates`. The vendored client's reader
//! thread decodes frames; this router classifies them into [`RouterEvent`]s,
//! keeps the append-only event log (crash-during-stream reconciliation reads
//! this log, plan §74 R3), and forwards to subscribers.
//!
//! Unknown event kinds are tolerated (forwarded as `Unknown`), never fatal —
//! protocol forward-compat. Classification is a pure typed `match`, never
//! Debug-string parsing.

use omp_rpc::wire::{AssistantMessageEvent, RpcAgentEvent, RpcNotification};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{mpsc, Arc};

/// Classified view of one OMP frame for cedian consumers.
#[derive(Debug, Clone)]
pub enum RouterEvent {
    /// Agent turn started.
    AgentStart,
    /// Agent turn ended (terminal or continuation — check flags).
    AgentEnd { yielded: bool, is_terminal: bool },
    /// Streaming text/thinking/toolcall delta with the RPC message id.
    MessageDelta { message_id: String, kind: DeltaKind },
    /// One message completed (full content in payload).
    MessageEnd { message_id: String },
    /// Tool execution lifecycle from OMP's own tools.
    ToolStart {
        tool_call_id: String,
        tool_name: String,
    },
    /// Tool execution finished.
    ToolEnd {
        tool_call_id: String,
        tool_name: String,
    },
    /// A prompt ticket completed.
    PromptResult {
        prompt_id: String,
        status: PromptStatus,
    },
    /// Session quiescent — safe to tear down / recycle.
    Settled,
    /// Queue snapshot changed (steering/follow-up chips).
    Queue {
        steering: Vec<String>,
        follow_up: Vec<String>,
    },
    /// Forward-compat: recognized frame, unmodeled kind — never fatal.
    Unknown { frame_type: String },
}

/// Streaming delta kinds (coarse — Phase 2 panel refines rendering).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaKind {
    Text,
    Thinking,
    ToolCall,
    Other,
}

/// Terminal status of one prompt ticket. Mirrors the wire enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptStatus {
    Completed,
    Aborted,
    Error,
}

impl From<omp_rpc::wire::PromptStatus> for PromptStatus {
    fn from(s: omp_rpc::wire::PromptStatus) -> Self {
        match s {
            omp_rpc::wire::PromptStatus::Completed => Self::Completed,
            omp_rpc::wire::PromptStatus::Aborted => Self::Aborted,
            omp_rpc::wire::PromptStatus::Error => Self::Error,
        }
    }
}

/// Append-only router log entry: sequence number + event.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Monotonic sequence within this router.
    pub seq: u64,
    /// Classified event.
    pub event: RouterEvent,
}

/// Single fan-out point: classifies session events, appends to the log, and
/// broadcasts to subscribers. Reader-thread safe (`parking_lot::Mutex`, no await).
pub struct EventRouter {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    log: Vec<LogEntry>,
    next_seq: u64,
    subscribers: HashMap<u64, mpsc::Sender<RouterEvent>>,
    next_sub: u64,
}

impl EventRouter {
    /// Empty router with no subscribers.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                log: Vec::new(),
                next_seq: 0,
                subscribers: HashMap::new(),
                next_sub: 0,
            })),
        }
    }

    /// Classify one notification frame, append to the log, broadcast. Dead
    /// subscribers are pruned. Never fails on unknown input.
    pub fn dispatch_notification(&self, frame: &RpcNotification) {
        let classified = classify_notification(frame);
        self.push(classified);
    }

    /// Subscribe to classified events. Returns (id, receiver); drop the
    /// receiver to unsubscribe (pruned on next dispatch).
    pub fn subscribe(&self) -> (u64, mpsc::Receiver<RouterEvent>) {
        let (tx, rx) = mpsc::channel();
        let mut inner = self.inner.lock();
        let id = inner.next_sub;
        inner.next_sub += 1;
        inner.subscribers.insert(id, tx);
        (id, rx)
    }

    /// Explicit unsubscribe.
    pub fn unsubscribe(&self, id: u64) {
        self.inner.lock().subscribers.remove(&id);
    }

    /// Full log snapshot for crash reconciliation (plan §74: `tool.started`
    /// without `tool.completed` → `INTERRUPTED`).
    pub fn log_snapshot(&self) -> Vec<LogEntry> {
        self.inner.lock().log.clone()
    }

    /// Tool calls started but not ended in log order — the INTERRUPTED set.
    pub fn interrupted_tool_calls(&self) -> Vec<String> {
        let log = self.inner.lock().log.clone();
        let mut started: Vec<String> = Vec::new();
        let mut ended: Vec<String> = Vec::new();
        for entry in &log {
            match &entry.event {
                RouterEvent::ToolStart { tool_call_id, .. } => started.push(tool_call_id.clone()),
                RouterEvent::ToolEnd { tool_call_id, .. } => ended.push(tool_call_id.clone()),
                _ => {}
            }
        }
        started
            .into_iter()
            .filter(|id| !ended.contains(id))
            .collect()
    }

    fn push(&self, event: RouterEvent) {
        let mut inner = self.inner.lock();
        let seq = inner.next_seq;
        inner.next_seq += 1;
        inner.log.push(LogEntry {
            seq,
            event: event.clone(),
        });
        inner
            .subscribers
            .retain(|_, tx| tx.send(event.clone()).is_ok());
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Pure classifier: notification frame → router event. Typed `match` over the
/// generated unions; unknown variants stay `Unknown`, never panic.
fn classify_notification(frame: &RpcNotification) -> RouterEvent {
    match frame {
        RpcNotification::RpcAgentEvent(event) => classify_agent_event(event),
        RpcNotification::PromptResult(result) => RouterEvent::PromptResult {
            prompt_id: result.id.clone().unwrap_or_default(),
            status: result.status.into(),
        },
        RpcNotification::SessionSettled(_) => RouterEvent::Settled,
        RpcNotification::Unknown(raw) => RouterEvent::Unknown {
            frame_type: raw
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("?")
                .to_string(),
        },
        other => RouterEvent::Unknown {
            frame_type: notification_name(other).to_string(),
        },
    }
}

fn notification_name(frame: &RpcNotification) -> &'static str {
    match frame {
        RpcNotification::Ready(_) => "ready",
        RpcNotification::ExtensionError(_) => "extension_error",
        RpcNotification::ExtensionUiRequest(_) => "extension_ui_request",
        RpcNotification::AvailableCommandsUpdate(_) => "available_commands_update",
        RpcNotification::SubagentLifecycle(_) => "subagent_lifecycle",
        RpcNotification::SubagentProgress(_) => "subagent_progress",
        RpcNotification::SubagentEvent(_) => "subagent_event",
        RpcNotification::CommandOutput(_) => "command_output",
        RpcNotification::SessionInfoUpdate(_) => "session_info_update",
        RpcNotification::ConfigUpdate(_) => "config_update",
        RpcNotification::RpcFrameError(_) => "rpc_frame_error",
        _ => "other",
    }
}

/// Pure classifier: session event → router event.
fn classify_agent_event(event: &RpcAgentEvent) -> RouterEvent {
    match event {
        RpcAgentEvent::AgentStart(_) => RouterEvent::AgentStart,
        RpcAgentEvent::AgentEnd(end) => RouterEvent::AgentEnd {
            yielded: end.yielded.unwrap_or(true),
            is_terminal: end.is_terminal.unwrap_or(true),
        },
        RpcAgentEvent::MessageUpdate(update) => RouterEvent::MessageDelta {
            message_id: update.message_id.clone().unwrap_or_default(),
            kind: match &update.assistant_message_event {
                AssistantMessageEvent::TextStart(_)
                | AssistantMessageEvent::TextDelta(_)
                | AssistantMessageEvent::TextEnd(_) => DeltaKind::Text,
                AssistantMessageEvent::ThinkingStart(_)
                | AssistantMessageEvent::ThinkingDelta(_)
                | AssistantMessageEvent::ThinkingEnd(_) => DeltaKind::Thinking,
                AssistantMessageEvent::ToolcallStart(_)
                | AssistantMessageEvent::ToolcallDelta(_)
                | AssistantMessageEvent::ToolcallEnd(_) => DeltaKind::ToolCall,
                _ => DeltaKind::Other,
            },
        },
        RpcAgentEvent::MessageEnd(end) => RouterEvent::MessageEnd {
            message_id: end.message_id.clone().unwrap_or_default(),
        },
        RpcAgentEvent::ToolExecutionStart(start) => RouterEvent::ToolStart {
            tool_call_id: start.tool_call_id.clone(),
            tool_name: start.tool_name.clone(),
        },
        RpcAgentEvent::ToolExecutionEnd(end) => RouterEvent::ToolEnd {
            tool_call_id: end.tool_call_id.clone(),
            tool_name: end.tool_name.clone(),
        },
        RpcAgentEvent::QueueUpdate(queue) => RouterEvent::Queue {
            steering: queue.steering.clone(),
            follow_up: queue.follow_up.clone(),
        },
        _ => RouterEvent::Unknown {
            frame_type: "agent_event".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omp_rpc::wire::{PromptResultEvent, SessionSettledEvent};

    #[test]
    fn interrupted_set_empty_when_balanced() {
        let r = EventRouter::new();
        assert!(r.interrupted_tool_calls().is_empty());
    }

    #[test]
    fn prompt_result_classified() {
        let r = EventRouter::new();
        let (_id, rx) = r.subscribe();
        r.dispatch_notification(&RpcNotification::PromptResult(PromptResultEvent {
            agent_invoked: true,
            status: omp_rpc::wire::PromptStatus::Aborted,
            session_settled: true,
            id: Some("p1".to_string()),
            error: None,
        }));
        match rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap() {
            RouterEvent::PromptResult { prompt_id, status } => {
                assert_eq!(prompt_id, "p1");
                assert_eq!(status, PromptStatus::Aborted);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn settled_classified() {
        let r = EventRouter::new();
        let (_id, rx) = r.subscribe();
        r.dispatch_notification(&RpcNotification::SessionSettled(SessionSettledEvent {}));
        assert!(matches!(
            rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap(),
            RouterEvent::Settled
        ));
    }

    #[test]
    fn dead_subscriber_pruned() {
        let r = EventRouter::new();
        let (_id, rx) = r.subscribe();
        drop(rx);
        r.dispatch_notification(&RpcNotification::SessionSettled(SessionSettledEvent {}));
        assert_eq!(r.inner.lock().subscribers.len(), 0);
    }
}
