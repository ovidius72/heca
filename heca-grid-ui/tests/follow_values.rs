//! Builders that take a value which changes, and follow it: a minimum size, and what opens a
//! surface.

use heca_grid_ui::prelude::*;
use heca_grid_ui::{LayoutEngine, Length, Overlay, Size};

fn lay(root: &mut dyn Component) {
    LayoutEngine::new().compute(root, Size::new(600.0, 400.0));
}

#[test]
fn a_minimum_width_follows_a_value() {
    let narrowest = signal(Length::Px(100.0));
    let mut root = Flex::column().child(Flex::row().min_width(narrowest));
    lay(&mut root);
    assert_eq!(root.base().children[0].base().bounds.size.w, 100.0);
    narrowest.set(Length::Px(240.0));
    lay(&mut root);
    assert_eq!(
        root.base().children[0].base().bounds.size.w,
        240.0,
        "the next layout follows the new value"
    );
}

#[test]
fn a_surface_opens_while_a_value_reads_true() {
    let word = signal(String::new());
    let overlay = Overlay::new()
        .blocking(false)
        .keyboard(false)
        .open(move || !word.get().is_empty())
        .panel(Label::new("hi"));
    assert!(!overlay.open_signal().get());
    word.set("bo".into());
    assert!(overlay.open_signal().get(), "up once the value reads true");
    word.set(String::new());
    assert!(
        !overlay.open_signal().get(),
        "down again when it reads false"
    );
}
