//! Drop-target / drag-source resolution over a laid-out component tree.
//!
//! These are the geometry layer of the drag framework: pure bounds walks over the
//! **retained widget tree** (whose `Base.bounds` are filled in by layout each
//! frame). They replace hand-computed, per-surface hit-testing — any widget marked
//! via [`ComponentExt`](crate::builders::ComponentExt) participates automatically.
//!
//! Domain-neutral: a result is the widget's **identity** — the name it declared with
//! [`key`](crate::component::Base::key), or one derived from its content when it declared none
//! ([`nav::identity_of`](crate::nav::identity_of)). The same string the keyboard cursor, the
//! right-click target and a remembered hint letter use, so all four agree about what they are
//! pointing at. No app types, no GPU, no per-surface special-casing, and nothing handed out by a
//! host, so a plugin's row drags on the same terms as ours — and `.draggable()` on its own is
//! enough, with no name to invent.

use crate::component::Component;
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
#[derive(Clone, Debug, PartialEq)]
pub struct DropHit {
    /// The target's own name — what it declared with
    /// [`ComponentExt::key`](crate::builders::ComponentExt::key).
    pub key: String,
    /// The target's laid-out bounds (logical px) — paint the indicator against these.
    pub bounds: Rectangle, /// Which side of the target the cursor is over.
    pub side: DropSide}

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
    !c.base().visible.get_untracked() || c.base().style.layout.hidden
}

/// Find the **topmost, deepest** drop target whose bounds contain `point`.
///
/// Walks children last-added-first (top z-order wins, like event routing) and
/// descends before testing the parent, so the most specific target under the
/// cursor is returned. `None` if no drop target is hit.
pub fn resolve_at(root: &dyn Component, point: Point) -> Option<DropHit> {
    resolve_at_filtered(root, point, &|_| true)
}

/// Like [`resolve_at`], but only considers drop targets whose id satisfies
/// `accept`. A rejected target is skipped *and the walk continues outward*, so the
/// deepest **accepted** target under the cursor wins — e.g. while dragging a
/// container you can accept only container-level targets and have a nested
/// leaf target fall through to its accepted ancestor. Domain-neutral: the app
/// decides acceptance from the target's own name.
pub fn resolve_at_filtered(
    root: &dyn Component,
    point: Point,
    accept: &dyn Fn(&str) -> bool,
) -> Option<DropHit> {
    let mut path = Vec::new();
    drop_at(root, root, point, accept, &mut path)
}

/// The recursive half of [`resolve_at_filtered`], carrying the path so a hit can be named by
/// [`identity_of`](crate::nav::identity_of) — which needs the whole chain from the root to derive a
/// name for a widget that declared none.
fn drop_at(
    root: &dyn Component,
    node: &dyn Component,
    point: Point,
    accept: &dyn Fn(&str) -> bool,
    path: &mut Vec<usize>,
) -> Option<DropHit> {
    if skip(node) {
        return None;
    }
    for (i, child) in node.base().children.iter().enumerate().rev() {
        path.push(i);
        if let Some(hit) = drop_at(root, child.as_ref(), point, accept, path) {
            return Some(hit);
        }
        path.pop();
    }
    if node.is_drop_target()
        && let Some(key) = drag_identity(root, node, path)
        && accept(&key)
    {
        let bounds = node.base().bounds;
        if bounds.contains(point) {
            return Some(DropHit {
                key,
                bounds,
                side: side_for(bounds, point),
            });
        }
    }
    None
}

/// Find the **topmost, deepest** drag source whose bounds contain `point` — the
/// thing a press at `point` would start dragging. `None` if none is hit.
pub fn source_at(root: &dyn Component, point: Point) -> Option<String> {
    let mut path = Vec::new();
    source_at_path(root, root, point, &mut path)
}

/// The recursive half of [`source_at`] — see [`drop_at`] for why the path is carried.
fn source_at_path(
    root: &dyn Component,
    node: &dyn Component,
    point: Point,
    path: &mut Vec<usize>,
) -> Option<String> {
    if skip(node) {
        return None;
    }
    for (i, child) in node.base().children.iter().enumerate().rev() {
        path.push(i);
        if let Some(id) = source_at_path(root, child.as_ref(), point, path) {
            return Some(id);
        }
        path.pop();
    }
    if node.is_drag_source() && node.base().bounds.contains(point) {
        return drag_identity(root, node, path);
    }
    None
}

/// **What a dragged widget is called** — the name it declared, and a derived one when it declared
/// none.
///
/// Two readers have to agree and they want it spelled slightly differently, so this is the one
/// place that decides. A widget that named itself answers with **that name, unscoped**, because a
/// right-click and a drag must point at the same thing and
/// [`key_at`](crate::nav::key_at) answers a right-click with the row's own name. A widget that named
/// nothing answers with the identity derived from its content
/// ([`identity_of`](crate::nav::identity_of)) — which is what keeps `.draggable()` working on its
/// own, with no name for an author to invent and no internal rule to learn first.
fn drag_identity(root: &dyn Component, node: &dyn Component, path: &[usize]) -> Option<String> {
    node.base()
        .key
        .clone()
        .or_else(|| crate::nav::identity_of(root, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::ComponentExt;
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
            Surface::new().key("pane:5").drop_target(),
            0.0,
            0.0,
            100.0,
            90.0,
        ));

        // Top third → Before, middle → Onto, bottom third → After.
        assert_eq!(
            resolve_at(&root, Point::new(50.0, 10.0)).map(|h| h.side),
            Some(DropSide::Before)
        );
        assert_eq!(
            resolve_at(&root, Point::new(50.0, 45.0)).map(|h| h.side),
            Some(DropSide::Onto)
        );
        assert_eq!(
            resolve_at(&root, Point::new(50.0, 80.0)).map(|h| h.side),
            Some(DropSide::After)
        );
        assert_eq!(
            resolve_at(&root, Point::new(50.0, 45.0)).map(|h| h.key),
            Some("pane:5".to_string())
        );
    }

    #[test]
    fn resolve_at_returns_the_deepest_target() {
        // Outer target contains an inner target; a hit inside the inner returns it.
        let mut inner = Surface::new().key("pane:2").drop_target();
        inner.base_mut().bounds = Rectangle::new(Point::new(10.0, 10.0), Size::new(30.0, 30.0));
        let mut outer = Surface::new().key("col:1").drop_target();
        outer.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        outer.base_mut().children.push(Box::new(inner));

        assert_eq!(
            resolve_at(&outer, Point::new(20.0, 20.0)).map(|h| h.key),
            Some("pane:2".to_string())
        );
        // Outside the inner but inside the outer → the outer.
        assert_eq!(
            resolve_at(&outer, Point::new(80.0, 80.0)).map(|h| h.key),
            Some("col:1".to_string())
        );
    }

    #[test]
    fn resolve_at_filtered_falls_through_rejected_nested_target_to_accepted_ancestor() {
        // Outer accepted target (1) contains an inner rejected target (2) — like a
        // column MarkerGroup containing pane cards. A hit inside the inner must skip it
        // and resolve to the outer, so dragging a column targets the column, not a pane.
        let mut inner = Surface::new().key("pane:2").drop_target();
        inner.base_mut().bounds = Rectangle::new(Point::new(10.0, 10.0), Size::new(30.0, 30.0));
        let mut outer = Surface::new().key("col:1").drop_target();
        outer.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        outer.base_mut().children.push(Box::new(inner));

        let accept = |key: &str| key != "pane:2";
        assert_eq!(
            resolve_at_filtered(&outer, Point::new(20.0, 20.0), &accept).map(|h| h.key),
            Some("col:1".to_string()),
            "a hit inside the rejected inner target resolves to the accepted outer one",
        );
        // Unfiltered still returns the deepest (inner).
        assert_eq!(
            resolve_at(&outer, Point::new(20.0, 20.0)).map(|h| h.key),
            Some("pane:2".to_string()),
        );
    }

    /// **A drop nobody took reaches the host, with both names already resolved** — the same shape
    /// as an unclaimed right-click, so a row that can be dropped on writes no handler and a
    /// plugin's row writes none either.
    #[test]
    fn an_unclaimed_drop_is_handed_to_the_host() {
        use crate::component::dispatch;
        use crate::event::{Event, PointerButton};
        use crate::widgets::Label;
        use std::cell::RefCell;
        use std::rc::Rc;

        let seen: Rc<RefCell<Vec<crate::drag::Dropped>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = seen.clone();
        crate::drag::install_drop_sink(move |d| sink.borrow_mut().push(d));

        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 80.0));
        let mut a = Surface::new().key("row:a").draggable();
        a.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 40.0));
        a.base_mut().children.push(Box::new(Label::new("A")));
        let mut b = Surface::new().key("row:b").drop_target();
        b.base_mut().bounds = Rectangle::new(Point::new(0.0, 40.0), Size::new(100.0, 40.0));
        root.base_mut().children.push(Box::new(a));
        root.base_mut().children.push(Box::new(b));

        let at = |x: f64, y: f64| Point::new(x, y);
        dispatch(&mut root, &Event::pointer_pressed(at(50.0, 20.0), PointerButton::Left));
        dispatch(&mut root, &Event::pointer_moved(at(50.0, 60.0)));
        dispatch(&mut root, &Event::pointer_released(at(50.0, 60.0), PointerButton::Left));

        let got = seen.borrow();
        assert_eq!(got.len(), 1, "exactly one drop reached the host");
        assert_eq!(got[0].source, "row:a");
        assert_eq!(got[0].target, "row:b");
    }

    /// **`.draggable()` on its own is enough** — a widget that never named itself is still
    /// draggable, because its identity is derived from its content the way the picker's remembered
    /// letters are.
    ///
    /// Requiring a name here would put an internal rule in front of an author before they could
    /// drag anything, and would make `.draggable()` silently do nothing on every widget that had no
    /// reason to be named (Antonio, 2026-09-01: *"this makes developers know about an internal API
    /// not common"*).
    #[test]
    fn an_unnamed_widget_is_still_draggable() {
        use crate::widgets::Label;

        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 40.0));
        let mut row = Surface::new().draggable();
        row.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 40.0));
        row.base_mut().children.push(Box::new(Label::new("Containers")));
        root.base_mut().children.push(Box::new(row));

        assert!(
            source_at(&root, Point::new(50.0, 20.0)).is_some(),
            "an unnamed draggable widget must still be picked up",
        );
    }

    #[test]
    fn resolve_at_misses_outside_all_targets() {
        let mut root = Surface::new().key("col:1").drop_target();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0));
        assert!(resolve_at(&root, Point::new(50.0, 50.0)).is_none());
    }

    #[test]
    fn source_at_finds_the_topmost_drag_source() {
        // Two overlapping sources; the later child (top z-order) wins.
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        root.base_mut().children.push(at(
            Surface::new().key("row:1").draggable(),
            0.0,
            0.0,
            100.0,
            100.0,
        ));
        root.base_mut().children.push(at(
            Surface::new().key("row:2").draggable(),
            0.0,
            0.0,
            100.0,
            100.0,
        ));
        assert_eq!(
            source_at(&root, Point::new(50.0, 50.0)),
            Some("row:2".to_string())
        );
    }

    #[test]
    fn source_at_ignores_drop_only_widgets() {
        let mut root = Surface::new().key("row:9").drop_target();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        assert!(source_at(&root, Point::new(50.0, 50.0)).is_none());
    }
}
