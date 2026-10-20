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
    assert_eq!(Event::Focus.with_target_bounds(bounds).target_bounds(), None);
}
