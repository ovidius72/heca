//! [`ButtonGroup`] — the row of actions that fits the space it is given.
//!
//! Every assertion is made on the **laid-out tree** inside a parent of a known width, because that
//! is where a toolbar lives — a strip, a header, a card — and a group laid out as a root hugs its
//! content and can never learn the room was short.

#[path = "../common/mod.rs"]
mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{ButtonGroup, Display};
use heca_grid_ui::{LayoutEngine, Length, Size, WidgetSize};

fn group() -> ButtonGroup {
    ButtonGroup::new()
        .size(WidgetSize::Small)
        .child(Button::new("Split").icon(Glyph::Plus))
        .child(Button::new("Zoom").icon(Glyph::FrameCorners))
        .child(Button::new("Close").icon(Glyph::Minus))
}

/// Lay `g` out inside a parent `w` wide.
fn lay(g: ButtonGroup, w: f64) -> Flex {
    lay_n(g, w, 4)
}

fn lay_n(g: ButtonGroup, w: f64, passes: usize) -> Flex {
    let mut parent = Flex::row().width(Length::Px(w as f32)).child(g);
    for _ in 0..passes {
        LayoutEngine::new()
            .base_font(13.0)
            .compute(&mut parent, Size::new(w, 60.0));
    }
    parent
}

/// The buttons' row: parent → group → row.
fn row(parent: &Flex) -> &dyn Component {
    // The group **is** the row that arranges the buttons — they are its own children.
    parent.base().children[0].as_ref()
}

/// How many children are still laid out as buttons.
fn visible(parent: &Flex) -> usize {
    row(parent)
        .base()
        .children
        .iter()
        .filter(|c| !c.base().style.layout.hidden)
        .count()
}

/// Widths of the children still shown.
fn widths(parent: &Flex) -> Vec<f64> {
    row(parent)
        .base()
        .children
        .iter()
        .filter(|c| !c.base().style.layout.hidden)
        .map(|c| c.base().bounds.size.w)
        .collect()
}

mod overflow;
mod paint_states;
mod spacing;
mod stability;
