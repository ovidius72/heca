//! Phase A integration tests: the reactive + layout + component model, headless.

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, LayoutEngine, PaintCx, Scene, Size, Theme};

/// A leaf box with a fixed size, for deterministic layout assertions.
fn fixed_box(w: f32, h: f32) -> Flex {
    Flex::column().width(Length::Px(w)).height(Length::Px(h))
}

#[test]
fn row_lays_children_left_to_right_with_gap() {
    let mut root = Flex::row()
        .gap(10.0)
        .width(Length::Px(300.0))
        .height(Length::Px(100.0))
        .child(fixed_box(50.0, 40.0))
        .child(fixed_box(50.0, 40.0));

    LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));

    assert_eq!(root.base().bounds.size.w, 300.0);
    let first = &root.base().children[0];
    let second = &root.base().children[1];
    assert_eq!(first.base().bounds.loc.x, 0.0);
    // second sits after the first (50px) plus the 10px gap.
    assert_eq!(second.base().bounds.loc.x, 60.0);
}

#[test]
fn padding_offsets_child_origin() {
    let mut root = Flex::column()
        .padding(10.0)
        .width(Length::Px(200.0))
        .height(Length::Px(200.0))
        .child(fixed_box(50.0, 50.0));

    LayoutEngine::new().compute(&mut root, Size::new(200.0, 200.0));

    let child = &root.base().children[0];
    assert_eq!(child.base().bounds.loc.x, 10.0);
    assert_eq!(child.base().bounds.loc.y, 10.0);
}

#[test]
fn flex_grow_absorbs_remaining_space() {
    let mut root = Flex::row()
        .width(Length::Px(200.0))
        .height(Length::Px(50.0))
        .child(fixed_box(40.0, 50.0))
        .child(Flex::row().grow(1.0).height(Length::Px(50.0)));

    LayoutEngine::new().compute(&mut root, Size::new(200.0, 50.0));

    let grower = &root.base().children[1];
    // 200 total - 40 fixed = 160 absorbed by the growing child.
    assert_eq!(grower.base().bounds.size.w, 160.0);
    assert_eq!(grower.base().bounds.loc.x, 40.0);
}

#[test]
fn signal_set_updates_value() {
    let count = signal(0);
    assert_eq!(count.get_untracked(), 0);
    count.set(7);
    assert_eq!(count.get_untracked(), 7);
}

#[test]
fn label_signal_drives_text() {
    let label = Label::new("ONLINE");
    let sig = label.text_signal();
    assert_eq!(sig.get_untracked(), "ONLINE");
    sig.set("OFFLINE".to_string());
    assert_eq!(sig.get_untracked(), "OFFLINE");
}

#[test]
fn paint_emits_background_rect_and_label_text() {
    let root = Flex::column()
        .background(Color::rgb(10, 10, 10))
        .child(Label::new("HI"));

    let theme = Theme::grid_tron();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        root.paint(&mut cx);
    }

    let rects = scene
        .iter()
        .filter(|c| matches!(c, DrawCommand::Rect(_)))
        .count();
    let texts = scene
        .iter()
        .filter(|c| matches!(c, DrawCommand::Text(_)))
        .count();
    assert_eq!(rects, 1, "container background should emit one rect");
    assert_eq!(texts, 1, "label should emit one text run");
}

#[test]
fn intensity_off_suppresses_glow() {
    let mut theme = Theme::grid_tron();
    theme.intensity = Intensity::Off;

    let root = Flex::column().glow(Color::rgb(64, 224, 255));
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        root.paint(&mut cx);
    }

    // The rect is still emitted, but its glow is stripped at Off intensity.
    let glow_present = scene.iter().any(|c| match c {
        DrawCommand::Rect(r) => r.glow.is_some(),
        _ => false,
    });
    assert!(!glow_present, "glow must be suppressed when intensity is Off");
}
