use super::*;

#[test]
fn closed_overlay_is_inert() {
    let mut o = Overlay::new().panel(Flex::column());
    assert!(!o.overlay_active());
    assert!(!o.focusable());
    assert!(!o.overlay_occludes(Point::new(1.0, 1.0)));
    assert_eq!(
        crate::component::dispatch(
            &mut o,
            &Event::pointer_pressed(Point::new(1.0, 1.0), PointerButton::Left)
        ),
        Handled::No
    );
}

#[test]
fn blocking_overlay_occludes_everywhere_and_swallows_outside_input() {
    let mut o = open_overlay();
    assert!(
        o.overlay_occludes(Point::new(-500.0, -500.0)),
        "scrim owns every point"
    );
    // A press on the scrim: inside the viewport the layer covers, outside the panel it holds.
    let outside = Point::new(4.0, 4.0);
    assert!(
        !o.panel_bounds().contains(outside),
        "the corner is scrim, not panel"
    );
    assert_eq!(
        crate::component::dispatch(
            &mut o,
            &Event::pointer_pressed(outside, PointerButton::Left)
        ),
        Handled::Yes,
        "modal swallows the outside press"
    );
    assert_eq!(
        crate::component::dispatch(&mut o, &Event::wheel(outside, 0.0, 1.0)),
        Handled::Yes
    );
}

#[test]
fn non_blocking_overlay_occludes_only_its_panel_and_lets_outside_fall_through() {
    use std::cell::Cell;
    use std::rc::Rc;
    let dismissed = Rc::new(Cell::new(false));
    let d = dismissed.clone();
    let mut o = Overlay::new()
        .blocking(false)
        .panel(Flex::column().child(Label::new("hi")))
        .default_open(true)
        .on_outside_click(move || d.set(true));
    // Give the panel real bounds (as layout would).
    o.base.children[0].base_mut().bounds =
        Rectangle::new(Point::new(100.0, 100.0), Size::new(50.0, 20.0));
    assert!(
        o.overlay_occludes(Point::new(110.0, 110.0)),
        "panel point occludes"
    );
    assert!(
        !o.overlay_occludes(Point::new(0.0, 0.0)),
        "outside point does not"
    );
    assert_eq!(
        crate::component::dispatch(
            &mut o,
            &Event::pointer_pressed(Point::new(0.0, 0.0), PointerButton::Left)
        ),
        Handled::No,
        "light layer lets the outside press fall through"
    );
    assert!(dismissed.get(), "outside press fired the dismissal hook");
}

/// **The dismiss key closes a surface that said what dismissal means.**
#[test]
fn the_dismiss_key_runs_the_dismissal_a_surface_declared() {
    use crate::event::WidgetIntent;
    use std::cell::Cell;
    use std::rc::Rc;

    let closed = Rc::new(Cell::new(false));
    let flag = closed.clone();
    let mut o = Overlay::new()
        .panel(Flex::column().child(Label::new("body")))
        .on_dismiss(move || flag.set(true));
    o.show();

    assert_eq!(
        crate::component::dispatch(&mut o, &Event::Widget(WidgetIntent::Dismiss)),
        Handled::Yes,
    );
    assert!(
        closed.get(),
        "Escape ran what the surface said dismissal means"
    );
}

/// **A surface that declared none leaves the key alone.**
///
/// The half that matters more: `Dialog`, `ContextMenu` and `CommandPalette` each answer this key
/// themselves and set no dismissal on their inner overlay. An overlay that swallowed it regardless
/// would have taken Escape from all three at once — three shipped surfaces broken by a fix aimed
/// at a fourth.
#[test]
fn an_overlay_with_no_dismissal_declared_does_not_swallow_the_key() {
    use crate::event::WidgetIntent;

    let mut o = Overlay::new().panel(Flex::column().child(Label::new("body")));
    o.show();
    assert_eq!(
        crate::component::dispatch(&mut o, &Event::Widget(WidgetIntent::Dismiss)),
        Handled::No,
        "the key passes to whatever composes this overlay, untouched",
    );
}

/// **A surface says which control the keyboard starts on, and needs no `key` to do it.**
///
/// The author's call, because only the author knows which control is safe. The name is the
/// control's declared `key` when it has one and **the words it reads by** when it does not —
/// `key` is optional everywhere in this library and a control you point at is no exception.
#[test]
fn a_surface_places_the_keyboard_on_the_control_it_named() {
    use crate::reactive::SignalGet;
    use crate::widgets::Button;

    let mut o = Overlay::new().default_focus("Cancel").panel(
        Flex::row()
            .child(Button::new("Cancel"))
            .child(Button::new("Delete")),
    );
    o.show();

    let panel = &o.base().children[0];
    let cancel = &panel.base().children[0];
    let delete = &panel.base().children[1];
    assert!(
        cancel.base().focused.get_untracked(),
        "named by the words it reads by, with no key declared anywhere",
    );
    assert!(!delete.base().focused.get_untracked(), "and only that one");
}

/// **Taking the caller's signal rebinds the keyboard to it.**
///
/// The keyboard follows whichever signal says whether the surface is up — that binding is
/// the whole of how keys reach a panel. Adopting the caller's without rebinding would leave the
/// keyboard following a signal nobody writes any more, so an overlay that was visibly open would
/// answer nothing.
#[test]
fn following_your_signal_rebinds_the_keyboard_to_it() {
    use crate::reactive::{SignalUpdate, signal};

    let editing = signal(false);
    let o = Overlay::new()
        .panel(Flex::column().child(Label::new("body")))
        .open_when(editing);

    editing.set(true);
    assert!(
        crate::holds_keyboard(&o),
        "an open surface holds the keyboard, whichever signal says it is open",
    );
}

/// **Opening by flag puts the keyboard where opening by call does.** ⚠️ Ran red against its own bug.
///
/// There are two ways a surface goes up — a caller says [`Overlay::show`], or the open flag is
/// simply set (a [`SurfaceHandle`], [`open_when`](Overlay::open_when), a host binding its own
/// state) — and only the first used to place the keyboard, because placing it lived inside the
/// verb. The arrival played either way, so the second looked right and was deaf: it came up with
/// the keyboard on nothing, and the first Tab was spent travelling to the control the surface had
/// already named.
///
/// The two existing guards each cover half of this and cross in the middle: one proves the handle
/// moves the flag, the other proves `show()` places focus. Neither proves the handle places focus,
/// which is exactly what it did not do.
#[test]
fn opening_by_flag_places_the_keyboard_the_same_as_opening_by_call() {
    use crate::component::Component as _;
    use crate::reactive::SignalGet;
    use crate::widgets::Button;

    let mut o = Overlay::new().default_focus("Cancel").panel(
        Flex::row()
            .child(Button::new("Cancel"))
            .child(Button::new("Delete")),
    );
    // The door a click handler uses: copied into a closure, nothing borrowed.
    let handle = o.handle();
    let opener = move || handle.show();
    opener();

    let focused = |o: &Overlay| {
        o.base().children[0].base().children[0]
            .base()
            .focused
            .get_untracked()
    };
    assert!(
        !focused(&o),
        "the flag is set, but nothing has reacted to it yet",
    );

    o.tick(0.016);
    assert!(
        focused(&o),
        "a surface raised by its handle starts on the control it named, exactly as one raised by \
         the call does — otherwise the first Tab is a wasted press",
    );
}

/// **The same, for a signal the caller owns.** A host binding its own state gets the keyboard
/// placed too; the surface does not care who set the flag.
#[test]
fn following_your_own_signal_also_places_the_keyboard() {
    use crate::component::Component as _;
    use crate::reactive::{SignalUpdate, signal};
    use crate::widgets::Button;

    let editing = signal(false);
    let mut o = Overlay::new()
        .default_focus("Cancel")
        .panel(
            Flex::row()
                .child(Button::new("Cancel"))
                .child(Button::new("Delete")),
        )
        .open_when(editing);

    editing.set(true);
    o.tick(0.016);
    assert!(
        o.base().children[0].base().children[0]
            .base()
            .focused
            .get_untracked(),
        "your signal raises it and the keyboard lands where the surface said",
    );
}

/// **Placing the keyboard happens once per arrival, not every frame.**
///
/// Settling runs on every tick, so it has to react to the *change* rather than to the state. If it
/// re-placed on each frame it would drag the keyboard back to the default control continuously,
/// and Tab would appear to do nothing at all while the surface was open.
#[test]
fn a_surface_places_the_keyboard_once_and_then_leaves_it_alone() {
    use crate::component::Component as _;
    use crate::reactive::SignalGet;
    use crate::widgets::Button;

    let mut o = Overlay::new().default_focus("Cancel").panel(
        Flex::row()
            .child(Button::new("Cancel"))
            .child(Button::new("Delete")),
    );
    o.show();
    // Move on, the way a Tab would.
    o.advance_focus(true);
    let moved = |o: &Overlay| {
        o.base().children[0].base().children[1]
            .base()
            .focused
            .get_untracked()
    };
    assert!(moved(&o), "the keyboard moved off the default control");

    o.tick(0.016);
    o.tick(0.016);
    assert!(
        moved(&o),
        "and stayed there — settling reacts to the arrival, not to being open",
    );
}

/// **A closed surface is gone to every walk** — not lettered, not under the pointer — whether it
/// was never opened or was opened and closed. It keeps its box for the next showing, so being laid
/// out is not being shown: a closed map of the panes, built at startup, was lettered over the whole
/// window and hid every other letter behind it.
#[test]
fn a_closed_overlay_is_not_lettered_or_hit() {
    let panel = || Flex::column().child(crate::widgets::Button::new("go").on_click(|| {}));
    let at = Point::new(400.0, 300.0);
    let laid_out = |mut o: Overlay| {
        crate::layout::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));
        o
    };

    let never_opened = laid_out(Overlay::new().panel(panel()));
    assert!(crate::hint::collect_hints(&never_opened).is_empty());
    assert_eq!(crate::pointer::hit_test(&never_opened, at), None);

    let mut closed = laid_out(Overlay::new().panel(panel()).default_open(true));
    assert_eq!(
        crate::hint::collect_hints(&closed).len(),
        1,
        "open, its button is lettered"
    );
    closed.close();
    assert!(crate::hint::collect_hints(&closed).is_empty());
    assert_eq!(crate::pointer::hit_test(&closed, at), None);
}
