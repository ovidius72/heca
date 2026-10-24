use super::*;
use crate::style::Length;
use crate::widgets::{Button, Input};

fn confirm() -> Dialog {
    Dialog::new("Delete pane?")
        .action(Button::new("Cancel"))
        .action(Button::new("Delete"))
        .size(crate::WidgetSize::Large)
        .default_open(true)
}

#[test]
fn short_dialogs_have_a_font_relative_floor_and_follow_font_changes() {
    let mut dialog = confirm();
    for font in [12.0, 15.0, 24.0, 12.0] {
        crate::LayoutEngine::new().base_font(font)
            .compute(&mut dialog, Size::new(1600.0, 900.0));
        let width = dialog.panel_bounds().size.w;
        assert!((width - 22.0 * font as f64).abs() < 1.0,
            "short content should have a 22em baseline at font {font}, got {width}");
    }
}

#[test]
fn larger_content_is_not_reduced_to_the_dialog_floor() {
    let mut dialog = Dialog::new("Rename pane")
        .body(Input::new().width("35em"))
        .size(crate::WidgetSize::Large)
        .default_open(true);
    crate::LayoutEngine::new().base_font(16.0)
        .compute(&mut dialog, Size::new(1600.0, 900.0));
    assert!(dialog.panel_bounds().size.w > 35.0 * 16.0,
        "the floor does not replace the content's natural width and padding");
}

#[test]
fn a_dialog_stays_inside_a_viewport_narrower_than_its_baseline() {
    let mut dialog = confirm();
    let mut engine = crate::LayoutEngine::new().base_font(20.0);
    for width in [800.0, 240.0, 160.0, 800.0] {
        let viewport = Size::new(width, 600.0);
        engine.compute(&mut dialog, viewport);
        let panel = dialog.panel_bounds();
        assert!(panel.loc.x >= 0.0 && panel.loc.x + panel.size.w <= width,
            "the font-relative baseline must give way to the viewport: {panel:?}");
    }
}

#[test]
fn explicit_panel_width_remains_an_explicit_size() {
    let mut dialog = confirm().panel_size(Length::Px(200.0), Length::Auto);
    crate::LayoutEngine::new().compute(&mut dialog, Size::new(800.0, 600.0));
    assert!((dialog.panel_bounds().size.w - 200.0).abs() < 1.0,
        "panel_size keeps its existing sizing contract");
}

#[test]
fn the_baseline_follows_the_inherited_size_variant() {
    for variant in [crate::WidgetSize::Small, crate::WidgetSize::Normal, crate::WidgetSize::Header] {
        let mut dialog = confirm().size(variant);
        crate::LayoutEngine::new().base_font(20.0)
            .compute(&mut dialog, Size::new(1600.0, 900.0));
        assert!((dialog.panel_bounds().size.w - 22.0 * 20.0 * variant.font_scale() as f64).abs() < 1.0);
    }
}
