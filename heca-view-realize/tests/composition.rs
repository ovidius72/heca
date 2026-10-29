//! Generic wrappers have the same composition, geometry and input on both authoring paths.

use std::{cell::RefCell, rc::Rc};
use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::reactive::SignalGet;
use heca_grid_ui::{Button, ChromeRegion, Component, DrawCommand, Event, Flex, FocusScope,
    FrameStyle, Label, LayoutEngine, PaintCx, Pane, RegionMode, Scene, Size, Theme, WidgetIntent};
use heca_view::build::{self as view, Parent as _, Style as _};
use heca_view::{Intent, ViewFrameStyle, ViewNode, ViewRegionMode};
use heca_view_realize::{FormBindings, IntentEmitter, realize};

fn scene(widget: &mut dyn Component, size: Size) -> Vec<DrawCommand> {
    LayoutEngine::new().compute(widget, size);
    let theme = Theme::default();
    let mut scene = Scene::new();
    widget.paint(&mut PaintCx::new(&mut scene, &theme));
    scene.iter().cloned().collect()
}

#[test]
fn a_described_pane_composes_mixed_content_with_the_native_frame_and_layout() {
    let mut native = Pane::new().border_style(FrameStyle::Bracketed).padding(4.0)
        .width("100%").height("100%")
        .child(Flex::row().child(Label::new("Content")).child(Button::new("Run")));
    let node: ViewNode = view::Pane::new().border_style(ViewFrameStyle::Bracketed).padding(4.0)
        .width("100%").height("100%")
        .child(view::HStack::new().child(view::Label::new("Content")).child(view::Button::new().text("Run")))
        .into();
    let emit: IntentEmitter = Rc::new(|_| {});
    let mut described = realize(&node, &Theme::default(), &emit, &mut FormBindings::default())
        .expect("generic pane adapter");
    for size in [Size::new(320.0, 160.0), Size::new(180.0, 80.0)] {
        assert_eq!(scene(&mut native, size), scene(described.as_mut(), size));
        assert_eq!(native.base().bounds, described.base().bounds);
        assert!(described.base().bounds.size.w <= size.w);
        assert!(described.base().bounds.size.h <= size.h);
    }
    assert_eq!(described.base().children[0].base().children.len(), 2);
}

#[test]
fn a_described_focus_scope_paints_the_native_ring_and_routes_activation_to_its_child() {
    let node: ViewNode = view::FocusScope::new(
        view::Button::new().text("Run").on_press(Intent::new("example.run")),
    ).keeps_keyboard(false).radius(3.0).into();
    let fired = Rc::new(RefCell::new(Vec::new()));
    let seen = fired.clone();
    let emit: IntentEmitter = Rc::new(move |intent| seen.borrow_mut().push(intent));
    let mut described = realize(&node, &Theme::default(), &emit, &mut FormBindings::default())
        .expect("generic focus scope adapter");
    let mut native = FocusScope::new(Button::new("Run")).keeps_keyboard(false).radius(3.0);
    native.base().children[0].base().focus(true);
    let native_scene = scene(&mut native, Size::new(240.0, 100.0));
    described.base().children[0].base().focus(true);
    assert_eq!(native_scene, scene(described.as_mut(), Size::new(240.0, 100.0)));
    heca_grid_ui::component::dispatch(described.as_mut(), &Event::Widget(WidgetIntent::Activate));
    assert_eq!(fired.borrow().iter().map(|intent| intent.action.as_str()).collect::<Vec<_>>(), vec!["example.run"]);
}

#[test]
fn a_described_chrome_region_uses_native_orientation_and_collapse_behavior() {
    for horizontal in [false, true] {
        let native = if horizontal { ChromeRegion::horizontal() } else { ChromeRegion::vertical() };
        let mut native = native.mode(RegionMode::CollapsedRail).rail_size(44.0).child(Label::new("Dock"));
        let described = if horizontal { view::ChromeRegion::horizontal() } else { view::ChromeRegion::vertical() };
        let node: ViewNode = described.mode(ViewRegionMode::CollapsedRail).rail_size(44.0)
            .child(view::Label::new("Dock")).into();
        let emit: IntentEmitter = Rc::new(|_| {});
        let mut described = realize(&node, &Theme::default(), &emit, &mut FormBindings::default())
            .expect("generic chrome region adapter");
        for size in [Size::new(320.0, 160.0), Size::new(180.0, 80.0)] {
            assert_eq!(scene(&mut native, size), scene(described.as_mut(), size));
            assert_eq!(native.base().bounds, described.base().bounds);
        }
        assert_eq!(native.mode_signal().get_untracked(), RegionMode::CollapsedRail);
    }
}
