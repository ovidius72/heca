use heca_grid_ui::{Button, Component, Dialog, Event, Handled, Label, LayoutEngine, LayoutExt,
    Point, ScrollRegion, Size, WidgetIntent};
use heca_grid_ui::event::PointerEvent;
use heca_grid_ui::builders::Parent;

fn dialog() -> Dialog {
    Dialog::new("Choose an item")
        .width(320.0).height(180.0)
        .child(ScrollRegion::new().child((0..30).map(|i| Label::new(format!("Item {i}"))).collect::<Vec<_>>()))
        .action(Button::new("Cancel"))
        .default_open(true)
}

#[test]
fn common_dimensions_size_the_visible_panel_and_keep_viewport_modal_coverage() {
    let mut dialog = dialog();
    LayoutEngine::new().compute(&mut dialog, Size::new(800.0, 600.0));
    let overlay = &dialog.base().children[0];
    let panel = &overlay.base().children[0];
    assert!((panel.base().bounds.size.w - 320.0).abs() < 1.0);
    assert!((panel.base().bounds.size.h - 180.0).abs() < 1.0);
    assert_eq!(dialog.base().bounds.size, Size::new(800.0, 600.0));
    assert_eq!(overlay.base().bounds.size, Size::new(800.0, 600.0));
    assert!((panel.base().bounds.loc.x + 160.0 - 400.0).abs() < 1.0);
    assert!((panel.base().bounds.loc.y + 90.0 - 300.0).abs() < 1.0);
    let body = heca_grid_ui::area(&dialog, "dialog-body").expect("named body");
    assert!(body.base().bounds.size.h < 180.0, "title and actions reserve their own room");
    assert_eq!(heca_grid_ui::dispatch(&mut dialog,
        &Event::PointerDown(PointerEvent::at(Point::new(790.0, 590.0)))), Handled::Yes);
    assert_eq!(heca_grid_ui::dispatch(&mut dialog, &Event::Widget(WidgetIntent::Dismiss)), Handled::Yes);
}

#[test]
fn common_maximum_and_minimum_constraints_survive_resizing_and_font_changes() {
    let mut dialog = dialog().width("80%").height("70%").max_width(400.0).min_width(120.0);
    for (width, height, font) in [(1000.0, 700.0, 14.0), (240.0, 180.0, 20.0), (1000.0, 700.0, 14.0)] {
        LayoutEngine::new().base_font(font).compute(&mut dialog, Size::new(width, height));
        let panel = &dialog.base().children[0].base().children[0];
        let bounds = panel.base().bounds;
        assert!(bounds.size.w <= 401.0 && bounds.size.w <= width);
        assert!(bounds.loc.x >= 0.0 && bounds.loc.x + bounds.size.w <= width + 1.0);
        assert!(bounds.loc.y >= 0.0 && bounds.loc.y + bounds.size.h <= height + 1.0);
        if width > 400.0 { assert!(bounds.size.w > 390.0, "the narrow viewport must not permanently lower the authored maximum"); }
    }
}
