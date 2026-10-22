//! A project's blocks and the named layouts that place them (Interface, Blocks): split trees
//! of blocks, the moves the person makes on them, and where a new block goes. Pure data, so
//! the window draws it and the daemon keeps it in `desktop.layout` without either guessing.

mod block;
mod rect;
mod tree;
mod workspace;

pub use block::{Block, BlockId, Kind};
pub use rect::{Rect, leaves, neighbour};
pub use tree::{Dir, MIN_SHARE, Node, Place, Side};
pub use workspace::{Hidden, Layout, Workspace};
