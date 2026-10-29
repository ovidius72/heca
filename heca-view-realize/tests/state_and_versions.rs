//! Data inputs reach native state through the existing props seam.

use std::{cell::{Cell, RefCell}, rc::Rc};
use heca_grid_ui::{Label, Theme};
use heca_view::{PropValue, ViewNode, WidgetId};
use heca_view_realize::{FormBindings, IntentEmitter, RealizeErrorKind, WidgetDefinition, WidgetRegistry};

fn emitter() -> IntentEmitter { Rc::new(|_| {}) }

#[test]
fn a_described_scrollbar_updates_its_existing_native_state_and_form_value() {
    let registry = WidgetRegistry::builtins().expect("library registry");
    let node = ViewNode::new(WidgetId::ScrollBar)
        .prop("name", PropValue::Text("position".into()))
        .prop("content_extent", 1000.0.into())
        .prop("viewport_extent", 100.0.into())
        .prop("offset", 50.0.into());
    let mut forms = FormBindings::default();
    let mut widget = registry.realize(&node, &Theme::default(), &emitter(), &mut forms)
        .expect("state adapter");
    assert_eq!(forms.collect().get("position"), Some(&PropValue::Float(50.0)));
    let update = node.prop("offset", 150.0.into());
    assert!(registry.update_state(widget.as_mut(), &update, &Theme::default()).expect("in-place props"));
    assert_eq!(forms.collect().get("position"), Some(&PropValue::Float(150.0)),
        "the original native signal and form reader must survive the update");
}

#[test]
fn invalid_state_data_reports_the_property_and_node_path() {
    let node = ViewNode::new(WidgetId::VStack).child(
        ViewNode::new(WidgetId::ScrollBar)
            .prop("content_extent", true.into())
            .prop("name", "position".into())
            .prop("offset", 42.0.into()),
    );
    let diagnostics = Rc::new(RefCell::new(Vec::new()));
    let seen = diagnostics.clone();
    let registry = WidgetRegistry::builtins().expect("library registry")
        .on_diagnostic(move |error| seen.borrow_mut().push(error.clone()));
    let mut forms = FormBindings::default();
    let widget = registry.realize(&node, &Theme::default(), &emitter(), &mut forms)
        .expect("bad input keeps the default and the live control");
    assert_eq!(widget.base().children.len(), 1);
    assert_eq!(forms.collect().get("position"), Some(&PropValue::Float(42.0)));
    let diagnostics = diagnostics.borrow();
    assert_eq!(diagnostics.len(), 1);
    assert!(matches!(&diagnostics[0].kind, RealizeErrorKind::InvalidProperty { property, .. } if property == "content_extent"));
    assert_eq!(diagnostics[0].path, vec![0]);
}

#[test]
fn a_unsupported_widget_version_is_reported_before_building_the_tree() {
    let node = ViewNode::new(WidgetId::VStack).child(ViewNode::new(WidgetId::Label).schema_version(2));
    let registry = WidgetRegistry::builtins().expect("library registry");
    let Err(error) = registry.realize(&node, &Theme::default(), &emitter(), &mut FormBindings::default())
        else { panic!("unsupported contract version must be refused") };
    assert_eq!(error.kind, RealizeErrorKind::UnsupportedVersion {
        widget: WidgetId::Label, requested: 2, minimum: 1, maximum: 1,
    });
    assert_eq!(error.path, vec![0]);
}

#[test]
fn a_bad_description_never_runs_an_earlier_siblings_factory() {
    let called = Rc::new(Cell::new(0));
    let seen = called.clone();
    let mut registry = WidgetRegistry::builtins().expect("library registry");
    registry.register(WidgetDefinition::new("test.observed", move |_node, _cx| {
        seen.set(seen.get() + 1);
        Ok(Label::new("Called"))
    })).expect("host factory");
    let node = ViewNode::new(WidgetId::VStack)
        .child(ViewNode::new("test.observed"))
        .child(ViewNode::new("test.unknown"));
    assert!(registry.realize(&node, &Theme::default(), &emitter(), &mut FormBindings::default()).is_err());
    assert_eq!(called.get(), 0, "validate the complete tree before acquiring resources");
}
