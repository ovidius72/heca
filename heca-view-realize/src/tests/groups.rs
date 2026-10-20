//! Groups: item groups and marker groups.

use heca_view::WidgetKind;

use super::*;

// ── Groups (ItemGroup / MarkerGroup) ──

/// An `ItemGroup` node realizes its header from `text` and its rows from its children — the
/// widget's own header is `children[0]`, so the rows follow it.
#[test]
fn item_group_node_realizes_its_header_and_rows() {
    use heca_core::layout::Size;
    use heca_grid_ui::LayoutEngine;

    let node = ViewNode::new(WidgetKind::ItemGroup)
        .text("EXPLORER")
        .prop("expanded", PropValue::Bool(false))
        .child(ViewNode::new(WidgetKind::Item).text("src"))
        .child(ViewNode::new(WidgetKind::Item).text("tests"));

    let mut group = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    // The group applies its expanded state during layout (`remeasure`), like the native widget.
    LayoutEngine::new().compute(group.as_mut(), Size::new(300.0, 200.0));
    assert_eq!(
        group.base().children.len(),
        3,
        "the group's own header, then the two realized rows",
    );
    // Collapsed: the rows leave layout (`display: none`), the header stays.
    assert!(
        !group.base().children[0].base().style.layout.hidden,
        "the header stays"
    );
    assert!(
        group.base().children[1..]
            .iter()
            .all(|row| row.base().style.layout.hidden),
        "`expanded: false` folds the rows away",
    );
}

/// **`toggle`** — the third canonical event name, alongside `press` and `change`. A toggle is
/// only meaningful with its new state, so the intent carries `args["expanded"]`: an author binds
/// one action and learns which way it went.
#[test]
fn the_toggle_intent_carries_the_new_expanded_state() {
    use heca_core::layout::{Point, Size};
    use heca_grid_ui::{Event, LayoutEngine};
    use std::cell::RefCell;

    let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = fired.clone();
    let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

    let node = ViewNode::new(WidgetKind::ItemGroup)
        .text("EXPLORER")
        .on("toggle", Intent::new("fold_group"))
        .child(ViewNode::new(WidgetKind::Item).text("src"));
    let mut group = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    );
    LayoutEngine::new().compute(group.as_mut(), Size::new(300.0, 200.0));

    // Click the header (it starts expanded) → it collapses.
    let header = group.base().children[0].base().bounds;
    let at = Point::new(header.loc.x + 5.0, header.loc.y + header.size.h / 2.0);
    heca_grid_ui::dispatch(
        group.as_mut(),
        &Event::pointer_pressed(at, heca_grid_ui::PointerButton::Left),
    );
    heca_grid_ui::dispatch(
        group.as_mut(),
        &Event::pointer_released(at, heca_grid_ui::PointerButton::Left),
    );

    let fired = fired.borrow();
    let [intent] = fired.as_slice() else {
        panic!("expected exactly one intent, got {fired:?}");
    };
    assert_eq!(intent.action, "fold_group");
    assert_eq!(
        intent.args.get("expanded"),
        Some(&PropValue::Bool(false)),
        "the toggle reports the state it moved to",
    );
}

/// A `MarkerGroup` is an indicator: bools + children, no events of its own (the rows inside carry
/// their own intents).
#[test]
fn marker_group_node_realizes_its_rows_and_flags() {
    let node = ViewNode::new(WidgetKind::MarkerGroup)
        .prop("active", PropValue::Bool(true))
        .prop("nav_selected", PropValue::Bool(true))
        .child(ViewNode::new(WidgetKind::Item).text("pane 1"))
        .child(ViewNode::new(WidgetKind::Item).text("pane 2"));

    let markers = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(markers.base().children.len(), 2, "the two realized rows");
    assert!(
        hints(markers.as_ref()).is_empty(),
        "an indicator is not a pick target"
    );
}
