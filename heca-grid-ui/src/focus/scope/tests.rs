use super::*;
use crate::builders::{ComponentExt, LayoutExt, Parent};
use crate::component::{Base, Event, Handled};
use crate::event::PointerButton;
use crate::focus::keyboard_owner;
use crate::layout::LayoutEngine;
use crate::reactive::{SignalUpdate, signal};
use crate::style::Length;
use crate::widgets::{Flex, ScrollRegion};
use heca_core::layout::{Point, Size};

/// A widget the keyboard can land on.
struct Control {
    base: Base,
}

impl Control {
    fn new() -> Self {
        let mut base = Base::new();
        base.focusable = true;
        base.style.layout.width = Length::Px(40.0);
        base.style.layout.height = Length::Px(20.0);
        Self { base }
    }
}

impl Component for Control {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn on_event(&mut self, _: &Event) -> Handled {
        Handled::No
    }
}

/// A dock: a named region holding one control.
fn dock(name: &str) -> Flex {
    Flex::column().child(Control::new()).scope_key(name)
}

/// Two docks side by side, `a` then `b`.
fn window() -> Flex {
    Flex::row().child(dock("a")).child(dock("b"))
}

fn holder(root: &dyn Component) -> Option<Vec<usize>> {
    keyboard_owner(root).map(|owner| owner.path)
}

#[test]
fn focusing_a_region_gives_it_the_keyboard() {
    let mut root = window();
    assert!(focus_scope(&mut root, "b"));
    assert_eq!(page_scope(&root).as_deref(), Some("b"));
    assert!(contains_keyboard(root.base().children[1].as_ref()));
    assert!(!contains_keyboard(root.base().children[0].as_ref()));
}

#[test]
fn focusing_another_region_takes_it_from_the_first() {
    let mut root = window();
    focus_scope(&mut root, "a");
    focus_scope(&mut root, "b");
    assert_eq!(page_scope(&root).as_deref(), Some("b"));
    assert!(
        !contains_keyboard(root.base().children[0].as_ref()),
        "one keyboard per tree: the first dock let go"
    );
}

#[test]
fn a_name_nothing_answers_to_leaves_the_keyboard_where_it_was() {
    let mut root = window();
    focus_scope(&mut root, "a");
    assert!(!focus_scope(&mut root, "nowhere"));
    assert!(!release_scope(&mut root, "nowhere"));
    assert_eq!(page_scope(&root).as_deref(), Some("a"));
}

#[test]
fn a_region_that_is_not_shown_cannot_take_the_keyboard() {
    let mut root = window();
    root.base().children[1].base().visible.set(false);
    assert!(!focus_scope(&mut root, "b"));
    assert_eq!(page_scope(&root), None);
}

/// A dock told to take the keyboard does not steal it from the control the user just clicked in.
#[test]
fn a_control_inside_that_already_has_the_keyboard_keeps_it() {
    let mut root = window();
    root.base().children[0].base().children[0]
        .base()
        .focus(false);
    crate::settle_focus(&mut root);
    let before = holder(&root);
    assert!(focus_scope(&mut root, "a"));
    assert_eq!(holder(&root), before, "the control still holds it");
    assert_eq!(
        page_scope(&root).as_deref(),
        Some("a"),
        "and a control inside the dock is the dock holding the keyboard"
    );
}

#[test]
fn releasing_a_region_leaves_the_keyboard_with_nobody() {
    let mut root = window();
    focus_scope(&mut root, "a");
    assert!(release_scope(&mut root, "a"));
    assert_eq!(page_scope(&root), None);
    assert!(holder(&root).is_none());
}

/// Releasing a region that does not hold the keyboard is harmless.
#[test]
fn releasing_a_region_the_keyboard_is_not_in_changes_nothing() {
    let mut root = window();
    focus_scope(&mut root, "a");
    release_scope(&mut root, "b");
    assert_eq!(page_scope(&root).as_deref(), Some("a"));
}

/// Out of the dock and back in lands on the control it was on, not on the dock.
#[test]
fn taking_the_keyboard_back_returns_to_the_control_it_left() {
    let mut root = window();
    root.base().children[0].base().children[0]
        .base()
        .focus(false);
    crate::settle_focus(&mut root);
    let control = holder(&root);
    release_scope(&mut root, "a");
    assert!(holder(&root).is_none());
    focus_scope(&mut root, "a");
    assert_eq!(holder(&root), control);
}

/// A control outside every named region holds the keyboard on the page, in no dock.
#[test]
fn a_control_outside_every_region_is_in_no_dock() {
    let mut root = Flex::row().child(Control::new()).child(dock("a"));
    root.base().children[0].base().focus(false);
    crate::settle_focus(&mut root);
    assert_eq!(page_scope(&root), None);
    assert!(holder(&root).is_some());
}

/// **A dialog takes the keyboard but not the dock from under it.** While it is open the answer is
/// still the dock it will return to, and after it closes the keyboard is back there.
#[test]
fn a_surface_above_the_page_does_not_move_the_page_scope() {
    let open = signal(false);
    let mut dialog = Control::new();
    dialog.base_mut().follow_focus_modal(open);
    dialog.base_mut().surface_slot = Some("dialog".into());
    let mut root = Flex::row().child(dock("a")).child(dialog);

    focus_scope(&mut root, "a");
    open.set(true);
    crate::settle_focus(&mut root);

    let owner = keyboard_owner(&root).expect("the dialog holds the keyboard");
    assert!(owner.in_surface, "the dialog took it");
    assert_eq!(
        page_scope(&root).as_deref(),
        Some("a"),
        "but the dock is where it goes back to"
    );

    open.set(false);
    crate::settle_focus(&mut root);
    assert!(!keyboard_owner(&root).is_some_and(|o| o.in_surface));
    assert_eq!(page_scope(&root).as_deref(), Some("a"));
}

/// A surface that took the keyboard from nothing has no dock to look through to.
#[test]
fn a_surface_that_took_the_keyboard_from_nothing_has_no_page_scope() {
    let open = signal(false);
    let mut dialog = Control::new();
    dialog.base_mut().follow_focus_modal(open);
    dialog.base_mut().surface_slot = Some("dialog".into());
    let mut root = Flex::row().child(dock("a")).child(dialog);

    open.set(true);
    crate::settle_focus(&mut root);

    assert!(keyboard_owner(&root).is_some_and(|o| o.in_surface));
    assert_eq!(page_scope(&root), None);
}

/// A page with a control **outside** every dock, first in the tree, then two docks.
fn page_with_a_control_outside() -> Flex {
    Flex::row()
        .child(Control::new())
        .child(dock("a"))
        .child(dock("b"))
}

fn press(root: &mut Flex, x: f64) {
    LayoutEngine::new().compute(root, Size::new(300.0, 100.0));
    crate::component::dispatch(
        root,
        &Event::pointer_pressed(Point::new(x, 5.0), PointerButton::Left),
    );
    crate::settle_focus(root);
}

/// **A click outside the dock gives the keys back to the panes.** The tree is the only record, so
/// the press that moves the keyboard off the dock is all it takes: nothing else to update, nothing
/// to forget.
#[test]
fn a_click_outside_every_dock_takes_the_keyboard_out_of_the_dock() {
    let mut root = page_with_a_control_outside();
    focus_scope(&mut root, "a");
    assert_eq!(page_scope(&root).as_deref(), Some("a"));

    press(&mut root, 5.0);

    assert_eq!(
        page_scope(&root),
        None,
        "the keyboard is on the control the click landed on, in no dock"
    );
    assert!(
        holder(&root).is_some(),
        "and it is somewhere: on that control"
    );
}

/// A dock asked for after a click elsewhere takes the keyboard again. A request that only worked on
/// a change of a stored value would find nothing changed and do nothing.
#[test]
fn asking_for_a_dock_again_after_a_click_elsewhere_takes_it_back() {
    let mut root = page_with_a_control_outside();
    focus_scope(&mut root, "a");
    press(&mut root, 5.0);
    assert_eq!(page_scope(&root), None);

    assert!(focus_scope(&mut root, "a"));

    assert_eq!(page_scope(&root).as_deref(), Some("a"));
}

/// A scroll intent reaches the dock the keyboard was just given to, and only that one — which is
/// how `j`/`k`-era paging keeps working with nothing telling the region it is the target.
#[test]
fn a_scroll_intent_after_focusing_a_dock_scrolls_that_docks_region() {
    let region = |name: &str| {
        let mut r = ScrollRegion::new()
            .width(Length::Px(80.0))
            .height(Length::Px(40.0));
        for _ in 0..6 {
            r = r.child(Flex::column().height(30.0));
        }
        Flex::column().child(r).scope_key(name)
    };
    let mut root = Flex::row().child(region("a")).child(region("b"));
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));

    focus_scope(&mut root, "b");
    crate::component::dispatch(
        &mut root,
        &Event::Widget(crate::component::WidgetIntent::ScrollPageDown),
    );

    let offset = |dock: usize| {
        let wrapper = &root.base().children[dock];
        // The wrapper's only child is the region; a scrolled region shows its content higher, so
        // read where the first row's top edge is.
        wrapper.base().children[0].base().children[0]
            .base()
            .bounds
            .loc
            .y
    };
    let moved = offset(1);
    let still = offset(0);
    assert!(
        moved < still,
        "the focused dock's content moved up ({moved}) while the other's stayed ({still})"
    );
}
