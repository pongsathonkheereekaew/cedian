//! A layout's split tree: each leaf a block, or several as tabs; each split a row or a column
//! of trees with their shares.

use serde::{Deserialize, Serialize};

use crate::BlockId;

/// The least share a split gives a child, as a part of the split.
pub const MIN_SHARE: f32 = 0.08;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    /// Children left to right.
    Row,
    /// Children top to bottom.
    Col,
}

/// Where a block goes against another: beside it, or into its tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
    Up,
    Down,
    Center,
}

impl Side {
    pub fn dir(self) -> Option<Dir> {
        match self {
            Side::Left | Side::Right => Some(Dir::Row),
            Side::Up | Side::Down => Some(Dir::Col),
            Side::Center => None,
        }
    }

    /// Right or down: after the other block in its split.
    pub fn after(self) -> bool {
        matches!(self, Side::Right | Side::Down)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Node {
    Leaf {
        tabs: Vec<BlockId>,
        active: usize,
    },
    Split {
        dir: Dir,
        sizes: Vec<f32>,
        kids: Vec<Node>,
    },
}

impl Node {
    pub fn leaf(id: BlockId) -> Self {
        Node::Leaf {
            tabs: vec![id],
            active: 0,
        }
    }

    pub fn split(dir: Dir, parts: Vec<(f32, Node)>) -> Self {
        let (sizes, kids) = parts.into_iter().unzip();
        Node::Split { dir, sizes, kids }.flattened()
    }

    pub fn contains(&self, id: BlockId) -> bool {
        match self {
            Node::Leaf { tabs, .. } => tabs.contains(&id),
            Node::Split { kids, .. } => kids.iter().any(|k| k.contains(id)),
        }
    }

    /// Every block, tabs included, in reading order.
    pub fn blocks(&self) -> Vec<BlockId> {
        match self {
            Node::Leaf { tabs, .. } => tabs.clone(),
            Node::Split { kids, .. } => kids.iter().flat_map(Node::blocks).collect(),
        }
    }

    /// The block each leaf shows, in reading order.
    pub fn shown(&self) -> Vec<BlockId> {
        match self {
            Node::Leaf { tabs, active } => tabs.get(*active).copied().into_iter().collect(),
            Node::Split { kids, .. } => kids.iter().flat_map(Node::shown).collect(),
        }
    }

    /// The tabs of the leaf holding `id`.
    pub fn tabs_of(&self, id: BlockId) -> Option<(&[BlockId], usize)> {
        match self {
            Node::Leaf { tabs, active } if tabs.contains(&id) => Some((tabs, *active)),
            Node::Leaf { .. } => None,
            Node::Split { kids, .. } => kids.iter().find_map(|k| k.tabs_of(id)),
        }
    }

    /// Shows `id` in its leaf. False when no leaf holds it.
    pub fn activate(&mut self, id: BlockId) -> bool {
        match self {
            Node::Leaf { tabs, active } => match tabs.iter().position(|t| *t == id) {
                Some(n) => {
                    *active = n;
                    true
                }
                None => false,
            },
            Node::Split { kids, .. } => kids.iter_mut().any(|k| k.activate(id)),
        }
    }

    /// Puts `new` against the leaf holding `target`: beside it, splitting the leaf's place in
    /// two, or as its last tab. False when no leaf holds `target`.
    pub fn insert(&mut self, target: BlockId, new: BlockId, side: Side) -> bool {
        let done = self.insert_inner(target, new, side);
        if done {
            *self = std::mem::replace(self, Node::leaf(new)).flattened();
        }
        done
    }

    fn insert_inner(&mut self, target: BlockId, new: BlockId, side: Side) -> bool {
        match self {
            Node::Leaf { tabs, active } if tabs.contains(&target) => {
                match side.dir() {
                    None => {
                        tabs.push(new);
                        *active = tabs.len() - 1;
                    }
                    Some(dir) => {
                        let old = std::mem::replace(self, Node::leaf(new));
                        let kids = if side.after() {
                            vec![old, Node::leaf(new)]
                        } else {
                            vec![Node::leaf(new), old]
                        };
                        *self = Node::Split {
                            dir,
                            sizes: vec![1., 1.],
                            kids,
                        };
                    }
                }
                true
            }
            Node::Leaf { .. } => false,
            Node::Split { kids, .. } => kids.iter_mut().any(|k| k.insert_inner(target, new, side)),
        }
    }

    /// The tree without `id`; a split left with one child becomes that child, and none is
    /// left when `id` was the only block.
    pub fn remove(self, id: BlockId) -> Option<Node> {
        match self {
            Node::Leaf { mut tabs, active } => {
                let Some(n) = tabs.iter().position(|t| *t == id) else {
                    return Some(Node::Leaf { tabs, active });
                };
                tabs.remove(n);
                if tabs.is_empty() {
                    return None;
                }
                let active = if n < active { active - 1 } else { active };
                Some(Node::Leaf {
                    active: active.min(tabs.len() - 1),
                    tabs,
                })
            }
            Node::Split { dir, sizes, kids } => {
                let (sizes, kids): (Vec<f32>, Vec<Node>) = sizes
                    .into_iter()
                    .zip(kids)
                    .filter_map(|(size, kid)| Some((size, kid.remove(id)?)))
                    .unzip();
                match kids.len() {
                    0 => None,
                    _ => Some(Node::Split { dir, sizes, kids }.flattened()),
                }
            }
        }
    }

    /// A split of one child is that child, and a split inside a split of the same direction
    /// gives its children to the outer one, keeping every block's share of the whole.
    pub fn flattened(self) -> Node {
        match self {
            Node::Leaf { .. } => self,
            Node::Split { dir, sizes, kids } => {
                let mut out_sizes = Vec::new();
                let mut out_kids = Vec::new();
                for (size, kid) in sizes.into_iter().zip(kids) {
                    match kid.flattened() {
                        Node::Split {
                            dir: inner,
                            sizes: inner_sizes,
                            kids: inner_kids,
                        } if inner == dir => {
                            let total: f32 = inner_sizes.iter().sum();
                            for (s, k) in inner_sizes.into_iter().zip(inner_kids) {
                                out_sizes.push(size * s / total);
                                out_kids.push(k);
                            }
                        }
                        kid => {
                            out_sizes.push(size);
                            out_kids.push(kid);
                        }
                    }
                }
                if out_kids.len() == 1 {
                    return out_kids.remove(0);
                }
                Node::Split {
                    dir,
                    sizes: out_sizes,
                    kids: out_kids,
                }
            }
        }
    }

    /// Every split's children get equal shares.
    pub fn even(&mut self) {
        if let Node::Split { sizes, kids, .. } = self {
            sizes.iter_mut().for_each(|s| *s = 1.);
            kids.iter_mut().for_each(Node::even);
        }
    }

    /// The node at `path`, each step a child index.
    pub fn at_mut(&mut self, path: &[usize]) -> Option<&mut Node> {
        match path.split_first() {
            None => Some(self),
            Some((first, rest)) => match self {
                Node::Split { kids, .. } => kids.get_mut(*first)?.at_mut(rest),
                Node::Leaf { .. } => None,
            },
        }
    }

    /// Moves the divider after child `divider` of the split at `path` to `at`, a part of the
    /// split's length from its start; the two children beside it share the change.
    pub fn move_divider(&mut self, path: &[usize], divider: usize, at: f32) {
        self.move_divider_within(path, divider, at, (MIN_SHARE, MIN_SHARE));
    }

    /// `move_divider`, keeping at least `min.0` of the split's length before the divider's
    /// child and `min.1` for the one after it (each at least `MIN_SHARE`).
    pub fn move_divider_within(
        &mut self,
        path: &[usize],
        divider: usize,
        at: f32,
        min: (f32, f32),
    ) {
        let Some(Node::Split { sizes, .. }) = self.at_mut(path) else {
            return;
        };
        if divider + 1 >= sizes.len() {
            return;
        }
        let total: f32 = sizes.iter().sum();
        let start = sizes[..divider].iter().sum::<f32>() / total;
        let end = sizes[..=divider + 1].iter().sum::<f32>() / total;
        let (before, after) = (min.0.max(MIN_SHARE), min.1.max(MIN_SHARE));
        if end - start < before + after {
            return;
        }
        let at = at.clamp(start + before, end - after);
        sizes[divider] = (at - start) * total;
        sizes[divider + 1] = (end - at) * total;
    }

    /// Evens out the split at `path` only, as a double click on one of its dividers does.
    pub fn even_at(&mut self, path: &[usize]) {
        if let Some(Node::Split { sizes, .. }) = self.at_mut(path) {
            sizes.iter_mut().for_each(|s| *s = 1.);
        }
    }

    /// Grows the leaf holding `id` toward `side` by `by` of the nearest split that runs that
    /// way, taken from its neighbour on that side (or the other when it has none there).
    pub fn grow(&mut self, id: BlockId, side: Side, by: f32) -> bool {
        let Some(dir) = side.dir() else {
            return false;
        };
        let Some(path) = self.path_to(id) else {
            return false;
        };
        // The deepest split along `dir` on the way to the leaf.
        for depth in (0..path.len()).rev() {
            let Some(Node::Split {
                dir: split_dir,
                sizes,
                ..
            }) = self.at_mut(&path[..depth])
            else {
                continue;
            };
            if *split_dir != dir {
                continue;
            }
            let me = path[depth];
            let other = if side.after() {
                (me + 1 < sizes.len())
                    .then_some(me + 1)
                    .or(me.checked_sub(1))
            } else {
                me.checked_sub(1)
                    .or((me + 1 < sizes.len()).then_some(me + 1))
            };
            let Some(other) = other else {
                return false;
            };
            let total: f32 = sizes.iter().sum();
            let take = (by * total).min(sizes[other] - MIN_SHARE * total).max(0.);
            sizes[me] += take;
            sizes[other] -= take;
            return take > 0.;
        }
        false
    }

    /// Child indexes from here to the leaf holding `id`.
    pub fn path_to(&self, id: BlockId) -> Option<Vec<usize>> {
        match self {
            Node::Leaf { tabs, .. } => tabs.contains(&id).then(Vec::new),
            Node::Split { kids, .. } => kids.iter().enumerate().find_map(|(n, kid)| {
                let mut path = kid.path_to(id)?;
                path.insert(0, n);
                Some(path)
            }),
        }
    }

    /// The share of its parent split the leaf holding `id` has, the split's direction, and
    /// the block of the child before it (else after it) with the side it lies on from there.
    pub fn place_of(&self, id: BlockId) -> Option<Place> {
        let path = self.path_to(id)?;
        let Some((last, parent)) = path.split_last() else {
            return Some(Place {
                share: 1.,
                neighbour: None,
            });
        };
        let mut node = self;
        for n in parent {
            if let Node::Split { kids, .. } = node {
                node = &kids[*n];
            }
        }
        let Node::Split { dir, sizes, kids } = node else {
            return None;
        };
        let total: f32 = sizes.iter().sum();
        let (other, side) = if *last > 0 {
            (
                *last - 1,
                if *dir == Dir::Row {
                    Side::Right
                } else {
                    Side::Down
                },
            )
        } else {
            (
                1,
                if *dir == Dir::Row {
                    Side::Left
                } else {
                    Side::Up
                },
            )
        };
        Some(Place {
            share: sizes[*last] / total,
            neighbour: kids[other].shown().first().map(|b| (*b, side)),
        })
    }

    /// Sets the share of its parent split the leaf holding `id` has, the others keeping
    /// their proportions.
    pub fn set_share(&mut self, id: BlockId, share: f32) {
        let Some(path) = self.path_to(id) else {
            return;
        };
        let Some((last, parent)) = path.split_last() else {
            return;
        };
        if let Some(Node::Split { sizes, .. }) = self.at_mut(parent) {
            let others: f32 = sizes
                .iter()
                .enumerate()
                .filter(|(n, _)| n != last)
                .map(|(_, s)| s)
                .sum();
            let share = share.clamp(MIN_SHARE, 1. - MIN_SHARE);
            sizes[*last] = others * share / (1. - share);
        }
    }
}

/// Where a leaf sits in its split, to put it back there.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub share: f32,
    /// A block beside it and the side the leaf lies on from that block.
    pub neighbour: Option<(BlockId, Side)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(parts: Vec<(f32, Node)>) -> Node {
        Node::split(Dir::Row, parts)
    }

    #[test]
    fn splitting_beside_a_block_in_a_row_halves_its_share() {
        let mut tree = row(vec![(1., Node::leaf(1)), (2., Node::leaf(2))]);
        assert!(tree.insert(2, 3, Side::Right));
        assert_eq!(
            tree,
            Node::Split {
                dir: Dir::Row,
                sizes: vec![1., 1., 1.],
                kids: vec![Node::leaf(1), Node::leaf(2), Node::leaf(3)],
            }
        );
    }

    #[test]
    fn splitting_down_nests_a_column() {
        let mut tree = row(vec![(1., Node::leaf(1)), (1., Node::leaf(2))]);
        tree.insert(1, 3, Side::Up);
        assert_eq!(tree.blocks(), [3, 1, 2]);
        let Node::Split { kids, .. } = &tree else {
            panic!("split");
        };
        assert!(matches!(&kids[0], Node::Split { dir: Dir::Col, .. }));
    }

    #[test]
    fn a_tab_joins_the_leaf_and_shows() {
        let mut tree = Node::leaf(1);
        tree.insert(1, 2, Side::Center);
        assert_eq!(tree.tabs_of(2), Some((&[1, 2][..], 1)));
        assert_eq!(tree.shown(), [2]);
    }

    #[test]
    fn removing_collapses_a_split_of_one() {
        let tree = row(vec![
            (1., Node::leaf(1)),
            (
                1.,
                Node::split(Dir::Col, vec![(1., Node::leaf(2)), (1., Node::leaf(3))]),
            ),
        ]);
        let tree = tree.remove(3).expect("left");
        assert_eq!(
            tree,
            Node::Split {
                dir: Dir::Row,
                sizes: vec![1., 1.],
                kids: vec![Node::leaf(1), Node::leaf(2)],
            }
        );
        assert_eq!(Node::leaf(1).remove(1), None);
    }

    #[test]
    fn removing_a_tab_keeps_the_shown_one() {
        let mut tree = Node::leaf(1);
        tree.insert(1, 2, Side::Center);
        tree.insert(1, 3, Side::Center);
        tree.activate(3);
        let tree = tree.remove(1).expect("left");
        assert_eq!(tree.shown(), [3]);
    }

    #[test]
    fn a_divider_moves_between_its_two_children_only() {
        let mut tree = row(vec![
            (1., Node::leaf(1)),
            (1., Node::leaf(2)),
            (2., Node::leaf(3)),
        ]);
        tree.move_divider(&[], 0, 0.5);
        let Node::Split { sizes, .. } = &tree else {
            panic!("split");
        };
        // The second child keeps its least share; the third does not move.
        assert!((sizes[0] - 1.68).abs() < 1e-5, "{sizes:?}");
        assert!((sizes[1] - 0.32).abs() < 1e-5, "{sizes:?}");
        assert_eq!(sizes[2], 2.);
    }

    #[test]
    fn growing_takes_from_the_neighbour_on_that_side() {
        let mut tree = row(vec![
            (1., Node::leaf(1)),
            (1., Node::leaf(2)),
            (1., Node::leaf(3)),
        ]);
        assert!(tree.grow(2, Side::Right, 0.1));
        let Node::Split { sizes, .. } = &tree else {
            panic!("split");
        };
        assert!((sizes[0] - 1.).abs() < 1e-6);
        assert!((sizes[1] - 1.3).abs() < 1e-6);
        assert!((sizes[2] - 0.7).abs() < 1e-6);
        // A column has no split along a row.
        assert!(!Node::leaf(1).grow(1, Side::Right, 0.1));
    }

    #[test]
    fn a_place_names_the_neighbour_and_the_share() {
        let tree = row(vec![(1., Node::leaf(1)), (3., Node::leaf(2))]);
        let place = tree.place_of(1).expect("place");
        assert_eq!(place.share, 0.25);
        assert_eq!(place.neighbour, Some((2, Side::Left)));
        let place = tree.place_of(2).expect("place");
        assert_eq!(place.neighbour, Some((1, Side::Right)));
    }

    #[test]
    fn round_trips_through_json() {
        let tree = row(vec![(1., Node::leaf(1)), (3., Node::leaf(2))]);
        let json = serde_json::to_string(&tree).expect("json");
        assert_eq!(serde_json::from_str::<Node>(&json).expect("back"), tree);
    }
}
