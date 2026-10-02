//! **What a terminal tells whoever owns its process** — typed messages, not actions.
//!
//! A terminal hears the pointer through the tree like any widget, works out where in its grid it
//! landed, and says so in a [`TerminalInput`]. It never reaches for the process itself: the
//! message goes through the seam its owner gave it ([`Seams::input`]) to one handler that holds the
//! policy needing state. In a client/server split this is the message the client sends the server.
//!
//! It is not an action. An action is something a user or a script *means to do* — it is listed in
//! the palette and callable by RPC — and a wheel turn per frame is not one.

use heca_core::layout::{Rectangle, Size};
use heca_grid_ui::{GridKey, Modifiers, PointerButton};

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

/// **How many cells a terminal's box holds, and how big each is** — the size it asks its process to
/// be. Reported only when it changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Grid {
    pub cols: usize,
    pub rows: usize,
    /// One cell's size once the box is divided exactly: the grid fills the box with no spare pixels.
    pub cell_w: f32,
    pub cell_h: f32,
}

impl Grid {
    /// The most columns or rows a grid can have.
    const MAX_UNITS: f32 = 16_384.0;

    /// **The grid that fills `room` with cells about `nominal` big** — as many as fit, rounded up so
    /// the last one is partly under the edge rather than leaving a gap, then each cell stretched to
    /// share the box exactly. `None` when the nominal cell is not a size yet.
    pub(crate) fn fit(room: Size, nominal: (f32, f32)) -> Option<Self> {
        let (nominal_w, nominal_h) = nominal;
        if !(nominal_w > 0.0 && nominal_h > 0.0 && nominal_w.is_finite() && nominal_h.is_finite()) {
            return None;
        }
        let units = |extent: f64, cell: f32| -> usize {
            if !(extent > 0.0 && extent.is_finite()) {
                return 1;
            }
            (extent as f32 / cell).ceil().clamp(1.0, Self::MAX_UNITS) as usize
        };
        let cols = units(room.w, nominal_w);
        let rows = units(room.h, nominal_h);
        Some(Self {
            cols,
            rows,
            cell_w: (room.w as f32 / cols as f32).max(1.0),
            cell_h: (room.h as f32 / rows as f32).max(1.0),
        })
    }
}

/// One thing the user did to a terminal: the pointer, or the keyboard while it held it.
#[derive(Clone, Debug, PartialEq)]
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
    /// A button went down on the terminal itself — not on its scrollback controls.
    Press {
        button: PointerButton,
        cell: Option<Cell>,
        modifiers: Modifiers,
    },
    /// A button came up over the terminal.
    Release {
        button: PointerButton,
        cell: Option<Cell>,
        modifiers: Modifiers,
    },
    /// The pointer moved over the terminal itself.
    Move {
        cell: Option<Cell>,
        modifiers: Modifiers,
    },
    /// **Scroll back to the live bottom** — the scrollback chip was clicked.
    ScrollToBottom,
    /// **Scroll to `rows` above the live bottom** — the scrollbar thumb moved.
    ScrollTo { rows: usize },
    /// **Text the user typed** — or pasted, or an IME composed — while the terminal held the
    /// keyboard, exactly as the platform produced it (case and shifted symbols intact).
    Text(String),
    /// **A key that is not text** — Enter, an arrow, Ctrl+C — with what was held when it went
    /// down. Whether the program wants it as an escape sequence is the process's own business.
    Key { key: GridKey, modifiers: Modifiers },
    /// **The terminal's box changed size** (or its font did): this is the grid it wants its process
    /// to have. Said once per change, not once per frame — and the message a client sends a server.
    Resize(Grid),
}

/// What the terminal's handle asks of its process. Unlike [`TerminalInput`], these are things a user
/// or a script *means to do*, so the owner turns each into the action of the same name — the one way
/// a key, the palette, RPC or heca-pro reaches a terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TerminalCommand {
    /// Type a line, and press Enter after it if `enter`.
    Run { text: String, enter: bool },
    /// End the terminal.
    Kill,
}

/// **Everything a terminal's owner says to it, once**: what a click on the scrollback controls
/// means, and where its input goes. They travel as one group, built in `mod.rs`.
pub(crate) struct Seams {
    pub scroll: ScrollIntents,
    /// Where a [`TerminalInput`] goes.
    pub input: Box<dyn Fn(TerminalInput)>,
    /// Where a [`TerminalCommand`] about the terminal with this id goes.
    pub command: Box<dyn Fn(super::TerminalId, TerminalCommand)>,
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

    #[test]
    fn a_box_is_filled_with_as_many_cells_as_it_takes() {
        // 100 wide at 8 px: 12.5 cells, so 13, each a little under 8 so they share the box.
        let g = Grid::fit(Size::new(100.0, 48.0), (8.0, 16.0)).expect("a size");
        assert_eq!((g.cols, g.rows), (13, 3));
        assert!((g.cell_w - 100.0 / 13.0).abs() < 1e-4 && (g.cell_h - 16.0).abs() < 1e-4);
    }

    #[test]
    fn a_grid_needs_a_real_cell_size_and_never_has_no_cells() {
        assert_eq!(Grid::fit(Size::new(100.0, 50.0), (0.0, 16.0)), None);
        assert_eq!(Grid::fit(Size::new(100.0, 50.0), (8.0, f32::NAN)), None);
        let tiny = Grid::fit(Size::new(0.0, -5.0), (8.0, 16.0)).expect("still a grid");
        assert_eq!((tiny.cols, tiny.rows), (1, 1));
    }
}
