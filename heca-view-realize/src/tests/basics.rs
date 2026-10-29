//! Structure, children, and the hint a press declares.

use heca_view::{ViewVariant, WidgetId};

use super::*;

/// The realized tree mirrors the model's structure: the column has 2 children (label +
/// row) and the row has 2 children (the buttons). Verifies recursion + child attachment
/// through `base_mut().children` (the `heca_grid_ui::WidgetModel` push path).
#[test]
fn realizes_nested_structure() {
    let root = realize(
        &confirm_tree(),
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    assert_eq!(root.base().children.len(), 2, "column: label + row");
    let row = &root.base().children[1];
    assert_eq!(row.base().children.len(), 2, "row: two buttons");
}

/// Every actionable node (a `"press"` binding) declares exactly one hint carrying the node's
/// own intent — so the picker fires the identical action a click would. Non-actionable nodes
/// declare nothing.
#[test]
fn actionable_nodes_declare_the_hint_their_click_would_fire() {
    let (emit, fired) = recording_emitter();
    let mut root = realize(
        &confirm_tree(),
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    let found = hints(root.as_ref());
    assert_eq!(
        found.len(),
        2,
        "only the two buttons are actionable (label + containers are not)",
    );
    // Document order: the row is the column's second child, Cancel its first.
    assert_eq!(found, vec![vec![1, 0], vec![1, 1]]);
    for path in &found {
        assert!(heca_grid_ui::fire_hint(root.as_mut(), path));
    }
    let actions: Vec<String> = fired.borrow().iter().map(|i| i.action.clone()).collect();
    assert_eq!(actions, vec!["confirm_cancel", "confirm_ok"]);
}

/// **A pick is not a click.** A node that declares its own `hint` fires *that*, not its
/// `press` — which is the whole reason the two are separate events: heca's sidebar row
/// activates the pane and leaves on a click, and stays in the sidebar on a hint pick.
#[test]
fn a_declared_hint_wins_over_the_press() {
    let (emit, fired) = recording_emitter();
    let node = ViewNode::new(WidgetId::Row)
        .on_press(Intent::new("activate_and_leave"))
        .on_hint(Intent::new("hint_and_stay"));
    let mut row = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    assert!(heca_grid_ui::fire_hint(row.as_mut(), &[]));
    assert_eq!(
        fired
            .borrow()
            .iter()
            .map(|i| i.action.clone())
            .collect::<Vec<_>>(),
        vec!["hint_and_stay"],
    );
}

/// A `Button` node's **children are its content**: an arbitrary subtree is realized and mounted
/// inside the button. Before the button composed its content, `realize` had nowhere to put them
/// and dropped them silently — a declarative `Button(Icon + Label)` rendered as a bare button.
#[test]
fn button_children_are_realized_as_its_content() {
    let node = ViewNode::new(WidgetId::Button)
        .prop("variant", PropValue::Variant(ViewVariant::Destructive))
        .on_press(Intent::new("confirm_ok"))
        .child(
            ViewNode::new(WidgetId::VStack)
                .prop("gap", PropValue::Int(4))
                .child(
                    ViewNode::new(WidgetId::HStack)
                        .child(
                            ViewNode::new(WidgetId::Icon)
                                .prop("icon", PropValue::Glyph("trash".into())),
                        )
                        .child(ViewNode::new(WidgetId::Label).text("Delete")),
                )
                .child(ViewNode::new(WidgetId::Label).text("Ctrl+D")),
        );

    let button = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    let column = &button.base().children;
    assert_eq!(column.len(), 1, "the button holds its composed subtree");
    let column = &column[0].base().children;
    assert_eq!(
        column.len(),
        2,
        "column: the icon+label row, then the accelerator label"
    );
    assert_eq!(column[0].base().children.len(), 2, "row: icon + label");

    // The button is still one pick target, whatever it composes.
    assert_eq!(
        hints(button.as_ref()),
        vec![Vec::<usize>::new()],
        "the button, not its content"
    );
}

/// A **childless** Button node falls back to the scalar sugar — `text` (+ an optional leading
/// `icon`) — which builds the very same children the explicit form would. One content model,
/// two spellings.
#[test]
fn childless_button_node_uses_the_scalar_sugar() {
    let node = ViewNode::new(WidgetId::Button)
        .text("Delete")
        .prop("icon", PropValue::Glyph("trash".into()));
    let button = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    assert_eq!(
        button.base().children.len(),
        2,
        "text + icon props desugar into [Icon, Label] children",
    );
}
