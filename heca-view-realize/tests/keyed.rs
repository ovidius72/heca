use std::{cell::Cell, rc::Rc};
use heca_grid_ui::{Label, LayoutEngine, Size, Theme};
use heca_grid_ui::builders::IdentityExt;
use heca_view::{build::{Keyed, Parent as _}, ViewNode};
use heca_view_realize::{FormBindings, IntentEmitter, WidgetDefinition, WidgetRegistry};

#[test]
fn unchanged_keys_skip_child_factories_but_live_texts_still_update() {
    let builds = Rc::new(Cell::new(0));
    let seen = builds.clone();
    let mut registry = WidgetRegistry::builtins().unwrap();
    registry.register(WidgetDefinition::new("counted", move |_, _| {
        seen.set(seen.get() + 1); Ok(Label::new("Original").key("title"))
    })).unwrap();
    let emit: IntentEmitter = Rc::new(|_| {});
    let mut forms = FormBindings::default();
    let node = |key: &str, text: &str| Keyed::new().content_key(key)
        .texts([("title".into(), text.into())]).child(ViewNode::new("counted"));
    let mut widget = registry.realize(&node("a", "First"), &Theme::default(), &emit, &mut forms).unwrap();
    let size = Size::new(300.0, 100.0);
    LayoutEngine::new().compute(widget.as_mut(), size);
    let identity = widget.base().children[0].base().visible;
    assert_eq!(builds.get(), 1);
    registry.update(widget.as_mut(), &node("a", "Second"), &Theme::default(), &emit, &mut forms).unwrap();
    LayoutEngine::new().compute(widget.as_mut(), size);
    assert_eq!(builds.get(), 1);
    assert_eq!(widget.base().children[0].base().visible, identity);
    assert_eq!(widget.base().children[0].text_summary().as_deref(), Some("Second"));
    registry.update(widget.as_mut(), &node("b", "Third"), &Theme::default(), &emit, &mut forms).unwrap();
    LayoutEngine::new().compute(widget.as_mut(), size);
    assert_eq!(builds.get(), 2);
    assert_ne!(widget.base().children[0].base().visible, identity);
    assert_eq!(widget.base().children[0].text_summary().as_deref(), Some("Third"));
}
