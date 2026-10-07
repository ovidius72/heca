//! **What is sliding on one window's screen** — a column or a pane easing from where it was to
//! where it is now.
//!
//! A slide is how one window shows a change, not part of the change: two windows on one session
//! each animate their own copy of a move, so it is held by the window's [`ScrollView`], keyed by
//! the column or pane that is moving, and the shared content holds none of it.

use std::collections::HashMap;

use super::animation::{Animated, Animation, AnimationConfig};
use super::types::{ColumnId, PaneId, Point};

/// The slides in progress in one workspace, as one window sees it.
#[derive(Debug, Clone, Default)]
pub struct Motion {
    columns: HashMap<ColumnId, Animated<f64>>,
    panes: HashMap<PaneId, Animated<Point>>,
}

impl Motion {
    /// How far column `id` is still to slide horizontally; `0` when it is not sliding.
    pub(crate) fn column_offset(&self, id: ColumnId) -> f64 {
        self.columns.get(&id).map_or(0.0, |slide| slide.current())
    }

    /// How far pane `id` is still to slide; the origin when it is not sliding.
    pub(crate) fn pane_offset(&self, id: PaneId) -> Point {
        self.panes.get(&id).map_or_else(Point::default, |slide| slide.current())
    }

    /// Slide column `id` from `from_x` to where it is, adding to a slide already under way so a
    /// second move does not jump.
    pub(crate) fn slide_column(&mut self, id: ColumnId, from_x: f64, config: AnimationConfig) {
        let current = self.column_offset(id);
        let from = from_x + current;
        self.columns.insert(
            id,
            Animated::Animating {
                animation: Animation::new(from, 0.0, config),
                from,
                to: 0.0,
            },
        );
    }

    /// Slide pane `id` from `from` to where it is, adding to a slide already under way so a
    /// second move does not jump.
    pub(crate) fn slide_pane(&mut self, id: PaneId, from: Point, config: AnimationConfig) {
        let current = self.pane_offset(id);
        let from = Point::new(from.x + current.x, from.y + current.y);
        self.panes.insert(
            id,
            Animated::Animating {
                animation: Animation::new(0.0, 1.0, config),
                from,
                to: Point::default(),
            },
        );
    }

    /// Whether anything is still sliding.
    pub(crate) fn is_animating(&self) -> bool {
        self.columns.values().any(|a| !a.is_done()) || self.panes.values().any(|a| !a.is_done())
    }

    /// Forget the slides that have finished, and those of columns and panes that are gone.
    pub(crate) fn advance(
        &mut self,
        live_columns: impl Fn(ColumnId) -> bool,
        live_panes: impl Fn(PaneId) -> bool,
    ) {
        self.columns.retain(|id, slide| live_columns(*id) && !slide.is_done());
        self.panes.retain(|id, slide| live_panes(*id) && !slide.is_done());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slide_starts_at_its_offset_and_a_second_one_adds_to_it() {
        let mut motion = Motion::default();
        let id = ColumnId(1);
        assert_eq!(motion.column_offset(id), 0.0);
        motion.slide_column(id, 100.0, AnimationConfig::default());
        let first = motion.column_offset(id);
        assert!(first > 0.0 && first <= 100.0, "{first}");
        assert!(motion.is_animating());
        motion.slide_column(id, 50.0, AnimationConfig::default());
        assert!(motion.column_offset(id) > first, "the second slide adds to the first");
    }

    #[test]
    fn a_pane_slides_from_a_point() {
        let mut motion = Motion::default();
        motion.slide_pane(PaneId(3), Point::new(0.0, 80.0), AnimationConfig::default());
        assert!(motion.pane_offset(PaneId(3)).y > 0.0);
        assert_eq!(motion.pane_offset(PaneId(4)), Point::default());
    }

    #[test]
    fn what_is_gone_is_forgotten_on_the_next_turn() {
        let mut motion = Motion::default();
        motion.slide_column(ColumnId(1), 10.0, AnimationConfig::default());
        motion.slide_pane(PaneId(2), Point::new(5.0, 5.0), AnimationConfig::default());
        motion.advance(|c| c != ColumnId(1), |_| true);
        assert_eq!(motion.column_offset(ColumnId(1)), 0.0, "its column is gone");
        assert!(motion.pane_offset(PaneId(2)).x > 0.0, "its pane is still here");
        motion.advance(|_| true, |p| p != PaneId(2));
        assert!(!motion.is_animating());
    }
}
