//! Every session event carries its own id and its parent's; this reads them from any kind.

use crate::{EventId, SessionEvent};

impl SessionEvent {
    pub fn id(&self) -> &EventId {
        self.ids().0
    }

    /// The previous event in the same session, or `None` for a session's first event.
    pub fn parent_id(&self) -> Option<&EventId> {
        self.ids().1
    }

    fn ids(&self) -> (&EventId, Option<&EventId>) {
        match self {
            SessionEvent::Session { id, parent_id, .. }
            | SessionEvent::User { id, parent_id, .. }
            | SessionEvent::Request { id, parent_id, .. }
            | SessionEvent::Attempt { id, parent_id, .. }
            | SessionEvent::Assistant { id, parent_id, .. }
            | SessionEvent::Keepalive { id, parent_id, .. }
            | SessionEvent::Snapshot { id, parent_id, .. }
            | SessionEvent::Permission { id, parent_id, .. }
            | SessionEvent::ToolIntent { id, parent_id, .. }
            | SessionEvent::ToolResult { id, parent_id, .. }
            | SessionEvent::InputRepaired { id, parent_id, .. }
            | SessionEvent::Hook { id, parent_id, .. }
            | SessionEvent::Reminder { id, parent_id, .. }
            | SessionEvent::Effort { id, parent_id, .. }
            | SessionEvent::TurnEnd { id, parent_id, .. }
            | SessionEvent::CompactRequest { id, parent_id, .. }
            | SessionEvent::Compaction { id, parent_id, .. }
            | SessionEvent::Summary { id, parent_id, .. }
            | SessionEvent::Result { id, parent_id, .. } => (id, parent_id.as_ref()),
        }
    }
}
