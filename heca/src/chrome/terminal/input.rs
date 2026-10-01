//! **What a terminal tells whoever owns its process** — typed messages, not actions.
//!
//! A terminal hears the pointer through the tree like any widget, works out where in its grid it
//! landed, and says so in a [`TerminalInput`]. It never reaches for the process itself: the
//! message goes through the seam its owner gave it ([`Seams::input`]) to one handler that holds the
//! policy needing state. In a client/server split this is the message the client sends the server.
//!
//! It is not an action. An action is something a user or a script *means to do* — it is listed in
//! the palette and callable by RPC — and a wheel turn per frame is not one.

use heca_core::layout::Rectangle;
use heca_grid_ui::Modifiers;

use super::viewport::ScrollIntents;

/// Where in the grid a pointer landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cell {
    pub row: usize,
    pub col: usize,
    /// How far into the cell, in pixels.
    pub x_offset: isize,
    pub y_offset: isize,
}

impl Cell {
    /// The cell under `pos` in a grid drawn at `rect` with cells of `cell_w` × `cell_h` — `None`
    /// outside the rect or when the cell size is not usable. **The one geometry path**: every
    /// reader of "which cell" goes through here.
    pub(crate) fn at(pos: (f64, f64), rect: Rectangle, cell_w: f64, cell_h: f64) -> Option<Self> {
        let x = pos.0 - rect.loc.x;
        let y = pos.1 - rect.loc.y;
        if x < 0.0 || y < 0.0 || x >= rect.size.w || y >= rect.size.h {
            return None;
        }
        if cell_w <= 0.0 || cell_h <= 0.0 || !cell_w.is_finite() || !cell_h.is_finite() {
            return None;
        }
        let col = (x / cell_w).floor() as usize;
        let row = (y / cell_h).floor() as usize;
        Some(Self {
            row,
            col,
            x_offset: (x - col as f64 * cell_w).round() as isize,
            y_offset: (y - row as f64 * cell_h).round() as isize,
        })
    }
}

/// One thing the pointer did to a terminal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TerminalInput {
    /// The wheel turned. Lines, positive `y` meaning the content moves down (the grid's
    /// convention); the cell is where the pointer was, if it was on the grid.
    Wheel {
        x: f32,
        y: f32,
        /// The device's own pixels, when it counts pixels (a trackpad) — same convention as `x`/`y`.
        pixels: Option<(f32, f32)>,
        cell: Option<Cell>,
        modifiers: Modifiers,
    },
}

/// **Everything a terminal's owner says to it, once**: what a click on the scrollback controls
/// means, and where its input goes. They travel as one group, built in `mod.rs`.
pub(crate) struct Seams {
    pub scroll: ScrollIntents,
    /// Where a [`TerminalInput`] goes.
    pub input: Box<dyn Fn(TerminalInput)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::{Point, Size};

    fn rect() -> Rectangle {
        Rectangle::new(Point::new(10.0, 20.0), Size::new(100.0, 50.0))
    }

    #[test]
    fn a_point_in_the_grid_is_a_cell_and_an_offset() {
        let c = Cell::at((10.0 + 25.0, 20.0 + 17.0), rect(), 10.0, 16.0).expect("inside");
        assert_eq!((c.row, c.col, c.x_offset, c.y_offset), (1, 2, 5, 1));
    }

    #[test]
    fn a_point_outside_the_grid_is_no_cell() {
        assert_eq!(Cell::at((9.0, 30.0), rect(), 10.0, 16.0), None);
        assert_eq!(
            Cell::at((60.0, 70.0), rect(), 10.0, 16.0),
            None,
            "the far edge is outside"
        );
    }

    #[test]
    fn an_unusable_cell_size_is_no_cell() {
        assert_eq!(Cell::at((20.0, 30.0), rect(), 0.0, 16.0), None);
        assert_eq!(Cell::at((20.0, 30.0), rect(), 10.0, f64::NAN), None);
    }
}
