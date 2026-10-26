//! The `Overlay` widget's own tests — what it shows, what it holds, and what it lets through.
use super::*;
use crate::builders::Parent;
use crate::event::PointerButton;
use crate::scene::{DrawCommand, HostCmd, HostDraw, Scene};
use crate::widgets::{Flex, Label};

/// An open modal, **laid out** — as a host has it before any input reaches it. A layer occupies
/// what it draws, and what it draws is decided by the layout pass: the scrim fills the viewport
/// it was given. (It used to answer with a `hit_bounds` rect large enough to cover any point at
/// all, even before it had been laid out or drawn anywhere.)
fn open_overlay() -> Overlay {
    let mut o = Overlay::new()
        .panel(Flex::column().child(Label::new("hi")))
        .default_open(true);
    crate::layout::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));
    o
}

mod layout;
mod lifecycle;
mod input;
