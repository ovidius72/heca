use super::*;

/// **A handler asking what is held gets what is held**, whatever the event — a key press with Shift
/// down says Shift, not "nothing", because the library keeps that answer for every event.
#[test]
fn a_handler_reads_the_held_modifiers_on_an_event_with_no_pointer() {
    remember_modifiers(Modifiers {
        shift: true,
        ..Modifiers::default()
    });
    let focus = Event::Focus;
    assert!(EventCx::new(&focus).modifiers().shift);
}

/// Every event that carries a pointer, built at `at`.
fn pointer_events(at: heca_core::layout::Point) -> Vec<Event> {
    let p = PointerEvent::at(at);
    vec![
        Event::PointerDown(p),
        Event::PointerUp(p),
        Event::PointerMove(p),
        Event::PointerEnter(p),
        Event::PointerLeave(p),
        Event::Click(p),
        Event::DoubleClick(p),
        Event::TripleClick(p),
        Event::RightClick(p),
        Event::MiddleClick(p),
        Event::PointerDownOutside(p),
        Event::Scroll(p),
    ]
}

/// **Every pointer event answers where it happened and which widget it reached from its own
/// payload** — the one list — and the press broadcast to everything it missed is never stamped as
/// if it had landed there.
#[test]
fn every_pointer_event_answers_where_and_on_what_from_its_payload() {
    use heca_core::layout::{Point, Rectangle, Size};
    let at = Point::new(7.0, 9.0);
    let bounds = Rectangle::new(Point::new(1.0, 2.0), Size::new(30.0, 40.0));
    for ev in pointer_events(at) {
        assert_eq!(ev.position(), Some(at), "{ev:?}");
        let outside = matches!(ev, Event::PointerDownOutside(_));
        let stamped = ev.clone().with_target_bounds(bounds);
        assert_eq!(
            stamped.target_bounds(),
            (!outside).then_some(bounds),
            "{ev:?}"
        );
        assert_eq!(stamped.pointer().map(|p| p.pos), Some(at), "{ev:?}");
    }
    assert_eq!(Event::Focus.position(), None);
    assert_eq!(
        Event::Focus.with_target_bounds(bounds).target_bounds(),
        None
    );
}

/// **One wheel conversion for every host**: a wheel's notches are its lines; a trackpad keeps its
/// pixels and is counted in lines of the host's text; Shift turns a plain wheel sideways but leaves
/// a trackpad that already moves sideways alone.
#[test]
fn a_wheel_turn_fills_lines_and_pixels_in_one_convention() {
    use super::{RawPointer, RawPointerKind, WheelTurn};
    let raw = |turn, shift| {
        RawPointer::new(
            RawPointerKind::Wheel,
            heca_core::layout::Point::new(0.0, 0.0),
        )
        .with_wheel(turn, 20.0, shift)
    };
    let wheel = raw(WheelTurn::Lines(0.0, 3.0), false);
    assert_eq!(
        (wheel.delta_x, wheel.delta_y, wheel.delta_pixels),
        (0.0, 3.0, None)
    );
    let pad = raw(WheelTurn::Pixels(0.0, 30.0), false);
    assert_eq!(
        (pad.delta_x, pad.delta_y, pad.delta_pixels),
        (0.0, 1.5, Some((0.0, 30.0))),
        "30 px of 20-px lines"
    );
    let sideways = raw(WheelTurn::Lines(0.0, 2.0), true);
    assert_eq!(
        (sideways.delta_x, sideways.delta_y),
        (2.0, 0.0),
        "Shift: a plain wheel scrolls sideways"
    );
    let pad_sideways = raw(WheelTurn::Pixels(10.0, 4.0), true);
    assert_eq!(
        (pad_sideways.delta_x, pad_sideways.delta_y),
        (0.5, 0.2),
        "a trackpad already moving sideways is left alone"
    );
}
