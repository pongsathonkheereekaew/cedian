//! Message + tool-card view models (plan Phase 2 `streaming markdown`,
//! `tool calls`, `tool outputs`). Render projections over `Thread` events —
//! no markdown parsing here (GPUI renderer tokenizes at paint time).

use cedian_agent::{ThreadEvent, ToolCallStatus};

/// Render role of one message row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    Thinking,
}

/// One rendered row: role + text + streaming flag. Thinking is a SEPARATE row
/// from the answer text (collapsible in the panel, hidden in compact mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageModel {
    pub role: MessageRole,
    pub text: String,
    pub streaming: bool,
}

/// Tool card status (mirrors `ToolCallStatus`, adds display-only `Stale` for
/// Phase 5 review — kept here so cards render it without a second enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCardStatus {
    Running,
    Done,
    Error,
    Interrupted,
}

/// One tool card: name + status + result summary (filled at `ToolEnd` by the
/// Phase 3 registry; Phase 2 shows name + status + args preview).
#[derive(Debug, Clone)]
pub struct ToolCard {
    pub call_id: String,
    pub name: String,
    pub status: ToolCardStatus,
    pub summary: String,
}

impl ToolCard {
    /// New running card.
    pub fn running(call_id: &str, name: &str) -> Self {
        Self {
            call_id: call_id.to_string(),
            name: name.to_string(),
            status: ToolCardStatus::Running,
            summary: String::new(),
        }
    }

    /// Status transition; only `Running` cards move (terminal states stick).
    pub fn set_status(&mut self, status: ToolCardStatus) {
        if self.status == ToolCardStatus::Running {
            self.status = status;
        }
    }
}

impl From<ToolCallStatus> for ToolCardStatus {
    fn from(s: ToolCallStatus) -> Self {
        match s {
            ToolCallStatus::Running => Self::Running,
            ToolCallStatus::Done => Self::Done,
            ToolCallStatus::Error => Self::Error,
            ToolCallStatus::Interrupted => Self::Interrupted,
        }
    }
}

/// Project a thread's events into render rows: user/assistant rows, thinking
/// rows (non-empty only), tool cards, turn markers skipped (status line owns
/// them), queue chips appended as a single system row.
pub fn render_thread(events: &[ThreadEvent]) -> (Vec<MessageModel>, Vec<ToolCard>) {
    let mut messages = Vec::new();
    let mut cards = Vec::new();
    for event in events {
        match event {
            ThreadEvent::User { text } => {
                messages.push(MessageModel {
                    role: MessageRole::User,
                    text: text.clone(),
                    streaming: false,
                });
            }
            ThreadEvent::Assistant {
                text,
                thinking,
                streaming,
                ..
            } => {
                if !thinking.is_empty() {
                    messages.push(MessageModel {
                        role: MessageRole::Thinking,
                        text: thinking.clone(),
                        streaming: *streaming,
                    });
                }
                messages.push(MessageModel {
                    role: MessageRole::Assistant,
                    text: text.clone(),
                    streaming: *streaming,
                });
            }
            ThreadEvent::Tool {
                call_id,
                name,
                status,
            } => {
                let mut card = ToolCard::running(call_id, name);
                card.status = (*status).into();
                cards.push(card);
            }
            ThreadEvent::Queue {
                steering,
                follow_up,
            } => {
                if !steering.is_empty() || !follow_up.is_empty() {
                    messages.push(MessageModel {
                        role: MessageRole::User,
                        text: format!("queued: {} / {}", steering.join(", "), follow_up.join(", ")),
                        streaming: false,
                    });
                }
            }
            ThreadEvent::TurnEnd { .. } => {}
        }
    }
    (messages, cards)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_splits_into_own_row() {
        let events = vec![ThreadEvent::Assistant {
            message_id: "m".to_string(),
            text: "answer".to_string(),
            thinking: "hmm".to_string(),
            streaming: true,
        }];
        let (msgs, _) = render_thread(&events);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, MessageRole::Thinking);
        assert_eq!(msgs[1].role, MessageRole::Assistant);
    }

    #[test]
    fn terminal_card_status_sticks() {
        let mut card = ToolCard::running("c", "read");
        card.set_status(ToolCardStatus::Done);
        card.set_status(ToolCardStatus::Running);
        assert_eq!(card.status, ToolCardStatus::Done);
    }
}
