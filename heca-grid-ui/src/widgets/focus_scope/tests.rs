use super::*;
use crate::component::WidgetIntent;
use crate::component::{Event, Handled};
use crate::event::PointerButton;
use crate::layout::LayoutEngine;
use crate::scene::{DrawCommand, RectCmd, Scene};
use crate::theme::Theme;
use crate::widgets::Flex;
use heca_core::layout::Size;

/// Paint a wrapper and return the rects it drew, so the outline can be looked for by colour.
fn painted(ring: &mut FocusScope, theme: &Theme) -> Vec<RectCmd> {
    LayoutEngine::new().compute(ring, Size::new(200.0, 100.0));
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, theme);
        ring.paint(&mut cx);
    }
    scene
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCommand::Rect(r) => Some(*r),
            _ => None,
        })
        .collect()
}

fn child() -> Flex {
    Flex::column().width(120.0).height(60.0)
}

/// A scope with the keyboard **inside** it: the child holds it, as a control in a dock would.
fn scope_holding_keyboard() -> FocusScope {
    let inner = child();
    inner.base().focus(true);
    FocusScope::new(inner)
}

#[test]
fn an_unfocused_wrapper_draws_nothing_of_its_own() {
    let theme = Theme::default();
    let mut ring = FocusScope::new(child());
    assert!(
        painted(&mut ring, &theme).is_empty(),
        "a wrapper that has no focus must be invisible",
    );
}

#[test]
fn a_wrapper_with_the_keyboard_inside_outlines_the_child_in_the_themes_focus_colour() {
    let theme = Theme::default();
    let mut ring = scope_holding_keyboard();
    let rects = painted(&mut ring, &theme);
    let want = theme.colors.effective_focus_ring();
    assert!(
        rects.iter().any(|r| r
            .border
            .is_some_and(|b| b.color == want && (b.width - theme.focus_border_width).abs() < 0.01)),
        "the outline is the theme's focus ring at its focus width: {rects:?}",
    );
}

#[test]
fn the_outline_follows_the_keyboard_without_a_rebuild() {
    let theme = Theme::default();
    let mut ring = FocusScope::new(child());
    assert!(painted(&mut ring, &theme).is_empty());
    ring.base().children[0].base().focus(true);
    assert!(
        !painted(&mut ring, &theme).is_empty(),
        "the keyboard arriving inside is all it takes",
    );
    ring.base().children[0].base().blur();
    assert!(painted(&mut ring, &theme).is_empty());
}

#[test]
fn a_theme_that_hides_focus_outlines_hides_this_one_too() {
    let mut theme = Theme::default();
    theme.colors.show_focus_border = false;
    let mut ring = scope_holding_keyboard();
    assert!(
        painted(&mut ring, &theme).is_empty(),
        "the affordance is the theme's call, uniformly",
    );
}

#[test]
fn a_colour_override_wins_over_the_theme() {
    let theme = Theme::default();
    let mine = Color::new(0x40, 0xe0, 0xff, 0xff);
    let mut ring = scope_holding_keyboard().color(mine);
    let rects = painted(&mut ring, &theme);
    assert!(
        rects
            .iter()
            .any(|r| r.border.is_some_and(|b| b.color == mine)),
        "an explicit colour marks a distinct kind of focus: {rects:?}",
    );
}

#[test]
fn the_wrapper_hugs_its_child_so_the_outline_lands_on_it() {
    let mut ring = scope_holding_keyboard();
    LayoutEngine::new().compute(&mut ring, Size::new(200.0, 100.0));
    let outer = ring.base().bounds;
    let inner = ring.base().children[0].base().bounds;
    assert_eq!(
        (outer.size.w, outer.size.h),
        (inner.size.w, inner.size.h),
        "a transparent wrapper measures exactly its child",
    );
}

// ── The keyboard ────────────────────────────────────────────────────────────────────────

/// Records the events that actually reached it, and consumes nothing.
struct Probe {
    base: Base,
    seen: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
}

impl Probe {
    fn new() -> (Self, std::rc::Rc<std::cell::RefCell<Vec<String>>>) {
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut base = Base::new();
        base.style.layout.width = Length::Px(120.0);
        base.style.layout.height = Length::Px(60.0);
        (
            Self {
                base,
                seen: seen.clone(),
            },
            seen,
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
    fn on_event(&mut self, ev: &Event) -> Handled {
        // Being told it gained the keyboard is not the keyboard arriving.
        if !matches!(ev, Event::Focus | Event::Blur) {
            self.seen.borrow_mut().push(format!("{ev:?}"));
        }
        // `No` on purpose: "it arrived" is the claim, and a widget that ignores an event must
        // not stop its siblings from seeing it.
        Handled::No
    }
}

/// A scope that holds the keyboard **itself**, around content that does not.
fn scope(focused: bool) -> (FocusScope, std::rc::Rc<std::cell::RefCell<Vec<String>>>) {
    let (probe, seen) = Probe::new();
    let scope = FocusScope::new(probe);
    if focused {
        scope.base().focus(true);
    }
    (scope, seen)
}

/// **A dock, as a host builds one**: the widget inside that answers the keys holds the keyboard,
/// and the wrapper around it outlines itself because the keyboard is within it.
fn dock(focused: bool) -> (FocusScope, std::rc::Rc<std::cell::RefCell<Vec<String>>>) {
    let (probe, seen) = Probe::new();
    if focused {
        probe.base().focus(true);
    }
    (FocusScope::new(probe), seen)
}

/// A widget intent reaches the widget that holds the keyboard.
#[test]
fn a_widget_intent_reaches_the_focused_content() {
    let (mut dock, seen) = dock(true);
    crate::component::dispatch(&mut dock, &Event::Widget(WidgetIntent::ScrollPageDown));
    assert_eq!(seen.borrow().len(), 1, "the focus owner answered");
}

/// …and not an unfocused one's.
#[test]
fn a_widget_intent_does_not_enter_an_unfocused_scope() {
    let (mut scope, seen) = dock(false);
    crate::component::dispatch(&mut scope, &Event::Widget(WidgetIntent::ScrollPageDown));
    assert!(
        seen.borrow().is_empty(),
        "an unfocused scope is inert to keys"
    );
}

/// **The load-bearing one.** Two docks side by side: only the one holding the keyboard answers,
/// whatever the document order — the intent is routed to it, not offered to the region and
/// taken by whoever is reached first.
#[test]
fn only_the_focused_dock_answers_whatever_the_document_order() {
    let (unfocused, quiet) = dock(false);
    let (focused, heard) = dock(true);
    // Document order: the inert one FIRST, which is the arrangement that breaks under a walk.
    let mut region = Flex::column().child(unfocused).child(focused);

    let handled =
        crate::component::dispatch(&mut region, &Event::Widget(WidgetIntent::ScrollPageDown));

    assert!(
        quiet.borrow().is_empty(),
        "the unfocused dock stayed out of it"
    );
    assert_eq!(heard.borrow().len(), 1, "and the focused one was reached");
    // The probe declines, so nothing claims it — what matters is that the walk got there.
    assert_eq!(handled, Handled::No);
}

/// **A scope that holds the keyboard answers for the capability inside it.** The intent enters
/// the focused region and whatever in there owns it (a scroll area) takes it — which is what lets a
/// host focus a dock by name and send one scroll intent.
#[test]
fn an_intent_enters_a_scope_that_holds_the_keyboard_itself() {
    let (mut open, heard) = scope(true);
    crate::component::dispatch(&mut open, &Event::Widget(WidgetIntent::ScrollPageDown));
    assert_eq!(
        heard.borrow().len(),
        1,
        "the content under the scope answered"
    );
}

/// **A key is not an intent — it stops at the focus owner.**
///
/// An intent names a capability, so it is offered to the focused region and whatever inside it
/// owns that capability answers. A raw key belongs to *one* widget: it reaches the focus owner
/// (this scope) and bubbles from there, and it does **not** descend into the subtree. That is
/// the rule that stops the first row in a list eating an Enter meant for the row the cursor is
/// on — the bug seven widgets used to patch by hand.
#[test]
fn a_key_stops_at_the_focus_owner_and_does_not_enter_its_subtree() {
    let (mut open, heard) = scope(true);
    let (mut shut, quiet) = scope(false);
    let key = Event::Key {
        key: crate::component::GridKey::Enter,
        pressed: true,
    };
    crate::component::dispatch(&mut open, &key);
    crate::component::dispatch(&mut shut, &key);
    assert!(
        heard.borrow().is_empty(),
        "the key is the scope's, not its content's"
    );
    assert!(
        quiet.borrow().is_empty(),
        "and an unfocused scope hears nothing at all"
    );
}

/// **The pointer is never gated.** The mouse carries its own target, so it needs no focus to
/// say where it meant — and a click on an unfocused dock is how you focus it in the first place.
/// (`tests/pointer_delivery.rs` holds the whole set to this.)
#[test]
fn the_pointer_reaches_an_unfocused_scope() {
    let (mut scope, seen) = scope(false);
    LayoutEngine::new().compute(&mut scope, Size::new(200.0, 100.0));
    let pos = heca_core::layout::Point::new(10.0, 10.0);
    for ev in [
        Event::pointer_moved(pos),
        Event::pointer_pressed(pos, PointerButton::Left),
        Event::pointer_released(pos, PointerButton::Left),
        Event::wheel(pos, 0.0, 1.0),
    ] {
        crate::component::dispatch(&mut scope, &ev);
    }
    let seen = seen.borrow();
    let kinds: Vec<&str> = seen
        .iter()
        .map(|s| s.split('(').next().unwrap_or(s))
        .collect();
    for want in [
        "PointerEnter",
        "PointerMove",
        "PointerDown",
        "PointerUp",
        "Click",
        "Scroll",
    ] {
        assert!(
            kinds.contains(&want),
            "{want} entered an unfocused scope: {kinds:?}",
        );
    }
}

#[test]
fn a_boxed_subtree_can_be_wrapped() {
    // What a chrome provider's render seam returns: a `Box<dyn Component>`, which is not itself
    // `Component`, so it cannot go through `new`.
    let inner = child();
    inner.base().focus(true);
    let body: Box<dyn Component> = Box::new(inner);
    let mut ring = FocusScope::new(body);
    let theme = Theme::default();
    assert!(!painted(&mut ring, &theme).is_empty());
}
