//! Where each leaf of a tree lands in an area, and which leaf lies next to another: what
//! placing a new block and moving focus with the arrows need.

use serde::{Deserialize, Serialize};

use crate::{BlockId, Dir, Node, Side};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn area(&self) -> f32 {
        self.w * self.h
    }

    fn right(&self) -> f32 {
        self.x + self.w
    }

    fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// Each leaf's shown block and its rectangle, with `gap` between siblings.
pub fn leaves(node: &Node, area: Rect, gap: f32) -> Vec<(BlockId, Rect)> {
    let mut out = Vec::new();
    collect(node, area, gap, &mut out);
    out
}

fn collect(node: &Node, area: Rect, gap: f32, out: &mut Vec<(BlockId, Rect)>) {
    match node {
        Node::Leaf { tabs, active } => {
            if let Some(id) = tabs.get(*active) {
                out.push((*id, area));
            }
        }
        Node::Split { dir, sizes, kids } => {
            let total: f32 = sizes.iter().sum();
            let gaps = gap * (kids.len().saturating_sub(1)) as f32;
            let length = match dir {
                Dir::Row => area.w,
                Dir::Col => area.h,
            };
            let free = (length - gaps).max(0.);
            let mut at = 0.;
            for (size, kid) in sizes.iter().zip(kids) {
                let part = free * size / total;
                let rect = match dir {
                    Dir::Row => Rect::new(area.x + at, area.y, part, area.h),
                    Dir::Col => Rect::new(area.x, area.y + at, area.w, part),
                };
                collect(kid, rect, gap, out);
                at += part + gap;
            }
        }
    }
}

/// The shown block next to `from` toward `side`: the nearest leaf past that edge that
/// overlaps it most along the other axis.
pub fn neighbour(node: &Node, area: Rect, from: BlockId, side: Side) -> Option<BlockId> {
    let all = leaves(node, area, 0.);
    let (_, me) = all.iter().find(|(id, _)| *id == from)?;
    let edge = 0.5;
    all.iter()
        .filter(|(id, _)| *id != from)
        .filter_map(|(id, r)| {
            let (gap, overlap) = match side {
                Side::Right => (r.x - me.right(), span(me.y, me.bottom(), r.y, r.bottom())),
                Side::Left => (me.x - r.right(), span(me.y, me.bottom(), r.y, r.bottom())),
                Side::Down => (r.y - me.bottom(), span(me.x, me.right(), r.x, r.right())),
                Side::Up => (me.y - r.bottom(), span(me.x, me.right(), r.x, r.right())),
                Side::Center => return None,
            };
            (gap > -edge && overlap > 0.).then_some((*id, gap, overlap))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.2.total_cmp(&a.2)))
        .map(|(id, _, _)| id)
}

fn span(a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    a1.min(b1) - a0.max(b0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Node {
        // 1 | (2 over 3)
        Node::split(
            Dir::Row,
            vec![
                (1., Node::leaf(1)),
                (
                    1.,
                    Node::split(Dir::Col, vec![(1., Node::leaf(2)), (1., Node::leaf(3))]),
                ),
            ],
        )
    }

    #[test]
    fn leaves_share_the_area_less_the_gaps() {
        let rects = leaves(&tree(), Rect::new(0., 0., 206., 106.), 6.);
        assert_eq!(rects[0], (1, Rect::new(0., 0., 100., 106.)));
        assert_eq!(rects[1], (2, Rect::new(106., 0., 100., 50.)));
        assert_eq!(rects[2], (3, Rect::new(106., 56., 100., 50.)));
    }

    #[test]
    fn the_neighbour_is_past_the_edge_and_overlapping() {
        let area = Rect::new(0., 0., 200., 100.);
        assert_eq!(neighbour(&tree(), area, 1, Side::Right), Some(2));
        assert_eq!(neighbour(&tree(), area, 3, Side::Up), Some(2));
        assert_eq!(neighbour(&tree(), area, 3, Side::Left), Some(1));
        assert_eq!(neighbour(&tree(), area, 1, Side::Left), None);
    }
}
