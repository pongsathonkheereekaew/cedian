//! A block: one tool open in a layout, of one kind (Interface, Blocks).

use serde::{Deserialize, Serialize};

pub type BlockId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The thread list with search, the project chat row and the groups.
    Threads,
    /// One session of the open thread.
    Agent,
    /// The project chat: the coordinator's thread.
    Chat,
    Terminal,
    Browser,
    Review,
    Editor,
    Files,
    Git,
    Outputs,
    Memory,
    Usage,
    Background,
    /// The open thread's details: its sessions, cost, worktree and instructions.
    Session,
}

impl Kind {
    pub const ALL: [Kind; 14] = [
        Kind::Threads,
        Kind::Agent,
        Kind::Chat,
        Kind::Terminal,
        Kind::Browser,
        Kind::Review,
        Kind::Editor,
        Kind::Files,
        Kind::Git,
        Kind::Outputs,
        Kind::Memory,
        Kind::Usage,
        Kind::Background,
        Kind::Session,
    ];

    /// A kind a layout holds once: opening it again focuses the block it has.
    pub fn one_per_layout(self) -> bool {
        !matches!(self, Kind::Agent | Kind::Terminal | Kind::Browser)
    }

    /// The thread list can be hidden but never closed.
    pub fn closable(self) -> bool {
        self != Kind::Threads
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Threads => "Threads",
            Kind::Agent => "Agent",
            Kind::Chat => "แชทโปรเจกต์",
            Kind::Terminal => "Terminal",
            Kind::Browser => "Browser",
            Kind::Review => "Review",
            Kind::Editor => "Editor",
            Kind::Files => "Files",
            Kind::Git => "Git",
            Kind::Outputs => "Outputs",
            Kind::Memory => "Memory",
            Kind::Usage => "Usage",
            Kind::Background => "Background tasks",
            Kind::Session => "Session",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    pub kind: Kind,
    /// For an agent block, the session of the open thread it shows, counted from 1; none
    /// follows the latest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<u32>,
}
