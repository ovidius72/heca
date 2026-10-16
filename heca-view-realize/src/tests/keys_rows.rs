//! Described keys, the picker opt-out, and clickable rows.

use heca_view::WidgetKind;

use super::*;

/// **A described key lands in the very slot a native `.key(..)` writes.** That is the whole of
/// the declarative half of the identity rule: one slot, so the keyboard cursor, the right-click
/// target, the drag identity and the picker's remembered letter cannot tell a described row
/// from a native one.
#[test]
fn a_described_key_lands_in_the_widgets_own_slot() {
    let emit: IntentEmitter = Rc::new(|_| {});
    let node = ViewNode::new(WidgetKind::Row)
        .key("pane:7")
        .on_press(Intent::new("focus_pane"));

    let row = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    assert_eq!(row.base().key.as_deref(), Some("pane:7"));
}

/// It is read **generically, for every kind** — not in one arm. A key belongs to no widget in
/// particular, because a collection can be built from any of them.
#[test]
fn every_kind_carries_a_described_key() {
    let emit: IntentEmitter = Rc::new(|_| {});
    for kind in [
        WidgetKind::Row,
        WidgetKind::Item,
        WidgetKind::Button,
        WidgetKind::Card,
        WidgetKind::Label,
        WidgetKind::VStack,
    ] {
        let node = ViewNode::new(kind).key("k");
        let w = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        );
        assert_eq!(
            w.base().key.as_deref(),
            Some("k"),
            "{kind:?} dropped its key"
        );
    }
}

/// A node that declares nothing carries nothing — its identity is derived from its content, and
/// an empty string would be a name that collides with every other empty one.
#[test]
fn a_node_with_no_key_declares_none() {
    let emit: IntentEmitter = Rc::new(|_| {});
    let node = ViewNode::new(WidgetKind::Row).on_press(Intent::new("focus_pane"));
    let row = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    assert_eq!(row.base().key, None);
}

/// `hintable` rides the same generic pass — universal on `Base`, so universal here. Being
/// pickable is not opt-in, so the only thing a description has to say is "not me".
#[test]
fn a_node_can_keep_itself_out_of_the_picker() {
    let emit: IntentEmitter = Rc::new(|_| {});
    let node = ViewNode::new(WidgetKind::Button)
        .text("×")
        .prop("hintable", PropValue::Bool(false))
        .on_press(Intent::new("close"));

    let w = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    assert!(!w.base().hintable);
    assert!(
        realize(
            &ViewNode::new(WidgetKind::Button).on_press(Intent::new("close")),
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        )
        .base()
        .hintable,
        "the default is pickable — a node says only \"not me\"",
    );
}

/// **A described `Row` is the interactive widget, not a box.** Click it and its intent fires;
/// press Enter on it and the same intent fires; it takes one hint target so `prefix+/` reaches
/// it; and it holds whatever content it was given.
///
/// Before the rename, `WidgetKind::Row` meant `Flex::row()` — a plain box with no focus, no
/// hover and no activation — so none of this was reachable from a description at all, and
/// `docs/chrome-and-ui.md` shipped an example that assumed otherwise.
#[test]
fn a_described_row_is_clickable_and_keyboard_activatable() {
    use heca_core::layout::{Point, Size};
    use heca_grid_ui::{Event, GridKey, LayoutEngine};
    use std::cell::RefCell;

    let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = fired.clone();
    let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

    let node = ViewNode::new(WidgetKind::Row)
        .prop("active", PropValue::Bool(true))
        .on_press(Intent::new("docker.select").arg("id", PropValue::Text("web".into())))
        .child(ViewNode::new(WidgetKind::Label).text("nginx"))
        .child(ViewNode::new(WidgetKind::Badge).text("UP"));

    let mut row = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    LayoutEngine::new().compute(row.as_mut(), Size::new(400.0, 40.0));

    assert_eq!(
        row.base().children.len(),
        2,
        "it holds its composed content"
    );
    assert!(row.base().focusable, "an actionable row is focusable");
    assert_eq!(
        hints(row.as_ref()),
        vec![Vec::<usize>::new()],
        "one pick target: the row itself"
    );

    let b = row.base().bounds;
    // A click is a press and the release that completes it — pressing and dragging off the row
    // cancels, the way every other control behaves.
    let at = Point::new(b.loc.x + 5.0, b.loc.y + b.size.h / 2.0);
    heca_grid_ui::dispatch(
        row.as_mut(),
        &Event::pointer_pressed(at, heca_grid_ui::PointerButton::Left),
    );
    heca_grid_ui::dispatch(
        row.as_mut(),
        &Event::pointer_released(at, heca_grid_ui::PointerButton::Left),
    );
    // A raw key reaches only the widget that owns the keyboard — a real surface focuses the row
    // before sending one, and an unfocused row taking Enter is what let a card eat the key
    // meant for the list around it.
    heca_grid_ui::reactive::SignalUpdate::set(&row.base_mut().focused, true);
    heca_grid_ui::dispatch(
        row.as_mut(),
        &Event::Key {
            key: GridKey::Enter,
            pressed: true,
        },
    );

    let fired = fired.borrow();
    assert_eq!(
        fired.len(),
        2,
        "a click and an Enter each fire it: {fired:?}"
    );
    for intent in fired.iter() {
        assert_eq!(intent.action, "docker.select");
        assert_eq!(intent.args.get("id"), Some(&PropValue::Text("web".into())));
    }
}

/// A `Row` with no press intent stays inert — not focusable, nothing for a letter to land on.
/// A described row that nothing can activate should not pretend to be a control.
#[test]
fn a_described_row_without_a_press_intent_is_inert() {
    let node = ViewNode::new(WidgetKind::Row)
        .child(ViewNode::new(WidgetKind::Label).text("just content"));
    let row = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert!(!row.base().focusable);
    assert!(hints(row.as_ref()).is_empty());
    assert_eq!(row.base().children.len(), 1, "it still holds its content");
}

/// A still-deferred structured kind (needs track/slot model support — `Grid`, `choice-6`)
/// realizes to an empty container instead of panicking — the tree stays total for untrusted
/// plugin/RPC input.
#[test]
fn deferred_kind_is_empty_not_panic() {
    let node = ViewNode::new(WidgetKind::Grid);
    let realized = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(realized.base().children.len(), 0);
    assert!(
        hints(realized.as_ref()).is_empty(),
        "an empty fallback declares no hint"
    );
}
