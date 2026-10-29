use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::widgets::Flex;
use heca_grid_ui::{Component, LayoutEngine, Size, Theme, WidgetSize};
use heca_view::build::{Style, VStack};
use heca_view_realize::{FormBindings, IntentEmitter, realize};
use std::rc::Rc;

#[test]
fn sdk_and_native_limits_have_identical_font_relative_layouts_after_a_wire_round_trip() {
    let node = VStack::new()
        .width("2em").min_width("3em").max_width("5em")
        .height("8em").min_height("1em").max_height("4em")
        .margin_left("0.5em").margin_top("1em").into_node();
    let encoded = serde_json::to_string(&node).unwrap();
    let node: heca_view::ViewNode = serde_json::from_str(&encoded).unwrap();
    let emit: IntentEmitter = Rc::new(|_| {});
    let mut described = realize(&node, &Theme::default(), &emit, &mut FormBindings::default()).expect("valid described test widget");
    let mut native = Flex::column()
        .width("2em").min_width("3em").max_width("5em")
        .height("8em").min_height("1em").max_height("4em")
        .margin_left("0.5em").margin_top("1em");
    for font in [12.0, 24.0, 12.0] {
        let mut engine = LayoutEngine::new().base_font(font);
        let viewport = Size::new(800.0, 600.0);
        engine.compute(&mut native, viewport);
        engine.compute(described.as_mut(), viewport);
        let bounds = native.base().bounds;
        assert_eq!(bounds, described.base().bounds);
        let resolved_font = native.base().font as f64;
        assert!((bounds.size.w - 3.0 * resolved_font).abs() < 1.0);
        assert!((bounds.size.h - 4.0 * resolved_font).abs() < 1.0);
        assert_eq!(bounds.loc.x, 0.5 * resolved_font);
        assert_eq!(bounds.loc.y, resolved_font);
    }
}

#[test]
fn an_em_minimum_below_natural_content_does_not_reduce_that_content() {
    let mut row = Flex::row().size(WidgetSize::Large)
        .min_width("2em").child(Flex::column().width("8em").height("1em"));
    LayoutEngine::new().base_font(16.0).compute(&mut row, Size::new(800.0, 600.0));
    assert_eq!(row.base().bounds.size.w, 128.0);
}

#[test]
fn serialized_layouts_keep_em_lengths_instead_of_freezing_a_font_size() {
    let layout = heca_grid_ui::style::Layout {
        min_width: Some("22em".into()),
        ..Default::default()
    };
    let encoded = serde_json::to_value(layout).unwrap();
    assert_eq!(encoded["min_width"], "22em");
    assert_eq!(serde_json::from_value::<heca_grid_ui::style::Layout>(encoded).unwrap(), layout);
}
