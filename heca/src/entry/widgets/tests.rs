use super::*;
use heca_grid_ui::Label;
use std::{cell::Cell, rc::Rc};

fn reset() { STARTED.set(false); REGISTRY.with(|registry| *registry.borrow_mut() = None); }
fn emitter() -> IntentEmitter { Rc::new(|_| {}) }

#[test]
fn a_host_addition_uses_the_apps_actual_entry_point_without_a_dispatcher_edit() {
    reset();
    register_widget(WidgetDefinition::new("custom.readout", |_, _| Ok(Label::new("Registered")))).unwrap();
    let duplicate = register_widget(WidgetDefinition::new("custom.readout", |_, _| Ok(Label::new("Wrong"))));
    assert!(matches!(duplicate.unwrap_err().kind, RealizeErrorKind::DuplicateWidget(_)));
    freeze();
    let widget = crate::chrome::realize(&ViewNode::new("custom.readout"), &Theme::default(),
        &emitter(), &mut FormBindings::default()).unwrap();
    assert_eq!(widget.text_summary().as_deref(), Some("Registered"));
    assert!(register_widget(WidgetDefinition::new("custom.late", |_, _| Ok(Label::new("Late")))).is_err());
}

#[test]
fn described_terminals_reuse_the_native_store_and_owner_cannot_come_from_wire_data() {
    reset();
    let node: ViewNode = heca_view::build::Terminal::new("described-sharing")
        .command("pwd").cwd("/tmp").title("Scratch").into();
    let emit = emitter();
    for _ in 0..2 { crate::chrome::realize(&node, &Theme::default(), &emit, &mut FormBindings::default()).unwrap(); }
    assert_eq!(crate::Terminal::new("described-sharing").shown_title().as_deref(), Some("Scratch"));
    let plugin = crate::extension("described_owner_test");
    let plugin_node = node.clone().prop("title", "Plugin scratch".into()).prop("owner", "heca".into());
    realize(plugin.name(), &plugin_node, &Theme::default(), &emit, &mut FormBindings::default()).unwrap();
    assert_eq!(plugin.terminal("described-sharing").shown_title().as_deref(), Some("Plugin scratch"));
    assert_eq!(crate::Terminal::new("described-sharing").shown_title().as_deref(), Some("Scratch"));
}

#[test]
fn malformed_terminal_identity_is_rejected_before_any_factory_acquires_resources() {
    reset();
    let called = Rc::new(Cell::new(false));
    let seen = called.clone();
    register_widget(WidgetDefinition::new("observe", move |_, _| { seen.set(true); Ok(Label::new("Observed")) })).unwrap();
    let node = ViewNode::new("v_stack").child(ViewNode::new("observe"))
        .child(ViewNode::new("terminal").prop("name", "another.owner".into()));
    let error = match crate::chrome::realize(&node, &Theme::default(), &emitter(), &mut FormBindings::default()) {
        Err(error) => error, Ok(_) => panic!("invalid identity must fail"),
    };
    assert_eq!(error.path, vec![1]);
    assert!(!called.get());
}
