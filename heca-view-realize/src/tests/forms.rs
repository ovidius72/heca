//! Containers, change bindings and form fields.

use heca_view::WidgetKind;

use super::*;

/// A newly-mapped container attaches its realized children. `Surface` injects no title, so
/// its child count is exactly the content (unlike `Card`/`DockFrame`, whose `new(title)` adds
/// a title child first).
#[test]
fn container_kind_attaches_children() {
    let node = ViewNode::new(WidgetKind::Surface)
        .child(ViewNode::new(WidgetKind::Label).text("a"))
        .child(ViewNode::new(WidgetKind::Label).text("b"));
    let realized = realize(
        &node,
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(
        realized.base().children.len(),
        2,
        "surface holds its two content children"
    );

    // `Card` prepends a title child, so title + 2 content = 3.
    let card = realize(
        &ViewNode::new(WidgetKind::Card)
            .text("Title")
            .child(ViewNode::new(WidgetKind::Label).text("a")),
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(card.base().children.len(), 2, "card = title + 1 content");
}

/// A `"change"` binding (value widgets) fires the intent but is NOT what a pick does (a value
/// change isn't a gesture a letter can stand for); a `"press"` binding (Item) is.
#[test]
fn change_binding_declares_no_hint_but_press_does() {
    let input = realize(
        &ViewNode::new(WidgetKind::Input).on("change", Intent::new("q_changed")),
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    // Lettered because it holds the keyboard — a pick gives it the keyboard — but the change
    // binding is not what a pick does.
    assert_eq!(
        hints(input.as_ref()).len(),
        1,
        "a field holds the keyboard, so it is lettered"
    );
    assert_eq!(
        heca_grid_ui::hint_intent(input.as_ref(), &[]),
        None,
        "a change binding is not what a pick does"
    );

    let item = realize(
        &ViewNode::new(WidgetKind::Item)
            .text("Row")
            .on_press(Intent::new("row_activated")),
        &Theme::default(),
        &noop_emitter(),
        &mut FormBindings::default(),
    );
    assert_eq!(
        hints(item.as_ref()),
        vec![Vec::<usize>::new()],
        "an actionable Item is"
    );
}

/// A value widget with a `"name"` prop is bound into the form; `collect()` reads its current
/// value under that name. An unnamed value widget is not collected.
#[test]
fn named_value_widgets_are_collected() {
    let mut forms = FormBindings::default();
    let node = ViewNode::new(WidgetKind::VStack)
        .child(
            ViewNode::new(WidgetKind::Input)
                .text("hello")
                .prop("name", PropValue::Text("q".into())),
        )
        .child(
            ViewNode::new(WidgetKind::Checkbox)
                .prop("checked", PropValue::Bool(true))
                .prop("name", PropValue::Text("agree".into())),
        )
        // Unnamed → not collected.
        .child(ViewNode::new(WidgetKind::Input).text("ignored"));
    let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut forms);

    let data = forms.collect();
    assert_eq!(data.len(), 2, "only the two named widgets are collected");
    assert_eq!(data.get("q").and_then(PropValue::as_text), Some("hello"));
    assert_eq!(data.get("agree").and_then(PropValue::as_bool), Some(true));
}

/// A named `Select` is a form field: `collect()` returns the **value** of the chosen option
/// (not its index), mapped through the options' `value` props at the live selection.
#[test]
fn named_select_is_collected_as_its_chosen_value() {
    let option = |value: &str, label: &str| {
        ViewNode::new(WidgetKind::Choice)
            .prop("value", PropValue::Text(value.into()))
            .text(label)
    };
    let mut forms = FormBindings::default();
    let node = ViewNode::new(WidgetKind::Select)
        .prop("name", PropValue::Text("priority".into()))
        .prop("selected", PropValue::Int(2))
        .child(option("low", "Low"))
        .child(option("medium", "Medium"))
        .child(option("high", "High"));
    let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut forms);

    assert_eq!(
        forms.collect().get("priority").and_then(PropValue::as_text),
        Some("high"),
        "the chosen option's value, not its index",
    );
}

/// A rich modal body with one of every value-widget kind marshals each named field into
/// `collect()` under its own name — the end-to-end shape `ModalResult::Action.data` returns.
#[test]
fn rich_modal_body_marshals_every_named_field() {
    let mut forms = FormBindings::default();
    let node = ViewNode::new(WidgetKind::VStack)
        .child(
            ViewNode::new(WidgetKind::Input)
                .text("nginx")
                .prop("name", PropValue::Text("host".into())),
        )
        .child(
            ViewNode::new(WidgetKind::Toggle)
                .prop("on", PropValue::Bool(true))
                .prop("name", PropValue::Text("tls".into())),
        )
        .child(
            ViewNode::new(WidgetKind::Checkbox)
                .prop("checked", PropValue::Bool(false))
                .prop("name", PropValue::Text("force".into())),
        )
        .child(
            ViewNode::new(WidgetKind::Select)
                .prop("name", PropValue::Text("region".into()))
                .prop("selected", PropValue::Int(1))
                .child(
                    ViewNode::new(WidgetKind::Choice)
                        .prop("value", PropValue::Text("eu".into()))
                        .text("Europe"),
                )
                .child(
                    ViewNode::new(WidgetKind::Choice)
                        .prop("value", PropValue::Text("us".into()))
                        .text("US"),
                ),
        );
    let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut forms);

    let data = forms.collect();
    assert_eq!(data.len(), 4, "every named field is collected");
    assert_eq!(data.get("host").and_then(PropValue::as_text), Some("nginx"));
    assert_eq!(data.get("tls").and_then(PropValue::as_bool), Some(true));
    assert_eq!(data.get("force").and_then(PropValue::as_bool), Some(false));
    assert_eq!(data.get("region").and_then(PropValue::as_text), Some("us"));
}

/// A named text `Input` exposes its **live value signal** for reactive validation (used to
/// disable a submit button while empty). Only text inputs register; other kinds / unnamed
/// inputs do not.
#[test]
fn named_input_exposes_live_text_signal() {
    use heca_grid_ui::reactive::SignalUpdate;
    let mut forms = FormBindings::default();
    let node = ViewNode::new(WidgetKind::Input)
        .text("term")
        .prop("name", PropValue::Text("name".into()));
    let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut forms);

    let sig = forms
        .text_signal("name")
        .expect("named input exposes its signal");
    assert_eq!(
        sig.get_untracked(),
        "term",
        "signal reflects the initial value"
    );
    // The signal is live: updating it is what `collect()` / validation later read.
    sig.set("renamed".to_string());
    assert_eq!(
        forms.collect().get("name").and_then(PropValue::as_text),
        Some("renamed"),
    );
    assert!(forms.text_signal("missing").is_none());
}
