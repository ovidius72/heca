//! **What a raw pointer stream means**, held to the answers the design settled on.
//!
//! The framework resolves the stream once: hit-test, hover transitions, press/release pairing,
//! click runs, drag thresholds. Every widget in the library reads those answers instead of working
//! them out, so a mistake here is a mistake in all of them at once — which is exactly why the
//! rules are written down as tests rather than as comments.
//!
//! The edge cases below are not hypothetical. Each one is a way the hand-rolled versions went
//! wrong: a leave suppressed by a neighbour's click, a first click swallowed while something
//! waited to see whether a second was coming, a right-click that also fired a left one, a press
//! whose widget was gone before the release, a container that ate an event on its way past.

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Dialog, DockFrame, FocusScope, Overlay, ScrollRegion, Select};
use heca_grid_ui::{
    Base, Component, Event, EventKind, Handled, LayoutEngine, Point, PointerButton, Rectangle,
    Size,
};
use std::cell::RefCell;
use std::rc::Rc;

// ── a probe that records the vocabulary ───────────────────────────────────────────────────────

/// Records every resolved event that reaches it, by name. Consumes nothing, so it never changes
/// what would have happened without it.
struct Probe {
    base: Base,
    seen: Seen,
    consume: Option<EventKind>,
}

impl Probe {
    fn new(w: f32, h: f32) -> (Self, Seen) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut base = Base::new();
        base.style.layout.width = Length::Px(w);
        base.style.layout.height = Length::Px(h);
        (
            Self {
                base,
                seen: seen.clone(),
                consume: None,
            },
            seen,
        )
    }

    /// The same probe, but it takes one kind of event — for the tests about what happens after
    /// something claims one.
    fn consuming(mut self, kind: EventKind) -> Self {
        self.consume = Some(kind);
        self
    }
}

impl Component for Probe {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn on_event(&mut self, ev: &Event) -> Handled {
        let name = format!("{:?}", ev.kind());
        self.seen.borrow_mut().push(name);
        if self.consume == Some(ev.kind()) {
            Handled::Yes
        } else {
            Handled::No
        }
    }
}

fn kinds(seen: &Seen) -> Vec<String> {
    seen.borrow().clone()
}

/// The same, without the two events every widget gets whatever happens — mounting, and the moves
/// that carry the pointer around — so a test can state the shape of a gesture rather than a
/// transcript of it.
fn meaningful(seen: &Seen) -> Vec<String> {
    seen.borrow()
        .iter()
        .filter(|k| *k != "Mount" && *k != "PointerMove")
        .cloned()
        .collect()
}

fn press(root: &mut dyn Component, pos: Point) {
    let _ = heca_grid_ui::dispatch(root, &Event::pointer_pressed(pos, PointerButton::Left));
}

fn release(root: &mut dyn Component, pos: Point) {
    let _ = heca_grid_ui::dispatch(root, &Event::pointer_released(pos, PointerButton::Left));
}

fn click(root: &mut dyn Component, pos: Point) {
    press(root, pos);
    release(root, pos);
}

fn mv(root: &mut dyn Component, pos: Point) {
    let _ = heca_grid_ui::dispatch(root, &Event::pointer_moved(pos));
}

/// What a probe records, shared with the test that mounted it.
type Seen = Rc<RefCell<Vec<String>>>;

/// Two probes side by side, laid out: `[0]` at x 0..100, `[1]` at x 100..200, both 0..50 tall.
fn two_probes() -> (Box<dyn Component>, Seen, Seen) {
    let (a, seen_a) = Probe::new(100.0, 50.0);
    let (b, seen_b) = Probe::new(100.0, 50.0);
    let mut root: Box<dyn Component> = Box::new(Flex::row().child(a).child(b));
    LayoutEngine::new().compute(root.as_mut(), Size::new(200.0, 50.0));
    (root, seen_a, seen_b)
}

const LEFT: Point = Point { x: 10.0, y: 10.0 };
const RIGHT: Point = Point { x: 150.0, y: 10.0 };

// ── the vocabulary ────────────────────────────────────────────────────────────────────────────

/// A press and a release on the same widget are a click, and the widget hears the whole story:
/// down, up, click — in that order, all of them targeted at it.
#[test]
fn a_press_and_a_release_on_one_widget_are_a_click() {
    let (mut root, a, b) = two_probes();
    click(root.as_mut(), LEFT);
    assert_eq!(
        meaningful(&a),
        vec!["PointerEnter", "PointerDown", "PointerUp", "Click"],
    );
    assert!(
        meaningful(&b).iter().all(|k| k == "PointerDownOutside"),
        "the widget that was not pressed heard only that a press happened elsewhere: {:?}",
        kinds(&b),
    );
}

/// **Press here, release there, and nothing was clicked.** Dragging off a control before letting
/// go is how a user changes their mind, and it works because the pairing is one rule rather than a
/// habit each widget has to remember.
#[test]
fn a_release_somewhere_else_is_not_a_click() {
    let (mut root, a, b) = two_probes();
    press(root.as_mut(), LEFT);
    release(root.as_mut(), RIGHT);
    assert!(
        !kinds(&a).contains(&"Click".to_string()),
        "no click on the widget the press started on: {:?}",
        kinds(&a),
    );
    assert!(
        !kinds(&b).contains(&"Click".to_string()),
        "and none on the one it ended over: {:?}",
        kinds(&b),
    );
}

/// A right-click is its own event and **does not also fire a left click** — the two mean different
/// things, and a widget that answers one must not accidentally answer both.
#[test]
fn a_right_click_is_a_right_click_and_nothing_else() {
    let (mut root, a, _b) = two_probes();
    let _ = heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::pointer_pressed(LEFT, PointerButton::Right),
    );
    let _ = heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::pointer_released(LEFT, PointerButton::Right),
    );
    let seen = kinds(&a);
    assert!(seen.contains(&"RightClick".to_string()), "{seen:?}");
    assert!(!seen.contains(&"Click".to_string()), "{seen:?}");
}

/// **The first click is never held back.** A widget that understands only single clicks must fire
/// on the first one, so the run adds a `DoubleClick` on top of a `Click` rather than replacing it.
#[test]
fn a_double_click_still_delivers_the_first_click() {
    let (mut root, a, _b) = two_probes();
    click(root.as_mut(), LEFT);
    click(root.as_mut(), LEFT);
    let seen = kinds(&a);
    let clicks = seen.iter().filter(|k| *k == "Click").count();
    assert_eq!(clicks, 2, "both clicks arrived as clicks: {seen:?}");
    assert_eq!(
        seen.iter().filter(|k| *k == "DoubleClick").count(),
        1,
        "and the second one was also a double click: {seen:?}",
    );
    assert!(
        seen.iter().position(|k| k == "DoubleClick") > seen.iter().rposition(|k| k == "Click"),
        "the click comes first, then the double: {seen:?}",
    );
}

/// A click run belongs to **one widget**. Two quick clicks on two different widgets are two first
/// clicks, however fast they follow each other.
#[test]
fn a_run_does_not_carry_from_one_widget_to_another() {
    let (mut root, a, b) = two_probes();
    click(root.as_mut(), LEFT);
    click(root.as_mut(), RIGHT);
    assert!(!kinds(&a).contains(&"DoubleClick".to_string()));
    assert!(
        !kinds(&b).contains(&"DoubleClick".to_string()),
        "the second widget's first click is a first click: {:?}",
        kinds(&b),
    );
}

// ── hover ─────────────────────────────────────────────────────────────────────────────────────

/// Hover follows the pointer: one enter, one leave, and never both widgets lit at once.
#[test]
fn hover_moves_from_one_widget_to_the_other() {
    let (mut root, a, b) = two_probes();
    mv(root.as_mut(), LEFT);
    assert_eq!(meaningful(&a), vec!["PointerEnter"]);
    assert!(meaningful(&b).is_empty(), "nothing happened to the other one");

    mv(root.as_mut(), RIGHT);
    assert_eq!(meaningful(&a), vec!["PointerEnter", "PointerLeave"]);
    assert!(kinds(&b).contains(&"PointerEnter".to_string()));
}

/// **A leave cannot be vetoed.** A widget that consumes the press must not stop the widget it is
/// leaving from being told the pointer has gone — that is how something stays lit for good.
#[test]
fn consuming_a_press_does_not_suppress_a_neighbours_leave() {
    let (a, seen_a) = Probe::new(100.0, 50.0);
    let (b, seen_b) = Probe::new(100.0, 50.0);
    let b = b.consuming(EventKind::PointerDown);
    let mut root: Box<dyn Component> = Box::new(Flex::row().child(a).child(b));
    LayoutEngine::new().compute(root.as_mut(), Size::new(200.0, 50.0));

    mv(root.as_mut(), LEFT);
    assert_eq!(meaningful(&seen_a), vec!["PointerEnter"]);
    // Press on the neighbour, which swallows it, in the same gesture that leaves the first.
    press(root.as_mut(), RIGHT);
    assert!(
        kinds(&seen_a).contains(&"PointerLeave".to_string()),
        "the widget being left heard about it anyway: {:?}",
        kinds(&seen_a),
    );
    assert!(kinds(&seen_b).contains(&"PointerDown".to_string()));
}

/// The hover state is on the widget, and it is **the CSS rule**: a container is hovered while the
/// pointer is over anything inside it.
#[test]
fn a_container_is_hovered_while_its_child_is() {
    let (child, _seen) = Probe::new(100.0, 50.0);
    let mut root = Flex::row().child(child);
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 50.0));

    mv(&mut root, LEFT);
    assert!(root.base().hovered(), "the container is hovered too");
    assert!(root.base().children[0].base().hovered());

    mv(&mut root, Point::new(500.0, 500.0));
    assert!(!root.base().hovered());
    assert!(!root.base().children[0].base().hovered());
}

/// The pointer leaving the window clears everything — hover included. Without it a widget stays
/// lit after the cursor has gone somewhere else entirely.
#[test]
fn leaving_the_window_clears_hover() {
    let (mut root, a, _b) = two_probes();
    mv(root.as_mut(), LEFT);
    assert!(root.base().children[0].base().hovered());
    let _ = heca_grid_ui::dispatch(root.as_mut(), &Event::pointer_cancelled());
    assert!(!root.base().children[0].base().hovered());
    assert!(kinds(&a).contains(&"PointerLeave".to_string()));
}

// ── capture ───────────────────────────────────────────────────────────────────────────────────

/// **Consuming a press captures the pointer**: the moves and the release come to that widget
/// wherever the cursor goes. This is the whole of what a scrollbar thumb needs, and it is why
/// gating a release on position welds one to the cursor.
#[test]
fn a_widget_that_takes_the_press_hears_the_rest_of_the_gesture() {
    let (a, seen_a) = Probe::new(100.0, 50.0);
    let a = a.consuming(EventKind::PointerDown);
    let (b, seen_b) = Probe::new(100.0, 50.0);
    let mut root: Box<dyn Component> = Box::new(Flex::row().child(a).child(b));
    LayoutEngine::new().compute(root.as_mut(), Size::new(200.0, 50.0));

    press(root.as_mut(), LEFT);
    mv(root.as_mut(), Point::new(500.0, 500.0));
    release(root.as_mut(), Point::new(500.0, 500.0));

    let seen = kinds(&seen_a);
    assert!(seen.contains(&"PointerMove".to_string()), "{seen:?}");
    assert!(
        seen.contains(&"PointerUp".to_string()),
        "the release found it far outside its bounds: {seen:?}",
    );
    assert!(
        !kinds(&seen_b).contains(&"PointerUp".to_string()),
        "and nothing else was told the gesture ended",
    );
}

/// A widget that is **gone** by the time the button comes up strands nothing: its state left with
/// it, and the release is simply a release.
#[test]
fn a_widget_removed_between_press_and_release_strands_nothing() {
    let (mut root, _a, _b) = two_probes();
    press(root.as_mut(), LEFT);
    root.base_mut().children.remove(0);
    // Nothing to assert but the absence of a panic and of a click for a widget that is not there.
    release(root.as_mut(), LEFT);
}

// ── the wheel ─────────────────────────────────────────────────────────────────────────────────

/// The wheel carries a position, so it goes to what is under the pointer — and to nothing else.
#[test]
fn the_wheel_goes_to_what_is_under_it() {
    let (mut root, a, b) = two_probes();
    let _ = heca_grid_ui::dispatch(root.as_mut(), &Event::wheel(RIGHT, 0.0, 1.0));
    assert!(!kinds(&a).contains(&"Scroll".to_string()));
    assert!(kinds(&b).contains(&"Scroll".to_string()));
}

/// A region that cannot scroll the axis it was asked for **declines**, and the wheel carries on to
/// the region outside it. That is what makes nesting work with nothing declared.
#[test]
fn an_unscrollable_inner_region_lets_the_wheel_reach_the_outer_one() {
    let inner = ScrollRegion::new()
        .width(Length::Px(100.0))
        .height(Length::Px(50.0))
        .child(Flex::column().width(Length::Px(80.0)).height(Length::Px(20.0)));
    let mut outer = ScrollRegion::new()
        .width(Length::Px(200.0))
        .height(Length::Px(100.0))
        .child(inner)
        .child(Flex::column().width(Length::Px(180.0)).height(Length::Px(600.0)));
    let offset = outer.scroll_offset();
    LayoutEngine::new().compute(&mut outer, Size::new(200.0, 100.0));

    let _ = heca_grid_ui::dispatch(&mut outer, &Event::wheel(Point::new(10.0, 10.0), 0.0, 1.0));
    assert!(
        offset.get_untracked() > 0.0,
        "the inner region had nothing to scroll, so the outer one did",
    );
}

// ── containers on the way past ────────────────────────────────────────────────────────────────

/// **Bubbling survives every container that used to own its own walk.** Each of these once
/// forwarded events to its children by hand, one event kind at a time; a widget inside one has to
/// receive the whole vocabulary, and the container above it has to see what its subtree did.
#[test]
fn every_routing_container_passes_the_whole_vocabulary_through() {
    fn probe_in(wrap: impl FnOnce(Probe) -> Box<dyn Component>) -> Vec<String> {
        let (probe, seen) = Probe::new(100.0, 50.0);
        let mut root = wrap(probe);
        LayoutEngine::new().compute(root.as_mut(), Size::new(400.0, 300.0));
        // Aim at the probe wherever layout put it.
        fn find(c: &dyn Component, w: f32, h: f32) -> Option<Rectangle> {
            let b = c.base().bounds;
            if (b.size.w - w as f64).abs() < 0.5 && (b.size.h - h as f64).abs() < 0.5 {
                return Some(b);
            }
            c.base().children.iter().find_map(|c| find(c.as_ref(), w, h))
        }
        let b = find(root.as_ref(), 100.0, 50.0).expect("the probe was laid out");
        let at = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
        mv(root.as_mut(), at);
        click(root.as_mut(), at);
        let _ = heca_grid_ui::dispatch(root.as_mut(), &Event::wheel(at, 0.0, 1.0));
        kinds(&seen)
    }

    let cases: Vec<(&str, Vec<String>)> = vec![
        ("FocusScope", probe_in(|p| Box::new(FocusScope::new(p)))),
        (
            "DockFrame",
            probe_in(|p| Box::new(DockFrame::new("DOCK").child(p))),
        ),
        (
            "Overlay",
            probe_in(|p| Box::new(Overlay::new().panel(Flex::column().child(p)).open(true))),
        ),
        (
            "Dialog",
            probe_in(|p| Box::new(Dialog::new("T").body(p).open(true))),
        ),
    ];
    for (name, seen) in cases {
        for want in ["PointerEnter", "PointerDown", "PointerUp", "Click", "Scroll"] {
            assert!(
                seen.contains(&want.to_string()),
                "{name} never let {want} through to its child: {seen:?}",
            );
        }
    }
}

/// A widget whose overlay is drawn **on top** takes the click, whatever its place in the child
/// order — what is drawn over a thing is what is clicked.
#[test]
fn an_open_dropdown_takes_a_click_over_a_sibling_drawn_under_it() {
    let (probe, seen) = Probe::new(200.0, 200.0);
    let mut root = Flex::column()
        .child(Select::new(["ONE", "TWO", "THREE"]))
        .child(probe);
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 300.0));

    // Open the list: it drops over the probe below it.
    let trigger = root.base().children[0].base().bounds;
    click(
        &mut root,
        Point::new(trigger.loc.x + 5.0, trigger.loc.y + trigger.size.h / 2.0),
    );
    let before = kinds(&seen).len();
    // A click just under the trigger is on the open list, not on the widget it covers.
    click(
        &mut root,
        Point::new(trigger.loc.x + 5.0, trigger.loc.y + trigger.size.h + 5.0),
    );
    let after = kinds(&seen);
    assert!(
        !after[before..].contains(&"Click".to_string()),
        "the widget under the panel was not clicked through: {after:?}",
    );
}

// ── handlers ──────────────────────────────────────────────────────────────────────────────────

/// The builder side: a handler on any widget, with no widget-specific support for it.
#[test]
fn a_handler_on_any_widget_hears_its_own_clicks() {
    let hits = Rc::new(std::cell::Cell::new(0));
    let h = hits.clone();
    let menus = Rc::new(std::cell::Cell::new(0));
    let m = menus.clone();
    let mut root = Flex::row().child(
        Label::new("plain")
            .width(Length::Px(100.0))
            .height(Length::Px(50.0))
            .on_click(move |_| h.set(h.get() + 1))
            .on_right_click(move |_| m.set(m.get() + 1)),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 50.0));

    click(&mut root, LEFT);
    assert_eq!(hits.get(), 1, "a Label with a handler is a click target");
    let _ = heca_grid_ui::dispatch(&mut root, &Event::pointer_pressed(LEFT, PointerButton::Right));
    let _ = heca_grid_ui::dispatch(
        &mut root,
        &Event::pointer_released(LEFT, PointerButton::Right),
    );
    assert_eq!(menus.get(), 1, "and it owns its own right-click");
    assert_eq!(hits.get(), 1, "which did not also count as a click");
}

/// `stop_propagation` is the named opt-out: the handler takes the event, and the widget's own
/// behaviour does not run.
#[test]
fn stop_propagation_takes_the_event_from_the_widget_itself() {
    let fired = Rc::new(std::cell::Cell::new(false));
    let f = fired.clone();
    let mut button = Button::primary("DEREZ")
        .on_click(move || f.set(true))
        .on(EventKind::Click, |cx| cx.stop_propagation());
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));
    let b = button.base().bounds;
    click(
        &mut button,
        Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0),
    );
    assert!(!fired.get(), "the handler owned the click");
}

/// Mount fires once, on the first layout pass that sees the widget — the first moment it is both
/// in a live tree and somewhere in it.
#[test]
fn mount_fires_once_when_the_tree_is_first_laid_out() {
    let mounted = Rc::new(std::cell::Cell::new(0));
    let m = mounted.clone();
    let mut root = Flex::row().child(Label::new("x").on_mount(move || m.set(m.get() + 1)));
    assert_eq!(mounted.get(), 0, "nothing happens on construction");
    LayoutEngine::new().compute(&mut root, Size::new(100.0, 50.0));
    assert_eq!(mounted.get(), 1);
    LayoutEngine::new().compute(&mut root, Size::new(100.0, 50.0));
    assert_eq!(mounted.get(), 1, "and not again on the next pass");
}

/// Unmount fires when the tree that held the widget is thrown away — the moment to release
/// whatever it registered with the host.
#[test]
fn unmount_fires_when_the_widget_is_dropped() {
    let gone = Rc::new(std::cell::Cell::new(false));
    let g = gone.clone();
    {
        let _root = Flex::row().child(Label::new("x").on_unmount(move || g.set(true)));
        assert!(!gone.get());
    }
    assert!(gone.get(), "the rebuilt-away tree said so on its way out");
}

/// Focus is an **event**, not only a signal: the moment it arrives and the moment it leaves are
/// both things a widget can act on.
#[test]
fn focus_and_blur_reach_their_widget() {
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
    let (a, b) = (log.clone(), log.clone());
    let mut root = Flex::row()
        .child(
            Button::primary("A")
                .on_focus_gained(move || a.borrow_mut().push("focus"))
                .on_focus_lost(move || b.borrow_mut().push("blur")),
        )
        .child(Button::primary("B"));
    LayoutEngine::new().compute(&mut root, Size::new(400.0, 80.0));

    let mut focus = FocusManager::new();
    focus.advance(&mut root, true);
    focus.advance(&mut root, true);
    assert_eq!(*log.borrow(), vec!["focus", "blur"]);
}

// ── which button a control owns ───────────────────────────────────────────────────────────────

/// **A control claims the primary press and nothing else.** A right-click has to reach whatever
/// answers one — the widget itself, or the host behind it — and a control that swallowed every
/// button is how a right-click on a sidebar row opened no menu at all while the pane behind it
/// worked fine.
#[test]
fn a_control_takes_the_left_press_and_lets_every_other_button_past() {
    let mut root = Flex::row().child(
        Button::primary("DEREZ")
            .width(Length::Px(100.0))
            .height(Length::Px(50.0)),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 50.0));

    assert_eq!(
        heca_grid_ui::dispatch(&mut root, &Event::pointer_pressed(LEFT, PointerButton::Left)),
        Handled::Yes,
        "the button owns the press that would activate it",
    );
    let _ = heca_grid_ui::dispatch(&mut root, &Event::pointer_released(LEFT, PointerButton::Left));
    assert_eq!(
        heca_grid_ui::dispatch(&mut root, &Event::pointer_pressed(LEFT, PointerButton::Right)),
        Handled::No,
        "a right press carries on to whoever answers right-clicks",
    );
}

/// The claim is a **declaration**, not nine copies of a match arm: it lives on `Base`, the router
/// applies it, and a widget that declares it never writes the claim out.
#[test]
fn one_click_target_is_declared_not_written_out() {
    let button = Button::primary("A");
    let row = Row::new().on_activate(|| {});
    let plain = Label::new("text");
    assert!(button.base().one_click_target);
    assert!(row.base().one_click_target, "an interactive row is one too");
    assert!(
        !plain.base().one_click_target,
        "and a plain leaf claims nothing",
    );
}

/// **Click away to close.** A press that lands anywhere else reaches an open menu through its own
/// hooks — the same two the widget uses for everything — so a popup dismisses itself with no
/// geometry of its own and nothing for the host to arrange.
#[test]
fn a_press_outside_an_open_menu_dismisses_it() {
    use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
    let dismissed = Rc::new(std::cell::Cell::new(false));
    let d = dismissed.clone();
    let mut menu = ContextMenu::new("test-menu")
        .child(Menu::new("Test", "a menu").child(MenuItem::new().label("Close").on_click(|| {})))
        .anchor(Point::new(40.0, 40.0))
        .on_dismiss(move || d.set(true))
        .open(true);
    LayoutEngine::new().compute(&mut menu, Size::new(400.0, 300.0));
    // Paint once so the menu caches a viewport and places its panel, as a host frame would.
    let theme = Theme::default();
    let mut scene = heca_grid_ui::Scene::new();
    {
        let mut cx = heca_grid_ui::PaintCx::new(&mut scene, &theme)
            .with_viewport(Size::new(400.0, 300.0));
        menu.paint(&mut cx);
    }

    press(&mut menu, Point::new(380.0, 290.0));
    assert!(dismissed.get(), "a press away from the panel closed it");
}
