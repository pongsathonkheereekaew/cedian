//! A project's blocks: its named layouts, the current one, the parked blocks, and the thread
//! list while hidden (Interface, Blocks). The daemon keeps one per project in
//! `desktop.layout`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Block, BlockId, Dir, Kind, Node, Rect, Side, leaves, tree::Place};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    /// None when the person closed every block.
    pub tree: Option<Node>,
}

/// The thread list while hidden, to put it back exactly where it was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hidden {
    pub block: BlockId,
    pub layout: usize,
    /// The tree with the list, as it was.
    pub with: Option<Node>,
    /// The tree just after hiding it: when the tree still is this, `with` comes back whole.
    pub without: Option<Node>,
    pub place: Place,
    /// Its share of the whole width when it ran down the layout's left edge, where it goes
    /// back to when the tree changed.
    #[serde(default)]
    pub left_edge: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub layouts: Vec<Layout>,
    pub current: usize,
    pub blocks: BTreeMap<BlockId, Block>,
    /// Out of every tree but not closed, in the order they were parked.
    #[serde(default)]
    pub parked: Vec<BlockId>,
    #[serde(default)]
    pub hidden: Option<Hidden>,
    /// Out of the tree, floating over it as picture in picture.
    #[serde(default)]
    pub floating: Option<BlockId>,
    pub next: BlockId,
}

/// The thread list's share of a row when a layout without one gets it.
const THREADS_SHARE: f32 = 0.2;

impl Default for Workspace {
    /// โฟกัส, เขียนโค้ด, พรีวิว and รีวิว, as the spec starts a project.
    fn default() -> Self {
        let mut ws = Workspace {
            layouts: Vec::new(),
            current: 0,
            blocks: BTreeMap::new(),
            parked: Vec::new(),
            hidden: None,
            floating: None,
            next: 1,
        };
        let mut b = |kind| Node::leaf(ws_new(&mut ws.blocks, &mut ws.next, kind));
        let focus = Node::split(
            Dir::Row,
            vec![
                (1., b(Kind::Threads)),
                (2.6, b(Kind::Agent)),
                (1.6, tabs(vec![b(Kind::Review), b(Kind::Session)])),
            ],
        );
        let code = Node::split(
            Dir::Row,
            vec![
                (0.9, b(Kind::Threads)),
                (2.2, b(Kind::Agent)),
                (
                    1.9,
                    Node::split(
                        Dir::Col,
                        vec![
                            (1.2, tabs(vec![b(Kind::Editor), b(Kind::Review)])),
                            (1., b(Kind::Terminal)),
                        ],
                    ),
                ),
            ],
        );
        let preview = Node::split(
            Dir::Row,
            vec![
                (0.9, b(Kind::Threads)),
                (
                    1.9,
                    Node::split(
                        Dir::Col,
                        vec![(1.7, b(Kind::Agent)), (1., b(Kind::Terminal))],
                    ),
                ),
                // The browser comes in phase C; until then the preview reads the files.
                (1.6, tabs(vec![b(Kind::Files), b(Kind::Editor)])),
            ],
        );
        let review = Node::split(
            Dir::Row,
            vec![
                (0.9, b(Kind::Threads)),
                (2.4, b(Kind::Review)),
                (
                    1.4,
                    Node::split(
                        Dir::Col,
                        vec![(1.5, b(Kind::Agent)), (1., b(Kind::Session))],
                    ),
                ),
            ],
        );
        ws.layouts = [
            ("โฟกัส", focus),
            ("เขียนโค้ด", code),
            ("พรีวิว", preview),
            ("รีวิว", review),
        ]
        .into_iter()
        .map(|(name, tree)| Layout {
            name: name.to_owned(),
            tree: Some(tree),
        })
        .collect();
        ws
    }
}

fn ws_new(blocks: &mut BTreeMap<BlockId, Block>, next: &mut BlockId, kind: Kind) -> BlockId {
    let id = *next;
    *next += 1;
    blocks.insert(
        id,
        Block {
            id,
            kind,
            session: None,
        },
    );
    id
}

/// The share of the whole width of the leaf holding `id` when it is alone (no tabs) and runs
/// down the left edge: the first child of rows all the way from the root.
fn left_edge_share(node: &Node, id: BlockId) -> Option<f32> {
    match node {
        Node::Leaf { tabs, .. } => (tabs.as_slice() == [id]).then_some(1.),
        Node::Split {
            dir: Dir::Row,
            sizes,
            kids,
        } => {
            let total: f32 = sizes.iter().sum();
            Some(sizes.first()? / total * left_edge_share(kids.first()?, id)?)
        }
        Node::Split { .. } => None,
    }
}

fn tabs(leaves: Vec<Node>) -> Node {
    let tabs = leaves.into_iter().flat_map(|l| l.blocks()).collect();
    Node::Leaf { tabs, active: 0 }
}

impl Workspace {
    /// A workspace of one block, `kind`, in a layout named after it: what a block dragged out
    /// of the window opens with in its new window. It has no thread list until a layout switch
    /// asks for one.
    pub fn alone(kind: Kind) -> Self {
        let mut ws = Workspace {
            layouts: Vec::new(),
            current: 0,
            blocks: BTreeMap::new(),
            parked: Vec::new(),
            hidden: None,
            floating: None,
            next: 1,
        };
        let id = ws.new_block(kind);
        ws.layouts.push(Layout {
            name: kind.name().into(),
            tree: Some(Node::leaf(id)),
        });
        ws
    }

    pub fn tree(&self) -> Option<&Node> {
        self.layouts.get(self.current)?.tree.as_ref()
    }

    fn tree_mut(&mut self) -> &mut Option<Node> {
        if self.layouts.is_empty() {
            self.layouts.push(Layout {
                name: "โฟกัส".to_owned(),
                tree: None,
            });
        }
        self.current = self.current.min(self.layouts.len() - 1);
        &mut self.layouts[self.current].tree
    }

    pub fn kind(&self, id: BlockId) -> Option<Kind> {
        self.blocks.get(&id).map(|b| b.kind)
    }

    /// The current layout's block of `kind`, the shown one first.
    pub fn find(&self, kind: Kind) -> Option<BlockId> {
        let tree = self.tree()?;
        let is = |id: &BlockId| self.kind(*id) == Some(kind);
        tree.shown()
            .into_iter()
            .find(is)
            .or_else(|| tree.blocks().into_iter().find(is))
    }

    pub fn new_block(&mut self, kind: Kind) -> BlockId {
        ws_new(&mut self.blocks, &mut self.next, kind)
    }

    /// Opens `kind` in the current layout: a kind held once and already there is shown;
    /// otherwise a new block goes into the largest leaf, split along its long side, or into
    /// `target`'s tabs. Returns the block.
    pub fn open(&mut self, kind: Kind, area: Rect, tab_of: Option<BlockId>) -> BlockId {
        if kind.one_per_layout()
            && let Some(id) = self.find(kind)
        {
            if let Some(tree) = self.tree_mut() {
                tree.activate(id);
            }
            return id;
        }
        if kind.one_per_layout() {
            if let Some(id) = self.floating.filter(|id| self.kind(*id) == Some(kind)) {
                return id;
            }
            if let Some(id) = self
                .parked
                .iter()
                .copied()
                .find(|id| self.kind(*id) == Some(kind))
            {
                self.unpark(id, tab_of.map(|t| (t, Side::Center)), area);
                return id;
            }
        }
        if kind == Kind::Threads
            && let Some(id) = self.hidden.as_ref().map(|h| h.block)
        {
            self.show_threads();
            return id;
        }
        let id = self.new_block(kind);
        match tab_of {
            Some(target) if self.tree().is_some_and(|t| t.contains(target)) => {
                if let Some(tree) = self.tree_mut() {
                    tree.insert(target, id, Side::Center);
                }
            }
            _ => self.place(id, area),
        }
        id
    }

    /// Puts `id` into the largest leaf, split along its long side.
    pub fn place(&mut self, id: BlockId, area: Rect) {
        let tree = self.tree_mut();
        match tree {
            None => *tree = Some(Node::leaf(id)),
            Some(node) => {
                let largest = leaves(node, area, 0.)
                    .into_iter()
                    .max_by(|a, b| a.1.area().total_cmp(&b.1.area()));
                if let Some((target, rect)) = largest {
                    let side = if rect.w >= rect.h {
                        Side::Right
                    } else {
                        Side::Down
                    };
                    node.insert(target, id, side);
                }
            }
        }
    }

    /// Takes `id` out of the tree and forgets it. The thread list cannot be closed.
    pub fn close(&mut self, id: BlockId) -> bool {
        if !self.kind(id).is_some_and(Kind::closable) {
            return false;
        }
        if self.floating == Some(id) {
            self.floating = None;
        }
        let tree = self.tree_mut();
        *tree = tree.take().and_then(|t| t.remove(id));
        self.parked.retain(|p| *p != id);
        if !self
            .layouts
            .iter()
            .any(|l| l.tree.as_ref().is_some_and(|t| t.contains(id)))
        {
            self.blocks.remove(&id);
        }
        true
    }

    /// Takes `id` out of the tree without closing it.
    pub fn park(&mut self, id: BlockId) {
        let tree = self.tree_mut();
        if !tree.as_ref().is_some_and(|t| t.contains(id)) {
            return;
        }
        *tree = tree.take().and_then(|t| t.remove(id));
        self.parked.push(id);
    }

    /// Floats `id` over the tree; the block floating before goes back by the placing rule.
    pub fn float(&mut self, id: BlockId, area: Rect) {
        if self.floating == Some(id) || !self.tree().is_some_and(|t| t.contains(id)) {
            return;
        }
        self.land(area);
        let tree = self.tree_mut();
        *tree = tree.take().and_then(|t| t.remove(id));
        self.floating = Some(id);
    }

    /// Puts the floating block back into the tree by the placing rule.
    pub fn land(&mut self, area: Rect) {
        if let Some(id) = self.floating.take() {
            self.place(id, area);
        }
    }

    /// Brings a parked block back against `target`, or by the placing rule.
    pub fn unpark(&mut self, id: BlockId, target: Option<(BlockId, Side)>, area: Rect) {
        let Some(n) = self.parked.iter().position(|p| *p == id) else {
            return;
        };
        self.parked.remove(n);
        self.put(id, target, area);
    }

    /// Moves `id` against `target`. A block dropped on itself stays.
    pub fn move_block(&mut self, id: BlockId, target: BlockId, side: Side) -> bool {
        if id == target {
            return false;
        }
        let tree = self.tree_mut();
        let Some(node) = tree.take() else {
            return false;
        };
        if !node.contains(target) || !node.contains(id) {
            *tree = Some(node);
            return false;
        }
        let mut node = node.remove(id).unwrap_or_else(|| Node::leaf(target));
        node.insert(target, id, side);
        *tree = Some(node);
        true
    }

    fn put(&mut self, id: BlockId, target: Option<(BlockId, Side)>, area: Rect) {
        match target {
            Some((target, side)) if self.tree().is_some_and(|t| t.contains(target)) => {
                if let Some(tree) = self.tree_mut() {
                    tree.insert(target, id, side);
                }
            }
            _ => self.place(id, area),
        }
    }

    /// Hides the thread list, keeping where it was. False when it is not shown.
    pub fn hide_threads(&mut self) -> bool {
        let Some(block) = self.find(Kind::Threads) else {
            return false;
        };
        let layout = self.current;
        let tree = self.tree_mut();
        let Some(with) = tree.clone() else {
            return false;
        };
        let Some(place) = with.place_of(block).map(|mut place| {
            // In a leaf of tabs, its neighbour is a tab beside it.
            if let Some((tabs, _)) = with.tabs_of(block)
                && let Some(other) = tabs.iter().find(|t| **t != block)
            {
                place.neighbour = Some((*other, Side::Center));
            }
            place
        }) else {
            return false;
        };
        let left_edge = left_edge_share(&with, block);
        *tree = with.clone().remove(block);
        let without = tree.clone();
        self.hidden = Some(Hidden {
            block,
            layout,
            with: Some(with),
            without,
            place,
            left_edge,
        });
        true
    }

    /// Shows the thread list again: the tree as it was when nothing changed meanwhile, else
    /// beside the same neighbour at the same share, else on the left.
    pub fn show_threads(&mut self) {
        let Some(hidden) = self.hidden.take() else {
            return;
        };
        let same_layout = hidden.layout == self.current;
        let tree = self.tree_mut();
        if same_layout && *tree == hidden.without {
            *tree = hidden.with;
            return;
        }
        let block = hidden.block;
        if let Some(share) = hidden.left_edge {
            return self.put_left(block, share);
        }
        match (tree.as_mut(), hidden.place.neighbour) {
            (Some(node), Some((neighbour, side))) if node.contains(neighbour) => {
                node.insert(neighbour, block, side);
                if side != Side::Center {
                    node.set_share(block, hidden.place.share);
                }
            }
            _ => self.put_left(block, hidden.place.share),
        }
    }

    /// Puts `id` on the left of the whole tree at `share` of the width.
    fn put_left(&mut self, id: BlockId, share: f32) {
        let tree = self.tree_mut();
        *tree = Some(match tree.take() {
            None => Node::leaf(id),
            Some(node) => Node::split(Dir::Row, vec![(share, Node::leaf(id)), (1. - share, node)]),
        });
    }

    /// Switches to layout `n`; one without a thread list gets it on the left.
    pub fn switch(&mut self, n: usize) -> bool {
        if n >= self.layouts.len() {
            return false;
        }
        if self
            .hidden
            .as_ref()
            .is_some_and(|h| h.layout == self.current)
            && n != self.current
        {
            // The list stays hidden only in the layout it was hidden in.
            self.show_threads();
        }
        self.current = n;
        if self.find(Kind::Threads).is_none() && self.hidden.is_none() {
            let id = self.new_block(Kind::Threads);
            self.put_left(id, THREADS_SHARE);
        }
        true
    }

    /// Saves the current tree as a new layout named `name`, its blocks copied.
    pub fn save_as(&mut self, name: &str) -> usize {
        let tree = self.tree().cloned().map(|tree| self.copy_tree(&tree));
        self.layouts.push(Layout {
            name: name.to_owned(),
            tree,
        });
        self.layouts.len() - 1
    }

    fn copy_tree(&mut self, node: &Node) -> Node {
        match node {
            Node::Leaf { tabs, active } => Node::Leaf {
                tabs: tabs
                    .iter()
                    .map(|id| {
                        let kind = self.kind(*id).unwrap_or(Kind::Agent);
                        self.new_block(kind)
                    })
                    .collect(),
                active: *active,
            },
            Node::Split { dir, sizes, kids } => Node::Split {
                dir: *dir,
                sizes: sizes.clone(),
                kids: kids.iter().map(|k| self.copy_tree(k)).collect(),
            },
        }
    }

    /// Every block of every tree has an entry, and only those and the parked ones do.
    pub fn tidy(&mut self) {
        let mut live: Vec<BlockId> = self
            .layouts
            .iter()
            .filter_map(|l| l.tree.as_ref())
            .flat_map(Node::blocks)
            .chain(self.parked.iter().copied())
            .chain(self.hidden.as_ref().map(|h| h.block))
            .chain(self.floating)
            .collect();
        live.sort_unstable();
        self.blocks.retain(|id, _| live.binary_search(id).is_ok());
        for id in live {
            // A tree that names a block nobody kept shows an agent there.
            self.blocks.entry(id).or_insert(Block {
                id,
                kind: Kind::Agent,
                session: None,
            });
            self.next = self.next.max(id + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect {
        x: 0.,
        y: 0.,
        w: 1400.,
        h: 800.,
    };

    #[test]
    fn a_block_alone_has_one_layout_and_no_thread_list() {
        let ws = Workspace::alone(Kind::Terminal);
        assert_eq!(ws.layouts.len(), 1);
        let id = ws.find(Kind::Terminal).expect("the terminal");
        assert_eq!(ws.tree(), Some(&Node::leaf(id)));
        assert!(ws.find(Kind::Threads).is_none());
    }

    fn threads_rect(ws: &Workspace) -> Rect {
        let id = ws.find(Kind::Threads).expect("threads shown");
        leaves(ws.tree().expect("tree"), AREA, 6.)
            .into_iter()
            .find(|(b, _)| *b == id)
            .map(|(_, r)| r)
            .expect("rect")
    }

    #[test]
    fn hiding_and_showing_threads_never_shrinks_or_moves_it() {
        let mut ws = Workspace::default();
        ws.switch(1);
        let before = threads_rect(&ws);
        for _ in 0..7 {
            assert!(ws.hide_threads());
            assert!(ws.find(Kind::Threads).is_none());
            ws.show_threads();
            assert_eq!(threads_rect(&ws), before);
        }
    }

    #[test]
    fn threads_come_back_beside_the_same_neighbour_after_a_change() {
        let mut ws = Workspace::default();
        ws.switch(1);
        let before = threads_rect(&ws);
        for _ in 0..7 {
            ws.hide_threads();
            // The person opens a terminal while the list is hidden.
            let term = ws.open(Kind::Terminal, AREA, None);
            ws.show_threads();
            let after = threads_rect(&ws);
            assert_eq!(after.x, before.x);
            assert!((after.w - before.w).abs() < 0.5, "{after:?} vs {before:?}");
            ws.close(term);
        }
    }

    #[test]
    fn threads_hidden_from_tabs_come_back_as_a_tab() {
        let mut ws = Workspace::default();
        let threads = ws.find(Kind::Threads).expect("threads");
        let chat = ws.open(Kind::Chat, AREA, Some(threads));
        ws.hide_threads();
        ws.open(Kind::Terminal, AREA, None);
        ws.show_threads();
        let threads = ws.find(Kind::Threads).expect("threads");
        let tree = ws.tree().expect("tree");
        assert_eq!(
            tree.tabs_of(threads).map(|t| t.0.to_vec()),
            Some(vec![chat, threads])
        );
    }

    #[test]
    fn the_thread_list_cannot_be_closed() {
        let mut ws = Workspace::default();
        let threads = ws.find(Kind::Threads).expect("threads");
        assert!(!ws.close(threads));
        assert!(ws.tree().expect("tree").contains(threads));
    }

    #[test]
    fn a_new_block_goes_into_the_largest_leaf_along_its_long_side() {
        let mut ws = Workspace::default();
        ws.switch(1);
        let agent = ws.find(Kind::Agent).expect("agent");
        let browser = ws.open(Kind::Browser, AREA, None);
        let tree = ws.tree().expect("tree");
        let path = tree.path_to(browser).expect("placed");
        assert_eq!(
            path,
            tree.path_to(agent)
                .map(|p| {
                    let mut p = p;
                    *p.last_mut().expect("index") += 1;
                    p
                })
                .expect("agent path")
        );
    }

    #[test]
    fn a_kind_held_once_is_focused_not_opened_again() {
        let mut ws = Workspace::default();
        let review = ws.find(Kind::Review).expect("review");
        let blocks = ws.blocks.len();
        assert_eq!(ws.open(Kind::Review, AREA, None), review);
        assert_eq!(ws.blocks.len(), blocks);
        assert_eq!(
            ws.tree().expect("tree").tabs_of(review).map(|t| t.1),
            Some(0)
        );
    }

    #[test]
    fn parking_keeps_the_block_and_unparking_puts_it_back() {
        let mut ws = Workspace::default();
        let agent = ws.find(Kind::Agent).expect("agent");
        ws.park(agent);
        assert!(!ws.tree().expect("tree").contains(agent));
        assert!(ws.blocks.contains_key(&agent));
        let threads = ws.find(Kind::Threads).expect("threads");
        ws.unpark(agent, Some((threads, Side::Right)), AREA);
        assert!(ws.tree().expect("tree").contains(agent));
        assert!(ws.parked.is_empty());
    }

    #[test]
    fn a_layout_without_threads_gets_it_on_the_left() {
        let mut ws = Workspace::default();
        ws.layouts.push(Layout {
            name: "x".into(),
            tree: Some(Node::leaf(99)),
        });
        ws.tidy();
        ws.switch(4);
        let threads = ws.find(Kind::Threads).expect("threads");
        assert_eq!(ws.tree().expect("tree").blocks()[0], threads);
    }

    #[test]
    fn moving_a_block_onto_another_splits_there() {
        let mut ws = Workspace::default();
        ws.switch(1);
        let term = ws.find(Kind::Terminal).expect("terminal");
        let threads = ws.find(Kind::Threads).expect("threads");
        assert!(ws.move_block(term, threads, Side::Down));
        let tree = ws.tree().expect("tree");
        assert_eq!(tree.blocks()[..2], [threads, term]);
    }

    #[test]
    fn a_floating_block_leaves_the_tree_and_lands_by_the_rule() {
        let mut ws = Workspace::default();
        let review = ws.find(Kind::Review).expect("review");
        ws.float(review, AREA);
        assert_eq!(ws.floating, Some(review));
        assert!(!ws.tree().expect("tree").contains(review));
        ws.land(AREA);
        assert!(ws.tree().expect("tree").contains(review));
        assert_eq!(ws.floating, None);
    }

    #[test]
    fn round_trips_through_json() {
        let ws = Workspace::default();
        let json = serde_json::to_value(&ws).expect("json");
        assert_eq!(serde_json::from_value::<Workspace>(json).expect("back"), ws);
    }
}
