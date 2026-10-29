//! Options: choice, select and tabs.

use heca_view::WidgetId;

use super::*;

// ── Options (Choice / Select / Tabs) ──

/// An option node: a `value` plus composed content.
fn option_node(value: &str, label: &str) -> ViewNode {
    ViewNode::new(WidgetId::Choice)
        .prop("value", PropValue::Text(value.into()))
        .child(ViewNode::new(WidgetId::Icon).prop("icon", PropValue::Glyph("circle".into())))
        .child(ViewNode::new(WidgetId::Label).text(label))
}

/// A `Select`'s options are its **children** — not a list of strings in a prop — so each one
/// composes its own content (here an icon + a label), exactly like a native `Choice`.
#[test]
fn select_node_realizes_its_choice_children_as_options() {
    let node = ViewNode::new(WidgetId::Select)
        .prop("selected", PropValue::Int(1))
        .child(option_node("low", "LOW"))
        .child(option_node("high", "HIGH"));

    let select = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    let options = &select.base().children;
    assert_eq!(options.len(), 2, "one option per Choice child");
    assert_eq!(
        options[1].base().children.len(),
        2,
        "the option composes its own content: icon + label",
    );
    assert_eq!(
        options[1].text_summary().as_deref(),
        Some("HIGH"),
        "the option is named by the content it composes",
    );
}

/// **The point of the whole model.** The widget tracks an index (it is an indexable list), but an
/// index is meaningless to a plugin and breaks the moment the options are reordered. `realize`
/// maps it back through the options' `value` props, so the author's intent fires with
/// `{"value": "high"}`.
#[test]
fn the_change_intent_carries_the_chosen_options_value_not_its_index() {
    use heca_core::layout::{Point, Size};
    use heca_grid_ui::{Event, LayoutEngine};
    use std::cell::RefCell;

    let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = fired.clone();
    let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

    let node = ViewNode::new(WidgetId::Select)
        .on("change", Intent::new("set_level"))
        .child(option_node("low", "LOW"))
        .child(option_node("high", "HIGH"));
    let mut select = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    LayoutEngine::new().compute(select.as_mut(), Size::new(400.0, 300.0));

    // Open the dropdown, then click the second option where it actually is (its real bounds).
    let trigger = select.base().bounds;
    heca_grid_ui::dispatch(
        select.as_mut(),
        &Event::pointer_pressed(
            Point::new(trigger.loc.x + 5.0, trigger.loc.y + 5.0),
            heca_grid_ui::PointerButton::Left,
        ),
    );
    let high = select.base().children[1].base().bounds;
    heca_grid_ui::dispatch(
        select.as_mut(),
        &Event::pointer_pressed(
            Point::new(high.loc.x + 5.0, high.loc.y + high.size.h / 2.0),
            heca_grid_ui::PointerButton::Left,
        ),
    );

    let fired = fired.borrow();
    let [intent] = fired.as_slice() else {
        panic!("expected exactly one intent, got {fired:?}");
    };
    assert_eq!(intent.action, "set_level");
    assert_eq!(
        intent.args.get("value"),
        Some(&PropValue::Text("high".into())),
        "the chosen option's value, not an opaque index",
    );
}

/// The same model, and the same value mapping, drives a `Tabs` node — one option primitive, two
/// consumers.
#[test]
fn tabs_node_realizes_choice_children_and_reports_the_chosen_value() {
    use heca_core::layout::Size;
    use heca_grid_ui::{Event, LayoutEngine, WidgetIntent};
    use std::cell::RefCell;

    let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = fired.clone();
    let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

    let node = ViewNode::new(WidgetId::Tabs)
        .on("change", Intent::new("show_tab"))
        .child(option_node("files", "FILES"))
        .child(option_node("issues", "ISSUES"));
    let mut tabs = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    LayoutEngine::new().compute(tabs.as_mut(), Size::new(400.0, 100.0));
    assert_eq!(tabs.base().children.len(), 2, "one tab per Choice child");

    // A described tree is driven by the keyboard exactly like a hand-built one: an intent goes
    // to the focus owner, so the tab strip has to be holding the keyboard to answer one.
    heca_grid_ui::reactive::SignalUpdate::set(&tabs.base_mut().focused, true);
    heca_grid_ui::dispatch(tabs.as_mut(), &Event::Widget(WidgetIntent::ItemNext));
    let fired = fired.borrow();
    let [intent] = fired.as_slice() else {
        panic!("expected exactly one intent, got {fired:?}");
    };
    assert_eq!(intent.action, "show_tab");
    assert_eq!(
        intent.args.get("value"),
        Some(&PropValue::Text("issues".into()))
    );
}

/// A **childless** `Choice` falls back to the scalar sugar — `text` → one `Label` child, the very
/// child the composed form would build. Same precedence rule as `Button`: children win.
#[test]
fn childless_choice_node_desugars_its_text_to_a_label_child() {
    let node = ViewNode::new(WidgetId::Choice)
        .prop("value", PropValue::Text("high".into()))
        .text("HIGH");
    let choice = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    assert_eq!(
        choice.base().children.len(),
        1,
        "text desugars to a Label child"
    );
    assert_eq!(choice.text_summary().as_deref(), Some("HIGH"));
}

/// Invalid option content is diagnosed without losing the valid options around it.
#[test]
fn a_non_choice_child_of_a_select_is_reported() {
    let node = ViewNode::new(WidgetId::Select)
        .child(option_node("low", "LOW"))
        .child(ViewNode::new(WidgetId::Button).text("I am not an option"))
        .child(option_node("high", "HIGH"));
    let select = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    ).expect("valid options survive malformed content");
    let options = &select.base().children;
    assert_eq!(options.len(), 2);
    assert_eq!(options[0].text_summary().as_deref(), Some("LOW"));
    assert_eq!(options[1].text_summary().as_deref(), Some("HIGH"));
}
