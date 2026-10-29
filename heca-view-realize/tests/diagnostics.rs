//! Malformed inputs are observable without discarding usable widget content.

use std::{cell::RefCell, rc::Rc};
use heca_grid_ui::Theme;
use heca_view::{ViewNode, WidgetId};
use heca_view_realize::{FormBindings, IntentEmitter, RealizeErrorKind, WidgetRegistry};

#[test]
fn invalid_properties_keep_native_defaults_and_valid_siblings_with_original_paths() {
    let diagnostics = Rc::new(RefCell::new(Vec::new()));
    let seen = diagnostics.clone();
    let registry = WidgetRegistry::builtins().expect("library registry")
        .on_diagnostic(move |error| seen.borrow_mut().push(error.clone()));
    let node = ViewNode::new(WidgetId::VStack)
        .child(ViewNode::new(WidgetId::Label).text("Before"))
        .child(ViewNode::new(WidgetId::Scroll)
            .prop("axes", "sideways".into())
            .prop("overscroll", "enormous".into())
            .prop("gap", 6.into())
            .child(ViewNode::new(WidgetId::Label).text("Inside")))
        .child(ViewNode::new(WidgetId::Label).text("After"));
    let emit: IntentEmitter = Rc::new(|_| {});
    let widget = registry.realize(&node, &Theme::default(), &emit, &mut FormBindings::default())
        .expect("usable data must render");
    assert_eq!(widget.base().children.len(), 3);
    assert_eq!(widget.base().children[0].text_summary().as_deref(), Some("Before"));
    assert_eq!(widget.base().children[2].text_summary().as_deref(), Some("After"));
    assert_eq!(widget.base().children[1].base().style.layout.gap, heca_grid_ui::style::Space::Px(6.0));
    let diagnostics = diagnostics.borrow();
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.path == vec![1]));
    assert!(diagnostics.iter().any(|diagnostic| matches!(&diagnostic.kind,
        RealizeErrorKind::InvalidProperty { property, .. } if property == "axes")));
    assert_eq!(node.children[1].props["axes"].as_text(), Some("sideways"),
        "reporting and realization must not modify the owner's description");
}

#[test]
fn invalid_children_are_diagnosed_while_valid_options_remain_in_order() {
    let diagnostics = Rc::new(RefCell::new(Vec::new()));
    let seen = diagnostics.clone();
    let registry = WidgetRegistry::builtins().expect("library registry")
        .on_diagnostic(move |error| seen.borrow_mut().push(error.clone()));
    let node = ViewNode::new(WidgetId::VStack).child(ViewNode::new(WidgetId::Select)
        .child(ViewNode::new(WidgetId::Choice).text("First"))
        .child(ViewNode::new(WidgetId::Label).text("Not an option"))
        .child(ViewNode::new(WidgetId::Choice).text("Last")));
    let emit: IntentEmitter = Rc::new(|_| {});
    let widget = registry.realize(&node, &Theme::default(), &emit, &mut FormBindings::default())
        .expect("valid options must render");
    let options = &widget.base().children[0].base().children;
    assert_eq!(options.len(), 2);
    assert_eq!(options[0].text_summary().as_deref(), Some("First"));
    assert_eq!(options[1].text_summary().as_deref(), Some("Last"));
    let diagnostics = diagnostics.borrow();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].path, vec![0]);
    assert!(matches!(diagnostics[0].kind, RealizeErrorKind::InvalidChild { index: 1, .. }));
}

#[test]
fn an_unregistered_child_is_blocking_even_inside_a_semantic_container() {
    let node = ViewNode::new(WidgetId::Select).child(ViewNode::new("missing.widget"));
    let emit: IntentEmitter = Rc::new(|_| {});
    let result = WidgetRegistry::builtins().expect("library registry")
        .realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
    let Err(error) = result else { panic!("unknown widgets cannot be silently omitted") };
    assert!(matches!(error.kind, RealizeErrorKind::UnknownWidget(_)));
    assert_eq!(error.path, vec![0]);
}
