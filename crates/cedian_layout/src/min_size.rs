//! How small a block may get (Interface, Blocks: minimum sizes): each kind keeps room for its
//! header and what it must show, and a split is as small as its children allow. The window
//! fits the tree to these when it draws and clamps dividers at them; the tree keeps its shares.

use crate::{BlockId, Dir, Kind, Node};

impl Kind {
    /// The narrowest a block of this kind is drawn, in px.
    pub fn min_width(self) -> f32 {
        match self {
            Kind::Threads => 210.,
            Kind::Agent => 340.,
            Kind::Review | Kind::Browser | Kind::Chat => 300.,
            Kind::Usage => 240.,
            _ => 260.,
        }
    }

    /// The shortest, in px: the 36 px header and a few lines.
    pub fn min_height(self) -> f32 {
        150.
    }
}

/// The smallest width and height `node` fits in with `gap` between siblings; a leaf takes the
/// largest of its tabs' minimums.
pub fn min_size(node: &Node, kind_of: &impl Fn(BlockId) -> Option<Kind>, gap: f32) -> (f32, f32) {
    match node {
        Node::Leaf { tabs, .. } => {
            tabs.iter()
                .filter_map(|id| kind_of(*id))
                .fold((0., 0.), |(w, h), kind| {
                    (
                        f32::max(w, kind.min_width()),
                        f32::max(h, kind.min_height()),
                    )
                })
        }
        Node::Split { dir, kids, .. } => {
            let sizes: Vec<(f32, f32)> = kids.iter().map(|k| min_size(k, kind_of, gap)).collect();
            let gaps = gap * kids.len().saturating_sub(1) as f32;
            let sum = |f: fn(&(f32, f32)) -> f32| sizes.iter().map(f).sum::<f32>() + gaps;
            let max = |f: fn(&(f32, f32)) -> f32| sizes.iter().map(f).fold(0., f32::max);
            match dir {
                Dir::Row => (sum(|s| s.0), max(|s| s.1)),
                Dir::Col => (max(|s| s.0), sum(|s| s.1)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_adds_its_children_and_a_column_takes_the_widest() {
        let kinds = |id: BlockId| match id {
            1 => Some(Kind::Threads),
            2 => Some(Kind::Agent),
            _ => Some(Kind::Terminal),
        };
        let tree = Node::split(
            Dir::Row,
            vec![
                (1., Node::leaf(1)),
                (2., Node::leaf(2)),
                (
                    1.,
                    Node::split(Dir::Col, vec![(1., Node::leaf(3)), (1., Node::leaf(4))]),
                ),
            ],
        );
        let (w, h) = min_size(&tree, &kinds, 6.);
        assert_eq!(w, 210. + 340. + 260. + 12.);
        assert_eq!(h, 150. + 150. + 6.);
    }
}
