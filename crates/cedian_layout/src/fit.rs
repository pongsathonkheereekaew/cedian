//! The fit pass (Interface, Blocks: minimum sizes and folding): when a tree's blocks cannot
//! all keep their minimums in the area, the least important leaf folds into its neighbour as
//! tabs, again until the rest fit. Only what is drawn changes; the layout keeps its tree.

use crate::{BlockId, Kind, Node, Side, min_size};

impl Kind {
    /// Which block keeps its own place longest when space runs out; higher stays.
    pub fn priority(self) -> u8 {
        match self {
            Kind::Agent => 9,
            Kind::Threads => 8,
            Kind::Browser => 7,
            Kind::Review => 6,
            Kind::Terminal => 5,
            Kind::Chat => 4,
            Kind::Editor | Kind::Files | Kind::Git => 3,
            Kind::Session => 2,
            Kind::Outputs | Kind::Memory | Kind::Background => 1,
            Kind::Usage => 0,
        }
    }
}

/// `tree` as it can be drawn in `width` × `height`, with `prefer` (the focused block) shown in
/// whichever leaf holds it.
pub fn fit(
    tree: &Node,
    width: f32,
    height: f32,
    kind_of: &impl Fn(BlockId) -> Option<Kind>,
    gap: f32,
    prefer: Option<BlockId>,
) -> Node {
    let mut tree = tree.clone();
    loop {
        let (w, h) = min_size(&tree, kind_of, gap);
        if w <= width && h <= height {
            break;
        }
        let shown = tree.shown();
        if shown.len() < 2 {
            break;
        }
        // The least important leaf, the later one on a tie.
        let Some(fold) = shown
            .iter()
            .rev()
            .min_by_key(|id| kind_of(**id).map_or(0, Kind::priority))
            .copied()
        else {
            break;
        };
        let Some((into, _)) = tree.place_of(fold).and_then(|p| p.neighbour) else {
            break;
        };
        let tabs = tree
            .tabs_of(fold)
            .map(|(tabs, _)| tabs.to_vec())
            .unwrap_or_default();
        let active = tree.tabs_of(into).map(|(tabs, n)| tabs[n]);
        let Some(mut rest) = tabs.iter().try_fold(tree.clone(), |t, id| t.remove(*id)) else {
            break;
        };
        for id in tabs {
            rest.insert(into, id, Side::Center);
        }
        if let Some(active) = active {
            rest.activate(active);
        }
        tree = rest;
    }
    if let Some(prefer) = prefer {
        tree.activate(prefer);
    }
    tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dir;

    fn kinds(id: BlockId) -> Option<Kind> {
        Some(match id {
            1 => Kind::Threads,
            2 => Kind::Agent,
            3 => Kind::Review,
            _ => Kind::Terminal,
        })
    }

    fn row() -> Node {
        Node::split(
            Dir::Row,
            vec![
                (1., Node::leaf(1)),
                (2., Node::leaf(2)),
                (
                    1.,
                    Node::split(Dir::Col, vec![(1., Node::leaf(3)), (1., Node::leaf(4))]),
                ),
            ],
        )
    }

    #[test]
    fn a_tree_that_fits_is_drawn_as_it_is() {
        let tree = row();
        assert_eq!(fit(&tree, 1600., 900., &kinds, 6., None), tree);
    }

    #[test]
    fn the_least_important_leaf_folds_into_its_neighbour_until_the_rest_fit() {
        // 210 + 340 + 300 and two gaps is 862: at 760 the terminal folds into the review,
        // which is still too wide, so the review's leaf folds into the agent.
        let drawn = fit(&row(), 760., 900., &kinds, 6., None);
        assert_eq!(drawn.shown(), vec![1, 2]);
        assert_eq!(drawn.tabs_of(2).map(|t| t.0.to_vec()), Some(vec![2, 3, 4]));
        // The agent stays the tab shown, and the tree itself is untouched.
        assert_eq!(drawn.tabs_of(2).map(|t| t.1), Some(0));
        let (w, _) = min_size(&drawn, &kinds, 6.);
        assert!(w <= 760.);
    }

    #[test]
    fn the_focused_block_shows_in_the_leaf_it_folded_into() {
        let drawn = fit(&row(), 760., 900., &kinds, 6., Some(3));
        assert_eq!(drawn.shown(), vec![1, 3]);
    }

    #[test]
    fn a_short_window_folds_a_column() {
        // The column needs 306 px for the review over the terminal.
        let drawn = fit(&row(), 1600., 300., &kinds, 6., None);
        assert!(drawn.tabs_of(3).is_some_and(|t| t.0.contains(&4)));
    }
}
