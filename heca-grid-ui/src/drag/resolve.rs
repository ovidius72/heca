//! Drop-target / drag-source resolution over a laid-out component tree.
//!
//! These are the geometry layer of the drag framework: pure bounds walks over the
//! **retained widget tree** (whose `Base.bounds` are filled in by layout each
//! frame). They replace hand-computed, per-surface hit-testing — any widget marked
//! via [`DragExt`](crate::builders::DragExt) participates automatically.
//!
//! Domain-neutral: results are opaque [`DragItemId`]s the app maps back to its own
//! model. No app types, no GPU, no per-surface special-casing.

use crate::component::Component;
use crate::drag::DragItemId;
use crate::reactive::SignalGet;
use heca_core::layout::{Point, Rectangle};

/// Where, relative to a drop target, a drop would land. The app decides what each
/// means (insert before/after a sibling, or drop *onto* a container).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropSide {
    /// Above/before the target (top third of a vertically-stacked target).
    Before,
    /// Onto the target itself (its middle band) — e.g. drop into a container.
    Onto,
    /// Below/after the target (bottom third).
    After,
}

/// A resolved drop position: which target, its laid-out bounds, and the side the
/// cursor is over (for an insertion indicator).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropHit {
    /// The opaque id the app registered via [`DragExt::drop_target`](crate::builders::DragExt::drop_target).
    pub id: DragItemId,
    /// The target's laid-out bounds (logical px) — paint the indicator against these.
    pub bounds: Rectangle,
    /// Which side of the target the cursor is over.
    pub side: DropSide,
}

/// Classify `point` against `bounds` into a [`DropSide`] by vertical thirds
/// (the common vertically-stacked list case).
fn side_for(bounds: Rectangle, point: Point) -> DropSide {
    let third = bounds.size.h / 3.0;
    let dy = point.y - bounds.loc.y;
    if dy < third {
        DropSide::Before
    } else if dy > bounds.size.h - third {
        DropSide::After
    } else {
        DropSide::Onto
    }
}

/// Should this subtree be considered for hit-testing? Hidden widgets have stale
/// bounds and never receive input, so they're skipped (matching paint/event).
fn skip(c: &dyn Component) -> bool {
    !c.base().visible.get_untracked() || c.base().style.hidden
}

/// Find the **topmost, deepest** drop target whose bounds contain `point`.
///
/// Walks children last-added-first (top z-order wins, like event routing) and
/// descends before testing the parent, so the most specific target under the
/// cursor is returned. `None` if no drop target is hit.
pub fn resolve_at(root: &dyn Component, point: Point) -> Option<DropHit> {
    if skip(root) {
        return None;
    }
    for child in root.base().children.iter().rev() {
        if let Some(hit) = resolve_at(child.as_ref(), point) {
            return Some(hit);
        }
    }
    if let Some(id) = root.as_drop_target() {
        let bounds = root.base().bounds;
        if bounds.contains(point) {
            return Some(DropHit {
                id,
                bounds,
                side: side_for(bounds, point),
            });
        }
    }
    None
}

/// Find the **topmost, deepest** drag source whose bounds contain `point` — the
/// thing a press at `point` would start dragging. `None` if none is hit.
pub fn source_at(root: &dyn Component, point: Point) -> Option<DragItemId> {
    if skip(root) {
        return None;
    }
    for child in root.base().children.iter().rev() {
        if let Some(id) = source_at(child.as_ref(), point) {
            return Some(id);
        }
    }
    root.as_drag_source()
        .filter(|_| root.base().bounds.contains(point))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{DragExt, Parent};
    use crate::widgets::{Flex, Surface};
    use heca_core::layout::{Point, Rectangle, Size};

    /// Force bounds on a widget for hit-testing (layout doesn't run in unit tests).
    fn at(mut w: impl Component + 'static, x: f64, y: f64, w_: f64, h_: f64) -> Box<dyn Component> {
        w.base_mut().bounds = Rectangle::new(Point::new(x, y), Size::new(w_, h_));
        Box::new(w)
    }

    #[test]
    fn resolve_at_finds_a_drop_target_and_side() {
        // A single drop-target surface at (0,0)-(100,90).
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 90.0));
        root.base_mut().children.push(at(
            Surface::new().drop_target(DragItemId::new(5)),
            0.0,
            0.0,
            100.0,
            90.0,
        ));

        // Top third → Before, middle → Onto, bottom third → After.
        assert_eq!(resolve_at(&root, Point::new(50.0, 10.0)).map(|h| h.side), Some(DropSide::Before));
        assert_eq!(resolve_at(&root, Point::new(50.0, 45.0)).map(|h| h.side), Some(DropSide::Onto));
        assert_eq!(resolve_at(&root, Point::new(50.0, 80.0)).map(|h| h.side), Some(DropSide::After));
        assert_eq!(resolve_at(&root, Point::new(50.0, 45.0)).map(|h| h.id), Some(DragItemId::new(5)));
    }

    #[test]
    fn resolve_at_returns_the_deepest_target() {
        // Outer target contains an inner target; a hit inside the inner returns it.
        let mut inner = Surface::new().drop_target(DragItemId::new(2));
        inner.base_mut().bounds = Rectangle::new(Point::new(10.0, 10.0), Size::new(30.0, 30.0));
        let mut outer = Surface::new().drop_target(DragItemId::new(1));
        outer.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        outer.base_mut().children.push(Box::new(inner));

        assert_eq!(resolve_at(&outer, Point::new(20.0, 20.0)).map(|h| h.id), Some(DragItemId::new(2)));
        // Outside the inner but inside the outer → the outer.
        assert_eq!(resolve_at(&outer, Point::new(80.0, 80.0)).map(|h| h.id), Some(DragItemId::new(1)));
    }

    #[test]
    fn resolve_at_misses_outside_all_targets() {
        let mut root = Surface::new().drop_target(DragItemId::new(1));
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0));
        assert!(resolve_at(&root, Point::new(50.0, 50.0)).is_none());
    }

    #[test]
    fn source_at_finds_the_topmost_drag_source() {
        // Two overlapping sources; the later child (top z-order) wins.
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        root.base_mut().children.push(at(Surface::new().draggable(DragItemId::new(1)), 0.0, 0.0, 100.0, 100.0));
        root.base_mut().children.push(at(Surface::new().draggable(DragItemId::new(2)), 0.0, 0.0, 100.0, 100.0));
        assert_eq!(source_at(&root, Point::new(50.0, 50.0)), Some(DragItemId::new(2)));
    }

    #[test]
    fn source_at_ignores_drop_only_widgets() {
        let mut root = Surface::new().drop_target(DragItemId::new(9));
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        assert!(source_at(&root, Point::new(50.0, 50.0)).is_none());
    }
}
