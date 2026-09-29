use super::*;

// ── the vocabulary ────────────────────────────────────────────────────────────────────────────

/// A press and a release on the same widget are a click, and the widget hears the whole story:
/// down, up, click — in that order, all of them targeted at it.
#[test]
fn a_press_and_a_release_on_one_widget_are_a_click() {
    let (mut root, a, b) = two_probes();
    click_at(root.as_mut(), LEFT, PointerButton::Left);
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
    press_at(root.as_mut(), LEFT, PointerButton::Left);
    release_at(root.as_mut(), RIGHT, PointerButton::Left);
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
    click_at(root.as_mut(), LEFT, PointerButton::Left);
    click_at(root.as_mut(), LEFT, PointerButton::Left);
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
    click_at(root.as_mut(), LEFT, PointerButton::Left);
    click_at(root.as_mut(), RIGHT, PointerButton::Left);
    assert!(!kinds(&a).contains(&"DoubleClick".to_string()));
    assert!(
        !kinds(&b).contains(&"DoubleClick".to_string()),
        "the second widget's first click is a first click: {:?}",
        kinds(&b),
    );
}
