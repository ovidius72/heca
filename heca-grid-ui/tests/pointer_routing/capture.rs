use super::*;

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

    press_at(root.as_mut(), LEFT, PointerButton::Left);
    mv(root.as_mut(), Point::new(500.0, 500.0));
    release_at(root.as_mut(), Point::new(500.0, 500.0), PointerButton::Left);

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
    press_at(root.as_mut(), LEFT, PointerButton::Left);
    root.base_mut().children.remove(0);
    // Nothing to assert but the absence of a panic and of a click for a widget that is not there.
    release_at(root.as_mut(), LEFT, PointerButton::Left);
}
