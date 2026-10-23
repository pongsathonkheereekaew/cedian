//! Blocks move instead of jumping (Interface, Blocks: a split, close, zoom or park animates for
//! 140 ms ease-out, and not at all when the system asks for reduced motion). A change keeps
//! where each shown block was; for the next 140 ms every block is drawn on its own between
//! that rectangle and its new one, and a block that was not shown before fades in. Then the
//! tree is drawn as usual.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use cedian_layout::{BlockId, Rect};

pub const DURATION: Duration = Duration::from_millis(140);

pub struct Motion {
    /// Where each shown block was before the change.
    from: HashMap<BlockId, Rect>,
    started: Instant,
}

impl Motion {
    /// A motion from these rectangles, or none when the system asks for reduced motion.
    pub fn from(rects: impl IntoIterator<Item = (BlockId, Rect)>) -> Option<Self> {
        (!reduced()).then(|| Self {
            from: rects.into_iter().collect(),
            started: Instant::now(),
        })
    }

    /// How far along it is, eased, from 0 to 1; None once it is over.
    pub fn progress(&self, now: Instant) -> Option<f32> {
        let t = now.saturating_duration_since(self.started).as_secs_f32() / DURATION.as_secs_f32();
        (t < 1.).then(|| ease_out(t))
    }

    /// Where `id` is drawn at eased progress `p`, ending at `to`, and its opacity: a block that
    /// was not shown before stays in place and fades in.
    pub fn at(&self, id: BlockId, to: Rect, p: f32) -> (Rect, f32) {
        match self.from.get(&id) {
            Some(from) => (lerp(*from, to, p), 1.),
            None => (to, p),
        }
    }
}

/// Cubic ease-out: fast at first, settling at the end.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    1. - (1. - t).powi(3)
}

fn lerp(a: Rect, b: Rect, p: f32) -> Rect {
    let mix = |x: f32, y: f32| x + (y - x) * p;
    Rect::new(mix(a.x, b.x), mix(a.y, b.y), mix(a.w, b.w), mix(a.h, b.h))
}

/// Whether the system asks for reduced motion (macOS: Accessibility › Display › Reduce
/// motion). Elsewhere nothing says so yet. Tests never move: a motion runs on the wall clock,
/// so a test that measured or clicked a block during one would pass or fail by how busy the
/// machine was.
#[cfg(all(target_os = "macos", not(test)))]
pub fn reduced() -> bool {
    objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

#[cfg(all(not(target_os = "macos"), not(test)))]
pub fn reduced() -> bool {
    false
}

#[cfg(test)]
pub fn reduced() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_out_and_ends() {
        assert_eq!(ease_out(0.), 0.);
        assert_eq!(ease_out(1.), 1.);
        // Past half the distance at a third of the time.
        assert!(ease_out(1. / 3.) > 0.5);
    }

    #[test]
    fn a_kept_block_moves_and_a_new_one_fades_in() {
        let motion = Motion {
            from: HashMap::from([(1, Rect::new(0., 0., 100., 100.))]),
            started: Instant::now(),
        };
        let to = Rect::new(100., 0., 300., 100.);
        assert_eq!(motion.at(1, to, 0.5), (Rect::new(50., 0., 200., 100.), 1.));
        assert_eq!(motion.at(2, to, 0.25), (to, 0.25));
        assert!(motion.progress(motion.started + DURATION).is_none());
    }
}
