//! Phase A integration tests: the reactive + layout + component model, headless.

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, PaintCx, Point, Scene, Size, Theme};

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

// ── Phase C: surface components ──

#[test]
fn surface_paints_styled_rect_with_border() {
    let theme = Theme::grid_tron();
    let surface = Surface::new()
        .background(Color::rgb(12, 18, 24))
        .border(theme.accent, 1.5)
        .glow(theme.glow);

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        surface.paint(&mut cx);
    }
    let has_bordered = scene
        .iter()
        .any(|c| matches!(c, DrawCommand::Rect(r) if r.border.is_some() && r.glow.is_some()));
    assert!(has_bordered, "surface should emit a bordered, glowing rect");
}

#[test]
fn card_carries_title_label() {
    let theme = Theme::grid_tron();
    let card = Card::new("UPLINK").child(Label::new("ONLINE"));

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        card.paint(&mut cx);
    }
    let texts: Vec<&str> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"UPLINK"), "card title should render");
    assert!(texts.contains(&"ONLINE"), "card body should render");
}

#[test]
fn button_click_fires_within_bounds() {
    use std::cell::Cell;
    use std::rc::Rc;

    let clicked = Rc::new(Cell::new(false));
    let flag = clicked.clone();
    let mut button = Button::new("DEREZ").on_click(move || flag.set(true));
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));

    let b = button.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let outside = Point::new(b.loc.x + b.size.w + 100.0, b.loc.y);

    button.event(&Event::PointerPressed { pos: outside });
    assert!(!clicked.get(), "click outside bounds must not fire");
    button.event(&Event::PointerPressed { pos: center });
    assert!(clicked.get(), "click inside bounds must fire");
}

#[test]
fn button_hover_tracks_pointer() {
    let mut button = Button::new("HOVER");
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));
    let hovered = button.hovered();
    let b = button.base().bounds;

    button.event(&Event::PointerMoved {
        pos: Point::new(b.loc.x + 2.0, b.loc.y + 2.0),
    });
    assert!(hovered.get_untracked(), "entering bounds sets hover");
    button.event(&Event::PointerMoved {
        pos: Point::new(b.loc.x + b.size.w + 50.0, b.loc.y),
    });
    assert!(!hovered.get_untracked(), "leaving bounds clears hover");
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
