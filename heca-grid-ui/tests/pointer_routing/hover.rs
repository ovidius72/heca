use super::*;

// ── hover ─────────────────────────────────────────────────────────────────────────────────────

/// Hover follows the pointer: one enter, one leave, and never both widgets lit at once.
#[test]
fn hover_moves_from_one_widget_to_the_other() {
    let (mut root, a, b) = two_probes();
    mv(root.as_mut(), LEFT);
    assert_eq!(meaningful(&a), vec!["PointerEnter"]);
    assert!(
        meaningful(&b).is_empty(),
        "nothing happened to the other one"
    );

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
    press_at(root.as_mut(), RIGHT, PointerButton::Left);
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
