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

/// **A position in [`EventKind::ALL`], by an exhaustive match** — adding a kind without listing it
/// here does not compile, which is what makes the two lists below checkable at all.
fn ordinal(kind: EventKind) -> usize {
    use EventKind::*;
    match kind {
        Key => 0,
        TextInput => 1,
        ModifiersChanged => 2,
        Widget => 3,
        Hint => 4,
        PointerDown => 5,
        PointerUp => 6,
        PointerMove => 7,
        PointerRecheck => 8,
        PointerEnter => 9,
        PointerLeave => 10,
        Click => 11,
        DoubleClick => 12,
        TripleClick => 13,
        RightClick => 14,
        MiddleClick => 15,
        PointerDownOutside => 16,
        Scroll => 17,
        DragStart => 18,
        Drag => 19,
        DragEnd => 20,
        DragEnter => 21,
        DragOver => 22,
        DragLeave => 23,
        Drop => 24,
        DragInFlight => 25,
        DragSettled => 26,
        Focus => 27,
        Blur => 28,
        Mount => 29,
        Unmount => 30,
    }
}

/// **Every kind has a name, and every name a kind.** `EventKind::ALL` lists each kind once (held by
/// an exhaustive match), the names table covers every one of them, a name is spelled once, and
/// only a key has halves.
#[test]
fn every_event_kind_has_a_name_and_every_name_a_kind() {
    let mut seen: Vec<usize> = EventKind::ALL.iter().map(|k| ordinal(*k)).collect();
    seen.sort_unstable();
    assert_eq!(
        seen,
        (0..seen.len()).collect::<Vec<_>>(),
        "EventKind::ALL lists every kind exactly once"
    );
    assert_eq!(seen.len(), 31, "the match above and the list agree on how many there are");

    for kind in EventKind::ALL {
        assert!(
            EVENT_NAMES.iter().any(|e| e.kind == *kind),
            "{kind:?} has no name a description can bind it by"
        );
    }
    for entry in EVENT_NAMES {
        assert!(EventKind::ALL.contains(&entry.kind), "{} names no kind", entry.name);
        assert_eq!(
            entry.key_pressed.is_some(),
            entry.kind == EventKind::Key,
            "{}: only a key has a down and an up",
            entry.name
        );
        assert_eq!(EventName::of(entry.name).map(|e| e.kind), Some(entry.kind));
    }
    let mut names: Vec<&str> = EVENT_NAMES.iter().map(|e| e.name).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(names.len(), before, "a name is spelled once");
    assert!(EventName::of("not_an_event").is_none());
}
