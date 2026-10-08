//! **A pane's frame is painted over what the pane holds.**
//!
//! A widget paints before its children, so a border drawn in `paint` would sit under a terminal.
//! `Pane` asks for its frame to be drawn last (`PaintCx::outline`), and the scene keeps that order:
//! fill, then the children, then border and glow. Nothing about the order is left to the host.

mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::{Color, DrawCommand, LayoutEngine, Size, Theme};

/// The pane's fill, its child's marker, then the border: each told apart by its colour.
fn painted_order(pane: Pane) -> Vec<&'static str> {
    let mut root = pane;
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 100.0));
    let scene = common::paint_via_child(&root, &Theme::default());
    scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) if r.fill == Color::rgb(1, 2, 3) => Some("fill"),
            DrawCommand::Rect(r) if r.fill == Color::rgb(9, 9, 9) => Some("child"),
            DrawCommand::Rect(r) if r.border.is_some() => Some("border"),
            _ => None,
        })
        .collect()
}

#[test]
fn the_border_is_drawn_after_the_children() {
    let pane = Pane::new()
        .bordered()
        .background(Color::rgb(1, 2, 3))
        .child(Surface::new().background(Color::rgb(9, 9, 9)));
    assert_eq!(painted_order(pane), vec!["fill", "child", "border"]);
}
