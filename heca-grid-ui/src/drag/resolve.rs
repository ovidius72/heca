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

/// **What a drop does** — the drag API's own answer, so nobody works it out twice.
///
/// A drag has always had two meanings: put this *where* I am pointing, or *exchange* it with what
/// is there. Which one it is comes from the modifiers, and the rule was written in two places at
/// once — the framework painted a swap outline on Shift, and the host separately read Shift off the
/// drop to decide what to do. Two copies of one convention, free to disagree, and they did: the
/// outline promised a swap while the drop performed a move (Antonio, driving, 2026-09-02).
///
/// So the rule lives here, where the gesture already lives. Ask [`DropAction::held`] or read it off
/// the [`Dropped`](crate::drag::Dropped) — never re-derive it from a modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropAction {
    /// Put the dragged thing at the drop position, taking it out of where it was.
    Move,
    /// Exchange the dragged thing with what it landed on; both keep a place.
    Swap,
}

type SwapRule = Box<dyn Fn(crate::event::Modifiers) -> bool>;

thread_local! {
    /// How the host spells "swap". `None` until it says, and Shift until then.
    static SWAP_RULE: std::cell::RefCell<Option<SwapRule>> =
        const { std::cell::RefCell::new(None) };
}

/// **Say which modifier means swap.** Call once at startup, like the sinks beside it.
///
/// A modifier is a *binding*, and a binding is data a user sets — so it cannot be a constant
/// compiled into a widget library. Shift is only the default, and a host whose own drag gesture
/// already claims Shift needs to be able to say so rather than live with two meanings on one key.
///
/// ```ignore
/// heca_grid_ui::drag::set_swap_rule(|m| m.alt);
/// ```
pub fn set_swap_rule(rule: impl Fn(crate::event::Modifiers) -> bool + 'static) {
    SWAP_RULE.with(|r| *r.borrow_mut() = Some(Box::new(rule)));
}

impl DropAction {
    /// What the modifiers currently held mean for a drop — **the one place that is decided**, for
    /// the framework's own drag feedback and for whatever the host does with the drop.
    pub fn held() -> Self {
        let swap = SWAP_RULE.with(|r| match r.borrow().as_ref() {
            Some(rule) => rule(crate::event::modifiers()),
            None => crate::event::modifiers().shift,
        });
        match swap {
            true => Self::Swap,
            false => Self::Move,
        }
    }

    /// Whether this is a swap, for a caller that wants a bool.
    pub fn is_swap(self) -> bool {
        matches!(self, Self::Swap)
    }
}

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
    /// [`ComponentExt::key`](crate::builders::ComponentExt::key), or one derived from its content.
    /// This is what a **host** reads: it says *what* was landed on, and a pane is the same pane
    /// whichever seating you dragged it in.
    pub key: String,
    /// **Which node it is** — the path the walk found it at.
    ///
    /// Carried because the walk already knows, and searching for it again by name is wrong: the
    /// same container seated twice gives two rows the same name, so the search finds whichever
    /// comes first and lights up the wrong sidebar (Antonio, driving, 2026-09-01). *What* was
    /// landed on and *which node* it is are two questions; one string cannot answer both.
    pub path: Vec<usize>,
    /// The target's laid-out bounds (logical px) — paint the indicator against these.
    pub bounds: Rectangle,
    /// Which side of the target the cursor is over.
    pub side: DropSide,
}

/// Classify `point` against `bounds` into a [`DropSide`] by vertical thirds
/// (the common vertically-stacked list case).
fn side_for(bounds: Rectangle, point: Point, onto: bool) -> DropSide {
    let dy = point.y - bounds.loc.y;
    if !onto {
        // A sibling in an ordered list: two halves, and the line flips at the midpoint. There is no
        // third band, because "onto" would have to silently mean one of the other two.
        return match dy < bounds.size.h / 2.0 {
            true => DropSide::Before,
            false => DropSide::After,
        };
    }
    let third = bounds.size.h / 3.0;
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

/// [`resolve_at`], for a drag that says **what it is** — a target that does not
/// [`accept`](crate::builders::ComponentExt::accepts) that word is skipped and the walk keeps
/// going outward, so the drop lands on the nearest thing that would actually take it and nothing
/// else is ever drawn on.
pub fn resolve_at_for(root: &dyn Component, point: Point, kind: Option<&str>) -> Option<DropHit> {
    let mut path = Vec::new();
    drop_at(root, root, point, &|_| true, kind, &mut path)
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
    drop_at(root, root, point, accept, None, &mut path)
}

/// The recursive half of [`resolve_at_filtered`], carrying the path so a hit can be named by
/// [`identity_of`](crate::nav::identity_of) — which needs the whole chain from the root to derive a
/// name for a widget that declared none.
fn drop_at(
    root: &dyn Component,
    node: &dyn Component,
    point: Point,
    accept: &dyn Fn(&str) -> bool,
    kind: Option<&str>,
    path: &mut Vec<usize>,
) -> Option<DropHit> {
    // **What is being carried is not somewhere to put it.** Dropping a thing on itself — or on
    // anything inside it — can only mean nothing happens, and a target that will do nothing must
    // not light up as though it will (Antonio, driving, 2026-09-01). Skipping the whole subtree is
    // deliberate: a column dropped on one of its own panes is the same no-op.
    if skip(node) || node.base().pointer.is_dragging() {
        return None;
    }
    for (i, child) in node.base().children.iter().enumerate().rev() {
        path.push(i);
        if let Some(hit) = drop_at(root, child.as_ref(), point, accept, kind, path) {
            return Some(hit);
        }
        path.pop();
    }
    if node.is_drop_target()
        && node.accepts_drag(kind)
        && let Some(key) = drag_identity(root, node, path)
        && accept(&key)
    {
        let bounds = node.base().bounds;
        if bounds.contains(point) {
            return Some(DropHit {
                key,
                path: path.clone(),
                bounds,
                side: side_for(bounds, point, node.base().accepts_onto),
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
    /// **Which modifier means swap is the host's to say.** It is a binding, and a binding is data
    /// a user sets — so it cannot be a constant compiled into a widget library. Shift is only the
    /// default.
    #[test]
    fn the_host_says_which_modifier_means_swap() {
        use crate::component::dispatch;
        use crate::event::{Event, Modifiers};

        let mut root = Flex::column();
        let mut hold = |m: Modifiers| {
            dispatch(&mut root, &Event::ModifiersChanged(m));
            DropAction::held()
        };
        let shift = Modifiers { shift: true, ..Modifiers::default() };
        let alt = Modifiers { alt: true, ..Modifiers::default() };

        assert_eq!(hold(shift), DropAction::Swap, "Shift by default");

        crate::drag::set_swap_rule(|m| m.alt);
        assert_eq!(hold(alt), DropAction::Swap, "the host moved swap onto Alt");
        assert_eq!(hold(shift), DropAction::Move, "and Shift stopped meaning it");

        // A host whose own gesture claims every candidate can say nothing means swap.
        crate::drag::set_swap_rule(|_| false);
        assert_eq!(hold(shift), DropAction::Move);
        assert_eq!(hold(alt), DropAction::Move);

        crate::drag::set_swap_rule(|m| m.shift);
    }

    /// **The outline and the drop must never disagree.** Move-versus-swap is one decision, so the
    /// picture the framework paints and the answer the host acts on come from the same place.
    ///
    /// They did disagree: the framework painted a swap outline when Shift was down while the host
    /// separately read the modifier off the drop — and the host's copy was reached through a path
    /// that had lost the modifiers entirely, so the outline promised a swap and the drop performed
    /// a move (Antonio, driving, 2026-09-02).
    #[test]
    fn what_a_drop_does_is_decided_in_one_place() {
        use crate::component::dispatch;
        use crate::event::{Event, Modifiers};

        let mut root = Flex::column();
        let mut held = |shift: bool| {
            dispatch(
                &mut root,
                &Event::ModifiersChanged(Modifiers { shift, ..Modifiers::default() }),
            );
            DropAction::held()
        };

        assert_eq!(held(true), DropAction::Swap, "Shift down means swap");
        assert!(held(true).is_swap());
        assert_eq!(held(false), DropAction::Move, "Shift up means move");
        assert!(!held(false).is_swap());
    }

    /// **A pointer event does not carry the modifiers; the framework fills them in.**
    ///
    /// The host has more than one place it feeds a tree from, and "remember to attach the
    /// modifiers" is a rule some of them always forget — silently, because an event built without
    /// them says "nothing held", which is a real answer. So a drop read as a plain move however
    /// hard Shift was pressed, and every test that built its own event still passed.
    #[test]
    fn a_pointer_event_built_without_modifiers_still_knows_what_is_held() {
        use crate::component::dispatch;
        use crate::event::{Event, Modifiers, PointerButton};

        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 80.0));
        dispatch(
            &mut root,
            &Event::ModifiersChanged(Modifiers { shift: true, ..Modifiers::default() }),
        );

        // Built the way every host convenience constructor builds one: no modifiers attached.
        let ev = Event::pointer_pressed(Point::new(50.0, 40.0), PointerButton::Left);
        dispatch(&mut root, &ev);

        assert_eq!(
            DropAction::held(),
            DropAction::Swap,
            "an event that carried no modifiers must not erase what the host announced",
        );
    }

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

    /// **Nothing hovers under a drag.** A row lit as the pointer crossed it reads as "you may drop
    /// here", while the drop is decided by the rules and lands elsewhere.
    #[test]
    fn a_drag_in_flight_clears_hover() {
        use crate::component::dispatch;
        use crate::event::{Event, PointerButton};

        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 80.0));
        let mut a = Surface::new().key("row:a").draggable();
        a.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 40.0));
        let mut b = Surface::new().key("row:b").drop_target();
        b.base_mut().bounds = Rectangle::new(Point::new(0.0, 40.0), Size::new(100.0, 40.0));
        root.base_mut().children.push(Box::new(a));
        root.base_mut().children.push(Box::new(b));

        let at = |y: f64| Point::new(50.0, y);
        dispatch(&mut root, &Event::pointer_moved(at(60.0)));
        assert!(
            root.base().children[1].base().pointer.is_hovered(),
            "with no drag, the row under the pointer hovers",
        );

        dispatch(&mut root, &Event::pointer_pressed(at(20.0), PointerButton::Left));
        dispatch(&mut root, &Event::pointer_moved(at(60.0)));
        assert!(
            !root.base().children[1].base().pointer.is_hovered(),
            "while a drag is in flight nothing under the pointer may light up",
        );
    }

    /// **What is being carried is not somewhere to put it**, and neither is anything inside it.
    ///
    /// Dropping a column on itself or on one of its own panes can only mean nothing happens, and a
    /// target that will do nothing must not light up as though it will. Skipping the subtree lets
    /// the walk reach what encloses it — the workspace — which is a real place to drop it.
    #[test]
    fn the_thing_being_dragged_is_not_a_target_for_itself() {
        use crate::component::dispatch;
        use crate::event::{Event, PointerButton};

        let mut ws = Surface::new().key("ws:0").accepts(["column"]);
        ws.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        let mut col = Surface::new()
            .key("col:1")
            .draggable_as("column")
            .accepts_beside(["column"]);
        col.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 60.0));
        ws.base_mut().children.push(Box::new(col));

        // Not dragging: the column is the nearest target under the point.
        assert_eq!(
            resolve_at_for(&ws, Point::new(50.0, 30.0), Some("column")).map(|h| h.key),
            Some("col:1".to_string()),
        );

        // Picked up and moved: it is no longer a place to put itself, so the walk reaches the
        // workspace that holds it.
        dispatch(&mut ws, &Event::pointer_pressed(Point::new(50.0, 30.0), PointerButton::Left));
        dispatch(&mut ws, &Event::pointer_moved(Point::new(50.0, 50.0)));
        assert_eq!(
            resolve_at_for(&ws, Point::new(50.0, 30.0), Some("column")).map(|h| h.key),
            Some("ws:0".to_string()),
        );
    }

    /// **A reorder has no middle.** A target that takes something *beside* it reads as two halves,
    /// so the answer flips at the midpoint and is never a silent third choice.
    #[test]
    fn a_sibling_target_is_two_halves_and_a_container_is_three_bands() {
        let mut sibling = Surface::new().key("col:1").accepts_beside(["column"]);
        sibling.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(90.0, 90.0));
        for (y, want) in [(20.0, DropSide::Before), (44.0, DropSide::Before), (46.0, DropSide::After), (70.0, DropSide::After)] {
            assert_eq!(
                resolve_at_for(&sibling, Point::new(45.0, y), Some("column")).map(|h| h.side),
                Some(want),
                "a sibling at y={y} flips at the midpoint",
            );
        }

        let mut container = Surface::new().key("pane:1").accepts(["pane"]);
        container.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(90.0, 90.0));
        assert_eq!(
            resolve_at_for(&container, Point::new(45.0, 45.0), Some("pane")).map(|h| h.side),
            Some(DropSide::Onto),
            "a container keeps a real middle — a pane dropped onto a pane means something",
        );
    }


    /// **A target that will not take it is never offered** — so a line is never drawn over
    /// something a release would then ignore.
    ///
    /// Antonio, driving 2026-09-01: a column could be dropped above a pane, the insertion line
    /// appeared, and releasing did nothing. The drawing and the rule lived in different places —
    /// the widget drew, and the app refused afterwards. A target says what it takes, and the walk
    /// keeps going outward past one that does not.
    #[test]
    fn a_target_that_refuses_the_kind_is_skipped_for_the_one_that_takes_it() {
        let mut outer = Surface::new().key("col:1").accepts(["column"]);
        outer.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        let mut inner = Surface::new().key("pane:2").accepts(["pane"]);
        inner.base_mut().bounds = Rectangle::new(Point::new(10.0, 10.0), Size::new(30.0, 30.0));
        outer.base_mut().children.push(Box::new(inner));

        // Dragging a column over the pane card lands on the column that contains it.
        assert_eq!(
            resolve_at_for(&outer, Point::new(20.0, 20.0), Some("column")).map(|h| h.key),
            Some("col:1".to_string()),
        );
        // Dragging a pane over the same point lands on the pane card itself.
        assert_eq!(
            resolve_at_for(&outer, Point::new(20.0, 20.0), Some("pane")).map(|h| h.key),
            Some("pane:2".to_string()),
        );
        // A drag that says nothing about itself is taken by the nearest target, as before.
        assert_eq!(
            resolve_at_for(&outer, Point::new(20.0, 20.0), None).map(|h| h.key),
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
