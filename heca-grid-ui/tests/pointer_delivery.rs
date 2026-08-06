//! **Every container must hand its children the whole pointer set.**
//!
//! This exists because one omission has now shipped three times, in three different surfaces, and
//! it is invisible every time: a widget with a *gesture* still lays out, still paints and still
//! hit-tests perfectly while being unusable.
//!
//! A [`ScrollRegion`](heca_grid_ui::ScrollRegion) is the widget that shows it, because it needs
//! two things beyond a press:
//!
//! - **`Scroll`** — or the wheel does nothing at all.
//! - **`PointerUp`** — or a thumb drag never ends, and the thumb stays welded to the cursor.
//!
//! A container that forwards the move and the press and stops there looks completely
//! correct. The region is mounted, sized, drawn, and dead. Nothing failed, so nothing was noticed
//! until someone tried to scroll — and the fix went into that one container, leaving every other
//! one to be discovered the same way later.
//!
//! So the rule is written down as a test instead of as a habit: mount a probe where a scroll region
//! would go, in each container that takes part in dispatch, and require all four kinds to arrive. A
//! container that forwards a subset fails here rather than in the app, months later.
//!
//! **Since F004/P084/T394 no container forwards anything**: the framework hit-tests the pointer and
//! carries it to the target and back up through whatever contains it, so the omission this file
//! guards against is no longer a line anyone can write. The test stays: it is what proves that is
//! still true, in each of the containers that used to own the walk.

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Dialog, FocusScope, Overlay, ScrollRegion};
use heca_grid_ui::{Base, Component, Event, Handled, LayoutEngine, Point, Rectangle, Size};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The four pointer events any tree must receive. Keys are not here: they are focus-routed and a
/// widget that wants none is correct to get none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Moved,
    Pressed,
    Released,
    Wheel,
}

const REQUIRED: [Kind; 4] = [Kind::Moved, Kind::Pressed, Kind::Released, Kind::Wheel];

/// What a probe hands back: itself, the log of what reached it, and where layout put it.
type Probed = (Probe, Rc<RefCell<Vec<Kind>>>, Rc<Cell<Rectangle>>);

/// Stands in for a scroll region: records what reached it, and where it ended up.
struct Probe {
    base: Base,
    seen: Rc<RefCell<Vec<Kind>>>,
    bounds: Rc<Cell<Rectangle>>,
}

impl Probe {
    fn new() -> Probed {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let bounds = Rc::new(Cell::new(Rectangle::from_size(Size::new(0.0, 0.0))));
        let mut base = Base::new();
        base.style.layout.width = Length::Px(120.0);
        base.style.layout.height = Length::Px(80.0);
        (
            Self { base, seen: seen.clone(), bounds: bounds.clone() },
            seen,
            bounds,
        )
    }
}

impl Component for Probe {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    /// Post-order, so this is the probe's real laid-out rect wherever the host put it — the same
    /// hook a `ScrollRegion` uses to re-place its content.
    fn on_layout(&mut self) {
        self.bounds.set(self.base.bounds);
    }
    /// The probe is a leaf: `on_event` is where a widget says what it does with what nobody
    /// below wanted, and a leaf has nobody below.
    fn on_event(&mut self, ev: &Event) -> Handled {
        let kind = match ev {
            Event::PointerMove(_) => Some(Kind::Moved),
            Event::PointerDown(_) => Some(Kind::Pressed),
            Event::PointerUp(_) => Some(Kind::Released),
            Event::Scroll(_) => Some(Kind::Wheel),
            _ => None,
        };
        if let Some(kind) = kind {
            self.seen.borrow_mut().push(kind);
        }
        // `No` on purpose: "it arrived" is the claim, and a widget that ignores an event must not
        // stop its siblings from seeing it.
        Handled::No
    }
}

/// Lay `host` out, then aim the four pointer events at the probe's own centre.
fn deliver(mut host: Box<dyn Component>, bounds: &Rc<Cell<Rectangle>>) {
    LayoutEngine::new().compute(host.as_mut(), Size::new(600.0, 400.0));
    let b = bounds.get();
    let pos = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let _ = heca_grid_ui::dispatch(host.as_mut(), &Event::pointer_moved(pos));
    let _ = heca_grid_ui::dispatch(host.as_mut(), &Event::pointer_pressed(pos, PointerButton::Left));
    let _ = heca_grid_ui::dispatch(host.as_mut(), &Event::pointer_released(pos, PointerButton::Left));
    let _ = heca_grid_ui::dispatch(host.as_mut(), &Event::wheel(pos, 0.0, 1.0));
}

fn assert_full_set(
    name: &str,
    host: Box<dyn Component>,
    seen: &Rc<RefCell<Vec<Kind>>>,
    bounds: &Rc<Cell<Rectangle>>,
) {
    deliver(host, bounds);
    let seen = seen.borrow();
    for kind in REQUIRED {
        assert!(
            seen.contains(&kind),
            "{name} never delivered {kind:?} to its child.\n\
             A container owes its children the WHOLE pointer set: a scroll region mounted here \
             would be silently broken — no wheel, or a thumb drag that never ends. Forward it in \
             {name}'s hooks, next to the kinds it already handles.\n\
             got: {seen:?}",
        );
    }
}

#[test]
fn a_plain_container_delivers_the_whole_pointer_set() {
    let (probe, seen, bounds) = Probe::new();
    assert_full_set("Flex", Box::new(Flex::column().child(probe)), &seen, &bounds);
}

/// The modal case: a `Dialog` body is where a described scroll region actually lands.
#[test]
fn a_dialog_delivers_the_whole_pointer_set_to_its_body() {
    let (probe, seen, bounds) = Probe::new();
    let dialog = Dialog::new("Pick one").body(probe).open(true);
    assert_full_set("Dialog", Box::new(dialog), &seen, &bounds);
}

/// The layer a host mounts a realized panel into, without a `Dialog` around it.
#[test]
fn an_overlay_delivers_the_whole_pointer_set_to_its_panel() {
    let (probe, seen, bounds) = Probe::new();
    let overlay = Overlay::new().panel(probe).open(true);
    assert_full_set("Overlay", Box::new(overlay), &seen, &bounds);
}

/// A [`FocusScope`] claims `routes_own_subtree` so it can gate **keys** on focus — which puts it
/// on the hook for the pointer, and the case that matters is the **unfocused** one: a click on a
/// dock that does not have focus is exactly how you give it focus, and a scroll region inside it
/// must still get its wheel and its release.
#[test]
fn an_unfocused_focus_scope_delivers_the_whole_pointer_set() {
    let (probe, seen, bounds) = Probe::new();
    let scope = FocusScope::new(probe).focus(signal(false));
    assert_full_set("FocusScope (unfocused)", Box::new(scope), &seen, &bounds);
}

/// And focused, obviously — same wrapper, same duty.
#[test]
fn a_focused_focus_scope_delivers_the_whole_pointer_set() {
    let (probe, seen, bounds) = Probe::new();
    let scope = FocusScope::new(probe).focus(signal(true));
    assert_full_set("FocusScope (focused)", Box::new(scope), &seen, &bounds);
}

/// A region inside a region: the outer one must not eat what the inner one needs.
#[test]
fn a_scroll_region_delivers_the_whole_pointer_set_to_its_children() {
    let (probe, seen, bounds) = Probe::new();
    let outer = ScrollRegion::new()
        .width(Length::Px(200.0))
        .height(Length::Px(140.0))
        .child(probe);
    assert_full_set("ScrollRegion", Box::new(outer), &seen, &bounds);
}

// ── The behaviour the set exists for ────────────────────────────────────────────────────────

/// A region whose content overflows scrolls on the wheel — which now needs no move first, because
/// the wheel carries its own position and is routed by it.
#[test]
fn the_wheel_scrolls_the_region_it_is_over() {
    let mut region = ScrollRegion::new()
        .width(Length::Px(200.0))
        .height(Length::Px(100.0))
        .child(Flex::column().width(Length::Px(180.0)).height(Length::Px(600.0)));
    let offset = region.scroll_offset();

    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
    heca_grid_ui::dispatch(&mut region, &Event::wheel(Point::new(100.0, 50.0), 0.0, 1.0));

    assert!(offset.get_untracked() > 0.0, "the wheel scrolled it");
}

/// A thumb drag ends on the release **wherever the release lands** — the cursor has usually left
/// the region by then, and a bounds check here is exactly how a drag gets stuck.
#[test]
fn a_thumb_drag_ends_on_a_release_outside_the_region() {
    let mut region = ScrollRegion::new()
        .width(Length::Px(200.0))
        .height(Length::Px(100.0))
        .child(Flex::column().width(Length::Px(180.0)).height(Length::Px(600.0)));
    let offset = region.scroll_offset();
    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));

    // Grab the thumb in its lane at the right edge, drag down.
    heca_grid_ui::dispatch(&mut region, &Event::pointer_pressed(Point::new(196.0, 10.0), PointerButton::Left));
    heca_grid_ui::dispatch(&mut region, &Event::pointer_moved(Point::new(196.0, 60.0)));
    let dragged = offset.get_untracked();
    assert!(dragged > 0.0, "the drag scrolled it");

    // Release far outside, then keep moving: the thumb must not follow any more.
    heca_grid_ui::dispatch(&mut region, &Event::pointer_released(Point::new(900.0, 900.0), PointerButton::Left));
    heca_grid_ui::dispatch(&mut region, &Event::pointer_moved(Point::new(196.0, 95.0)));
    assert_eq!(
        offset.get_untracked(),
        dragged,
        "the drag ended: further motion is not still dragging the thumb",
    );
}

/// And if a host loses the release anyway, the next press recovers — a stuck thumb can survive one
/// click, never the session.
#[test]
fn a_press_recovers_a_grab_whose_release_never_arrived() {
    let mut region = ScrollRegion::new()
        .width(Length::Px(200.0))
        .height(Length::Px(100.0))
        .child(Flex::column().width(Length::Px(180.0)).height(Length::Px(600.0)));
    let offset = region.scroll_offset();
    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));

    heca_grid_ui::dispatch(&mut region, &Event::pointer_pressed(Point::new(196.0, 10.0), PointerButton::Left));
    heca_grid_ui::dispatch(&mut region, &Event::pointer_moved(Point::new(196.0, 60.0)));
    // No release — the host dropped it.
    heca_grid_ui::dispatch(&mut region, &Event::pointer_pressed(Point::new(20.0, 20.0), PointerButton::Left));
    let after = offset.get_untracked();
    heca_grid_ui::dispatch(&mut region, &Event::pointer_moved(Point::new(196.0, 95.0)));

    assert_eq!(offset.get_untracked(), after, "the stale grab did not survive the next press");
}

// ── Following the cursor, and not fighting the wheel ─────────────────────────────────────────

/// A row that says it is the navigation cursor — the sidebar's `Row`/`MarkerGroup`/`DockFrame` do
/// exactly this, so the rule reaches them without either side wiring anything.
struct Marked {
    base: Base,
    current: Rc<Cell<bool>>,
}

impl Component for Marked {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn wants_visible(&self) -> bool {
        self.current.get()
    }
}

fn marked_row(current: &Rc<Cell<bool>>) -> Marked {
    let mut base = Base::new();
    base.style.layout.height = Length::Px(40.0);
    base.style.layout.width = Length::Px(180.0);
    Marked { base, current: current.clone() }
}

/// **A scroll region follows the cursor, in every region, forever.** The host wires nothing: the
/// widget says it is current, the region brings it into view. This is the keyboard half of
/// scrolling, and it used to be a call each list had to remember to make.
#[test]
fn a_region_scrolls_to_a_descendant_that_asks_to_be_visible() {
    let off_screen = Rc::new(Cell::new(false));
    let mut region = ScrollRegion::new()
        .width(Length::Px(200.0))
        .height(Length::Px(100.0));
    for _ in 0..4 {
        region = region.child(marked_row(&Rc::new(Cell::new(false))));
    }
    region = region.child(marked_row(&off_screen)); // the last row, well below the fold
    let offset = region.scroll_offset();

    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
    assert_eq!(offset.get_untracked(), 0.0, "nothing is current yet");

    off_screen.set(true);
    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
    assert!(offset.get_untracked() > 0.0, "the region came to the cursor");
}

/// And having followed it, it stops: scrolling away by hand must not snap back, or the view would
/// fight the user for as long as the selection sat still.
#[test]
fn following_the_cursor_does_not_fight_the_wheel() {
    let current = Rc::new(Cell::new(true));
    let mut region = ScrollRegion::new()
        .width(Length::Px(200.0))
        .height(Length::Px(100.0))
        .child(marked_row(&Rc::new(Cell::new(false))))
        .child(marked_row(&Rc::new(Cell::new(false))))
        .child(marked_row(&current));
    let offset = region.scroll_offset();
    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
    let followed = offset.get_untracked();
    assert!(followed > 0.0, "it followed the cursor first");

    region.scroll_to(0.0); // the user scrolls back up; the selection has not moved
    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
    assert_eq!(offset.get_untracked(), 0.0, "it stayed where the user put it");
}

/// Hovering a scrollbar lane must not light up the row behind it — the lane owns the pointer while
/// the cursor is over it.
#[test]
fn the_scrollbar_lane_does_not_leak_hover_to_the_content_behind_it() {
    let (probe, seen, _bounds) = Probe::new();
    let mut region = ScrollRegion::new()
        .width(Length::Px(200.0))
        .height(Length::Px(100.0))
        .child(Flex::column().width(Length::Px(180.0)).height(Length::Px(600.0)).child(probe));
    LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));

    // Over the content: the probe hears about it.
    heca_grid_ui::dispatch(&mut region, &Event::pointer_moved(Point::new(40.0, 50.0)));
    let after_content = seen.borrow().len();
    assert!(after_content > 0, "a move over the content reaches it");

    // Over the thumb lane at the right edge: it does not.
    heca_grid_ui::dispatch(&mut region, &Event::pointer_moved(Point::new(196.0, 50.0)));
    assert_eq!(
        seen.borrow().len(),
        after_content,
        "a move over the scrollbar belongs to the scrollbar",
    );
}
