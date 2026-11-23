//! **A builder given a value that changes follows it in place** — `Visibility::visible(..)` and
//! `Icon::glyph(..)`, with no rebuild.

use heca_grid_ui::prelude::*;
use heca_grid_ui::reactive::{SignalGet, SignalUpdate, signal};
use heca_grid_ui::widgets::{Glyph, Visibility};
use heca_grid_ui::{DrawCommand, LayoutEngine, PaintCx, Scene, Size, Theme};

fn texts(widget: &dyn heca_grid_ui::Component) -> Vec<String> {
    let theme = Theme::default();
    let mut scene = Scene::new();
    heca_grid_ui::paint_child(widget, &mut PaintCx::new(&mut scene, &theme));
    scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect()
}

/// A `Visibility` given a function shows and hides its child as the signal it reads changes —
/// gone from the layout while hidden, back when shown.
#[test]
fn a_visibility_follows_a_live_value() {
    let shown = signal(false);
    let mut column = Flex::column().child(
        Visibility::new()
            .visible(move || shown.get())
            .child(Label::new("here")),
    );
    let size = Size::new(200.0, 100.0);
    LayoutEngine::new().compute(&mut column, size);
    assert!(
        !texts(&column).contains(&"here".to_string()),
        "hidden to begin with"
    );
    shown.set(true);
    LayoutEngine::new().compute(&mut column, size);
    assert!(
        texts(&column).contains(&"here".to_string()),
        "shown once the value says so"
    );
    shown.set(false);
    LayoutEngine::new().compute(&mut column, size);
    assert!(
        !texts(&column).contains(&"here".to_string()),
        "and hidden again"
    );
}

/// An `Icon` given a function draws the glyph the signal it reads names now.
#[test]
fn an_icon_follows_a_live_glyph() {
    let which = signal(Glyph::Plus);
    let mut icon = Icon::new(Glyph::Circle).glyph(move || which.get());
    let size = Size::new(100.0, 100.0);
    let drawn = |icon: &mut Icon, glyph: Glyph| {
        LayoutEngine::new().compute(icon, size);
        let ch = glyph.primary_char().expect("a glyph");
        texts(icon).iter().any(|t| {
            t.chars()
                .next()
                .is_some_and(|c| (c as u32).abs_diff(ch as u32) <= 1)
        })
    };
    assert!(
        drawn(&mut icon, Glyph::Plus),
        "the live glyph, not the one it was made with"
    );
    which.set(Glyph::Minus);
    assert!(drawn(&mut icon, Glyph::Minus), "it follows the change");
    assert!(!drawn(&mut icon, Glyph::Plus), "and the old one is gone");
}
