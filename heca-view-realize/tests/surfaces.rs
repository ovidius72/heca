//! Stateful descriptions drive the existing native controls and props seam.
use std::{cell::RefCell, rc::Rc};
use heca_grid_ui::{Component, Dialog, Event, Handled, Label, LayoutEngine, LayoutExt,
    PaintCx, Scene, Size, Theme, WidgetIntent};
use heca_view::{build::{self as view, Parent as _, Style as _}, Intent, PropValue, ViewNode};
use heca_view_realize::{FormBindings, IntentEmitter, WidgetRegistry};

fn emitter() -> (IntentEmitter, Rc<RefCell<Vec<Intent>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    (Rc::new(move |intent| sink.borrow_mut().push(intent)), events)
}

fn layout(widget: &mut dyn Component, size: Size) {
    widget.tick(0.3);
    LayoutEngine::new().compute(widget, size);
    widget.paint(&mut PaintCx::new(&mut Scene::new(), &Theme::default()));
}

#[test]
fn dialog_common_sizing_and_modal_lifecycle_match_native_at_different_viewports() {
    let registry = WidgetRegistry::builtins().unwrap();
    let (emit, events) = emitter();
    let node = view::Dialog::new("Items").width("60%").height("70%").max_width(500.0)
        .child(view::Label::new("Choose an item"))
        .on_dismiss(Intent::new("cancel")).opened(true);
    let mut widget = registry.realize(&node, &Theme::default(), &emit, &mut FormBindings::default()).unwrap();
    let mut native = Dialog::new("Items").width("60%").height("70%").max_width(500.0)
        .child(Label::new("Choose an item")).default_open(true);
    for size in [Size::new(1000.0, 700.0), Size::new(240.0, 150.0), Size::new(1000.0, 700.0)] {
        layout(widget.as_mut(), size); layout(&mut native, size);
        assert_eq!(widget.base().bounds, native.base().bounds);
        assert_eq!(widget.base().children[0].base().children[0].base().bounds,
            native.base().children[0].base().children[0].base().bounds);
        assert!(widget.overlay_occludes(heca_grid_ui::Point::new(size.w - 1.0, size.h - 1.0)));
    }
    for _ in 0..2 {
        assert_eq!(heca_grid_ui::dispatch(widget.as_mut(), &Event::Widget(WidgetIntent::Dismiss)), Handled::Yes);
        assert!(!widget.overlay_active());
        assert!(registry.update_state(widget.as_mut(), node.as_ref(), &Theme::default()).unwrap());
        layout(widget.as_mut(), Size::new(800.0, 600.0));
        assert!(widget.overlay_active());
    }
    assert_eq!(events.borrow().iter().filter(|event| event.action == "cancel").count(), 2);
}

#[test]
fn palette_edits_its_native_input_reports_forms_runs_intents_and_reopens() {
    let registry = WidgetRegistry::builtins().unwrap();
    let (emit, events) = emitter();
    let command = Intent::new("open.editor").arg("pane", 42_i64.into());
    let node = view::CommandPalette::new().name("query").opened(true)
        .on_dismiss(Intent::new("palette.cancel"))
        .mode('@', "editors").mode('>', "actions")
        .command(view::Command::new("Neovim", command.clone()).mode("editors"))
        .command(view::Command::new("Build", Intent::new("build")).mode("actions"));
    let mut forms = FormBindings::default();
    let mut widget = registry.realize(&node, &Theme::default(), &emit, &mut forms).unwrap();
    layout(widget.as_mut(), Size::new(800.0, 600.0));
    assert_eq!(heca_grid_ui::dispatch(widget.as_mut(), &Event::TextInput("nv".into())), Handled::Yes);
    assert_eq!(forms.collect().get("query"), Some(&PropValue::Text("nv".into())));
    heca_grid_ui::dispatch(widget.as_mut(), &Event::Widget(WidgetIntent::Activate));
    assert_eq!(&*events.borrow(), &[command]);
    assert!(!widget.is_shown());
    registry.update_state(widget.as_mut(), node.as_ref(), &Theme::default()).unwrap();
    layout(widget.as_mut(), Size::new(800.0, 600.0));
    assert!(widget.is_shown());
    assert_eq!(forms.collect().get("query"), Some(&PropValue::Text("".into())), "native acceptance clears the query without replacing its form binding");
    heca_grid_ui::dispatch(widget.as_mut(), &Event::Widget(WidgetIntent::Dismiss));
    assert!(!widget.is_shown());
    assert_eq!(events.borrow().last().unwrap().action, "palette.cancel");
}

#[test]
fn described_palette_modes_keycaps_and_prefill_paint_like_native() {
    use heca_grid_ui::{Command, CommandPalette, KeyCap, NfGlyph};
    use heca_view::ViewNfGlyph;
    let mut native = CommandPalette::new().mode('@', "editors").mode('>', "actions")
        .command(Command::new("Build", || {}).mode("actions")
            .keys([KeyCap::Nf(NfGlyph::Command), KeyCap::Text("b".into())]))
        .query_in(Some("actions"), "bu").default_open(true);
    let described = view::CommandPalette::new().mode('@', "editors").mode('>', "actions")
        .command(view::Command::new("Build", Intent::new("build")).mode("actions")
            .keys([view::KeyCap::Nf(ViewNfGlyph::Command), view::KeyCap::Text("b".into())]))
        .query_in(Some("actions"), "bu").opened(true);
    let (emit, _) = emitter();
    let mut widget = WidgetRegistry::builtins().unwrap().realize(&described, &Theme::default(), &emit,
        &mut FormBindings::default()).unwrap();
    fn scene(widget: &mut dyn Component) -> Vec<heca_grid_ui::DrawCommand> {
        layout(widget, Size::new(800.0, 600.0));
        let mut scene = Scene::new(); widget.paint(&mut PaintCx::new(&mut scene, &Theme::default()));
        scene.iter().cloned().collect()
    }
    assert_eq!(scene(&mut native), scene(widget.as_mut()));
}

#[test]
fn queue_updates_preserve_native_card_identity_and_report_the_original_action_args() {
    let registry = WidgetRegistry::builtins().unwrap();
    let (emit, events) = emitter();
    let queue = || view::ToastStack::new().on_action(Intent::new("toast.action").arg("source", "plugin".into()))
        .item(view::ToastSpec::new(u64::MAX, "Build failed").action("retry", "Retry"));
    let mut widget = registry.realize(&queue(), &Theme::default(), &emit, &mut FormBindings::default()).unwrap();
    layout(widget.as_mut(), Size::new(800.0, 600.0));
    assert_eq!(widget.base().children.len(), 1);
    let first = widget.base().children[0].base().visible;
    let update: ViewNode = queue().item(view::ToastSpec::new(2, "Done")).into();
    assert!(registry.update_state(widget.as_mut(), &update, &Theme::default()).unwrap());
    layout(widget.as_mut(), Size::new(800.0, 600.0));
    assert_eq!(widget.base().children.len(), 2);
    assert_eq!(widget.base().children[0].base().visible, first);
    fn activate(widget: &mut dyn Component) -> bool {
        if widget.focusable() && widget.text_summary().as_deref() == Some("Retry") {
            widget.base().focus(true);
            heca_grid_ui::dispatch(widget, &Event::Widget(WidgetIntent::Activate)); return true;
        }
        widget.base_mut().children.iter_mut().any(|child| activate(child.as_mut()))
    }
    assert!(activate(widget.as_mut()));
    let events = events.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "toast.action");
    assert_eq!(events[0].args.get("toast_id"), Some(&PropValue::Text(u64::MAX.to_string())));
    assert_eq!(events[0].args.get("action"), Some(&PropValue::Text("retry".into())));
    assert_eq!(events[0].args.get("source"), Some(&PropValue::Text("plugin".into())));
}

#[test]
fn palette_entry_data_cannot_be_used_as_a_placeholder_widget() {
    let (emit, _) = emitter();
    assert!(WidgetRegistry::builtins().unwrap().realize(&ViewNode::new("command"),
        &Theme::default(), &emit, &mut FormBindings::default()).is_err());
}
