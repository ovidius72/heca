//! External authoring: native and described trees share the same ordinary child builders.

use std::{cell::RefCell, rc::Rc};
use heca_grid_ui::builders::{Parent, PlaceExt};
use heca_grid_ui::{ChromeRegion, Component, Dialog, Event, Flex, ItemGroup, Label,
    LayoutEngine, Pane, Size, Tag, Theme, Tooltip, Visibility, WidgetIntent, WidgetModel};
use heca_grid_ui::widgets::{CardGrid, MenuItem};
use heca_view::{build as view, Intent};
use heca_view_realize::{FormBindings, IntentEmitter, realize};

fn dynamic_button() -> WidgetModel {
    let emit: IntentEmitter = Rc::new(|_| {});
    realize(&view::Button::new().text("Dynamic"), &Theme::default(), &emit,
        &mut FormBindings::default()).expect("typed descriptions are accepted directly")
}

#[test]
fn a_realized_typed_builder_places_directly_and_preserves_its_native_input_target() {
    let fired = Rc::new(RefCell::new(Vec::new()));
    let seen = fired.clone();
    let emit: IntentEmitter = Rc::new(move |intent| seen.borrow_mut().push(intent));
    let description = view::Button::new().text("Run").on_press(Intent::new("example.run"));
    let content = realize(&description, &Theme::default(), &emit, &mut FormBindings::default())
        .expect("no root into_node conversion");
    let mut root = Pane::new()
        .child(content.area("content"))
        .child(Label::new("Native sibling"));
    LayoutEngine::new().compute(&mut root, Size::new(320.0, 160.0));
    assert_eq!(root.base().children.len(), 2);
    let dynamic = &root.base().children[0];
    assert_eq!(dynamic.base().grid_area.as_deref(), Some("content"));
    assert!(dynamic.base().one_click_target, "erasure must retain the native Button node");
    assert_eq!(dynamic.base().children.len(), 1, "no additional wrapper node");
    assert_eq!(dynamic.base().children[0].text_summary().as_deref(), Some("Run"));
    dynamic.base().focus(true);
    heca_grid_ui::component::dispatch(&mut root, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(fired.borrow().iter().map(|intent| intent.action.as_str()).collect::<Vec<_>>(),
        vec!["example.run"]);
}

#[test]
fn specialized_slots_accept_a_realized_tree_without_author_conversions() {
    let visible = Visibility::new(dynamic_button(), true);
    assert_eq!(visible.base().children[0].text_summary().as_deref(), Some("Dynamic"));
    let tooltip = Tooltip::new(dynamic_button(), "Details");
    assert_eq!(tooltip.base().children[0].text_summary().as_deref(), Some("Dynamic"));
    let group = ItemGroup::new("Group").child(dynamic_button());
    assert!(group.base().children.iter().any(|child| child.text_summary().as_deref() == Some("Dynamic")));
    let region = ChromeRegion::vertical().dock(dynamic_button());
    assert_eq!(region.base().children[0].text_summary().as_deref(), Some("Dynamic"));
    let tag = Tag::new("First")
        .leading(dynamic_button())
        .segment(dynamic_button())
        .append_to_segment(1, dynamic_button())
        .segment_label_with(Label::new("Last"), dynamic_button());
    assert_eq!(tag.base().children.len(), 3);
    let dialog = Dialog::new("Question").action(dynamic_button());
    assert!(heca_grid_ui::area(&dialog, "dialog-actions").is_some());
    let cards = CardGrid::new().row(Vec::new(), dynamic_button());
    assert_eq!(cards.base().children.len(), 1);
    // This builder is lazy: its concrete result is erased by the menu, not by the author.
    let _entry = MenuItem::new().child(dynamic_button);
}

#[test]
fn homogeneous_widget_collections_and_dynamic_collections_use_the_same_child_builder() {
    let root = Flex::column()
        .child([Label::new("A"), Label::new("B")])
        .child(vec![dynamic_button(), dynamic_button()]);
    assert_eq!(root.base().children.len(), 4);
    assert!(root.base().children[2].base().one_click_target);
    assert!(root.base().children[3].base().one_click_target);
}

#[test]
fn menu_callbacks_return_an_ordinary_closure_and_keep_the_declared_intent() {
    let fired = Rc::new(RefCell::new(Vec::new()));
    let menu = heca_view_realize::menu_from_items(
        "Tools", "", "tools",
        vec![heca_view::DropdownItem::new("example.run", "Run")],
        &|_| None,
        &|intent| {
            let fired = fired.clone();
            move || fired.borrow_mut().push(intent.clone())
        },
    );
    menu.into_items()[0].activate();
    assert_eq!(fired.borrow()[0].action, "example.run");
}
