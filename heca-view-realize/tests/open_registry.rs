//! Host additions use the same factory, recursion and diagnostics as library widgets.

use std::rc::Rc;
use heca_grid_ui::{Label, SetProp, Theme};
use heca_view::{ViewNode, WidgetId};
use heca_view_realize::{FormBindings, IntentEmitter, RealizeErrorKind, WidgetDefinition, WidgetRegistry};

fn emitter() -> IntentEmitter { Rc::new(|_| {}) }

#[test]
fn a_custom_widget_is_realized_inside_a_builtin_without_an_enum_or_dispatch_edit() {
    let mut registry = WidgetRegistry::builtins().expect("library definitions");
    registry.register(
        WidgetDefinition::new("test.custom_label", |_node, _cx| Ok(Label::new("Host extension")))
            .names("CustomLabel", "Label")
            .properties::<Label>(),
    ).expect("new definition");
    let tree = ViewNode::new(WidgetId::VStack).child(ViewNode::new("test.custom_label"));
    let widget = registry.realize(&tree, &Theme::default(), &emitter(), &mut FormBindings::default())
        .expect("registered nested widget");
    assert_eq!(widget.base().children[0].text_summary().as_deref(), Some("Host extension"));
    assert_eq!(registry.get(&WidgetId::new("test.custom_label")).expect("metadata").metadata().properties,
        Label::PROP_NAMES);
}

#[test]
fn an_unknown_nested_widget_returns_its_path_instead_of_an_empty_component() {
    let tree = ViewNode::new(WidgetId::VStack).child(
        ViewNode::new(WidgetId::HStack).child(ViewNode::new("test.missing")),
    );
    let registry = WidgetRegistry::builtins().expect("library definitions");
    let Err(error) = registry.realize(&tree, &Theme::default(), &emitter(), &mut FormBindings::default())
        else { panic!("unknown widget must be refused") };
    assert_eq!(error.kind, RealizeErrorKind::UnknownWidget(WidgetId::new("test.missing")));
    assert_eq!(error.path, vec![0, 0]);
}

#[test]
fn duplicate_registration_is_reported_without_replacing_the_original() {
    let mut registry = WidgetRegistry::new();
    registry.register(WidgetDefinition::new("test.unique", |_node, _cx| Ok(Label::new("First"))))
        .expect("first registration");
    let result = registry.register(WidgetDefinition::new("test.unique", |_node, _cx| Ok(Label::new("Second"))));
    assert_eq!(result.expect_err("duplicate" ).kind,
        RealizeErrorKind::DuplicateWidget(WidgetId::new("test.unique")));
    let widget = registry.realize(&ViewNode::new("test.unique"), &Theme::default(), &emitter(), &mut FormBindings::default())
        .expect("first definition still available");
    assert_eq!(widget.text_summary().as_deref(), Some("First"));
}

#[test]
fn all_legacy_wire_names_still_resolve_to_registered_implementations() {
    let legacy: Vec<String> = serde_json::from_str(include_str!("../../heca-view/tests/fixtures/legacy-widget-names.json"))
        .expect("frozen pre-migration wire names");
    let registry = WidgetRegistry::builtins().expect("library definitions");
    for name in legacy {
        let json = format!("{{\"kind\":\"{name}\"}}");
        let node: ViewNode = serde_json::from_str(&json).expect("old description");
        assert!(registry.get(&node.kind).is_some(), "legacy widget '{name}' was lost");
        assert_eq!(serde_json::to_value(&node).expect("serialized description")["kind"], name);
    }
}
