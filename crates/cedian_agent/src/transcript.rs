//! Folds cedia's stream-json lines, in order, into blocks. Works the same on a live run and
//! on a recorded fixture.
//!
//! A run's stream also carries its sub-threads' events. They go to a transcript of their own,
//! keyed by thread id, so the parent shows only the spawn call and its report.

use std::collections::{BTreeMap, HashMap};

use cedian_protocol::{AssistantBlock, EventId, HeadlessResult, Line, SessionEvent, StreamEvent};

use crate::block::{Block, Permission, ToolCall, ToolState};

#[derive(Debug, Default)]
pub struct Transcript {
    blocks: Vec<Block>,
    /// Where the in-flight turn's streamed blocks begin; the assistant event replaces them.
    streaming_from: Option<usize>,
    /// The provider's content index to the streamed block it fills.
    streamed: HashMap<u64, usize>,
    /// This transcript's thread, from its first session event.
    thread: Option<String>,
    subthreads: BTreeMap<String, Transcript>,
    /// Events of a sub-thread's sessions, to the sub-thread they belong to.
    routed: HashMap<EventId, String>,
    /// The sub-thread whose request is streaming now, if it is not this thread's.
    streaming_for: Option<String>,
    /// The first block changed since the last `take_changed_from`.
    changed_from: Option<usize>,
    /// Tokens the latest turn held: its prompt and its answer.
    context: Option<u64>,
    /// The tools the latest `code` session was given, by name.
    tools: Vec<String>,
}

impl Transcript {
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// The first block that changed since the last call, so a view re-lays out from there.
    pub fn take_changed_from(&mut self) -> Option<usize> {
        self.changed_from.take()
    }

    fn changed(&mut self, index: usize) {
        self.changed_from = Some(self.changed_from.map_or(index, |from| from.min(index)));
    }

    fn push(&mut self, block: Block) {
        self.changed(self.blocks.len());
        self.blocks.push(block);
    }

    /// Tokens in the context after the latest turn, for the header's meter.
    pub fn context(&self) -> Option<u64> {
        self.context
    }

    /// The model the thread's writer runs on: its latest `code` session's.
    pub fn model(&self) -> Option<&str> {
        self.blocks.iter().rev().find_map(|block| match block {
            Block::Session { model, role } if role == "code" => Some(model.as_str()),
            _ => None,
        })
    }

    /// The tools the thread's writer was given in its latest session: cedia's own and, named
    /// `mcp__<server>__<tool>`, those of each MCP server that connected for the run.
    pub fn tools(&self) -> &[String] {
        &self.tools
    }

    /// This transcript's thread id, once its first session event has arrived.
    pub fn thread(&self) -> Option<&str> {
        self.thread.as_deref()
    }

    /// Sub-threads started from this one, by thread id.
    pub fn subthreads(&self) -> &BTreeMap<String, Transcript> {
        &self.subthreads
    }

    pub fn apply(&mut self, line: &Line) {
        match line {
            Line::Stream(event) => match &self.streaming_for {
                Some(thread) => {
                    let thread = thread.clone();
                    self.subthread(thread).apply_stream(event);
                }
                None => self.apply_stream(event),
            },
            Line::Event(event) => match self.route(event) {
                Some(thread) => self.subthread(thread).apply(line),
                None => {
                    if matches!(event, SessionEvent::Request { .. }) {
                        self.streaming_for = None;
                    }
                    self.apply_event(event);
                }
            },
            Line::Result(result) => self.push_result(result),
        }
    }

    /// A run's receipt: the stream's last line on a live run, or the `result` event a session
    /// log ends with when the thread is rebuilt from cedia's files.
    fn push_result(&mut self, result: &HeadlessResult) {
        self.push(Block::Result {
            subtype: result.subtype,
            text: result.result.clone(),
            cost_usd: result.cost_usd,
            turns: result.num_turns as u32,
            wall_ms: result.timing.as_ref().map(|t| t.wall_ms),
            evidence: result.evidence.clone(),
        });
    }

    /// The sub-thread an event belongs to, or `None` when it is this thread's.
    fn route(&mut self, event: &SessionEvent) -> Option<String> {
        let thread = match event {
            SessionEvent::Session {
                thread: Some(thread),
                ..
            } if self.thread.as_ref().is_some_and(|own| own != thread) => thread.clone(),
            SessionEvent::Session { thread, .. } => {
                if self.thread.is_none() {
                    self.thread = thread.clone();
                }
                return None;
            }
            _ => self.routed.get(event.parent_id()?)?.clone(),
        };
        self.routed.insert(event.id().clone(), thread.clone());
        if matches!(event, SessionEvent::Request { .. }) {
            self.streaming_for = Some(thread.clone());
        }
        Some(thread)
    }

    fn subthread(&mut self, thread: String) -> &mut Transcript {
        self.subthreads.entry(thread).or_default()
    }

    fn apply_stream(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::Start => {
                self.streaming_from = Some(self.blocks.len());
                self.streamed.clear();
            }
            StreamEvent::TextStart { index } => self.open_streamed(*index, |text| Block::Text {
                text,
                streaming: true,
            }),
            StreamEvent::ThinkingStart { index } => {
                self.open_streamed(*index, |text| Block::Thinking {
                    text,
                    streaming: true,
                })
            }
            StreamEvent::TextDelta { index, delta }
            | StreamEvent::ThinkingDelta { index, delta } => {
                let position = self.streamed.get(&(*index as u64)).copied();
                if let Some(position) = position
                    && let Some(Block::Text { text, .. } | Block::Thinking { text, .. }) =
                        self.blocks.get_mut(position)
                {
                    text.push_str(delta);
                    self.changed(position);
                }
            }
            _ => {}
        }
    }

    fn open_streamed(&mut self, index: f64, block: impl FnOnce(String) -> Block) {
        self.streamed.insert(index as u64, self.blocks.len());
        self.push(block(String::new()));
    }

    fn apply_event(&mut self, event: &SessionEvent) {
        match event {
            SessionEvent::Session {
                model, role, tools, ..
            } => {
                if role == "code" {
                    self.tools = tools.iter().map(|t| t.name.clone()).collect();
                }
                self.push(Block::Session {
                    model: model.clone(),
                    role: role.clone(),
                })
            }
            SessionEvent::User { text, images, .. } => self.push(Block::User {
                text: text.clone(),
                images: images.clone(),
            }),
            SessionEvent::Result { result, .. } => self.push_result(result),
            SessionEvent::Assistant { blocks, usage, .. } => {
                self.context = Some(
                    (usage.uncached + usage.cache_read + usage.cache_write + usage.output) as u64,
                );
                if let Some(start) = self.streaming_from.take() {
                    self.blocks.truncate(start);
                    self.changed(start);
                }
                self.streamed.clear();
                for block in blocks.iter().filter_map(final_block) {
                    self.push(block);
                }
            }
            SessionEvent::Permission {
                call_id,
                decision,
                rule,
                reason,
                ..
            } => {
                if let Some(call) = self.tool_mut(call_id) {
                    call.permission = Some(Permission {
                        allowed: matches!(decision, cedian_protocol::SessionEventDecision::Allow),
                        rule: rule.clone(),
                        reason: reason.clone(),
                    });
                }
            }
            SessionEvent::ToolIntent { call_id, at, .. } => {
                if let Some(call) = self.tool_mut(call_id) {
                    call.state = ToolState::Running;
                    call.started_at = Some(at.clone());
                }
            }
            SessionEvent::ToolResult {
                call_id,
                status,
                output,
                at,
                ..
            } => {
                if let Some(call) = self.tool_mut(call_id) {
                    call.state = ToolState::Done(*status);
                    call.output = Some(output.text.clone());
                    call.is_error = output.is_error;
                    call.exit_code = output.exit_code;
                    call.ms = call
                        .started_at
                        .as_deref()
                        .and_then(|start| millis(start, at));
                }
            }
            _ => {}
        }
    }

    fn tool_mut(&mut self, call_id: &str) -> Option<&mut ToolCall> {
        let index = self
            .blocks
            .iter()
            .rposition(|block| matches!(block, Block::Tool(call) if call.call_id == call_id))?;
        self.changed(index);
        match self.blocks.get_mut(index) {
            Some(Block::Tool(call)) => Some(call),
            _ => None,
        }
    }
}

fn final_block(block: &AssistantBlock) -> Option<Block> {
    match block {
        AssistantBlock::Text { text, .. } if !text.is_empty() => Some(Block::Text {
            text: text.clone(),
            streaming: false,
        }),
        AssistantBlock::Thinking { text, .. } if !text.is_empty() => Some(Block::Thinking {
            text: text.clone(),
            streaming: false,
        }),
        AssistantBlock::ToolUse {
            call_id,
            id,
            name,
            input,
            ..
        } => Some(Block::Tool(ToolCall {
            call_id: call_id.clone().unwrap_or_else(|| id.clone()),
            name: name.clone(),
            input: input.0.clone(),
            permission: None,
            state: ToolState::Requested,
            output: None,
            is_error: false,
            exit_code: None,
            started_at: None,
            ms: None,
        })),
        _ => None,
    }
}

/// Milliseconds between two of cedia's RFC 3339 times.
fn millis(from: &str, to: &str) -> Option<i64> {
    let from = chrono::DateTime::parse_from_rfc3339(from).ok()?;
    let to = chrono::DateTime::parse_from_rfc3339(to).ok()?;
    Some((to - from).num_milliseconds())
}
