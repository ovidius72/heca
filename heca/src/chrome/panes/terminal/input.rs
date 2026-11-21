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
    /// **A drag held past the terminal's edge** — scroll `lines` (negative: back into the history,
    /// positive: towards the live bottom) and stretch a selection to `cell`, the edge cell nearest
    /// the pointer. Sent every frame the pointer stays outside, with the mouse still.
    EdgeScroll {
        lines: i32,
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
    /// **Text to paste** — dropped files' paths, say. Pasted the way the clipboard is (bracketed
    /// when the program asked for it), so a shell takes it as one piece of text.
    Paste(String),
    /// **A key that is not text** — Enter, an arrow, Ctrl+C — with what was held when it went
    /// down. Whether the program wants it as an escape sequence is the process's own business.
    Key { key: GridKey, modifiers: Modifiers },
    /// **The terminal's box changed size** (or its font did): this is the grid it wants its process
    /// to have. Said once per change, not once per frame — and the message a client sends a server.
    Resize(Grid),
    /// **Something about the terminal's own search** — see [`Search`].
    Search(Search),
    /// **Its font one step bigger, smaller, or back to the app's** — asked of the terminal that
    /// holds the keyboard, wherever it is placed.
    FontZoom(crate::input::FontZoomStep),
    /// **Its history paged from the keyboard** — asked of the terminal that holds the keyboard,
    /// wherever it is placed, as of any scroll area.
    Page(Page),
}

/// One paging step through a terminal's history: what the scroll keys ask of any scroll area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    /// One page back.
    Up,
    /// One page on.
    Down,
    /// The oldest line.
    Top,
    /// The live bottom.
    Bottom,
}

impl Page {
    /// The widget intent that asks for this step.
    pub(crate) fn intent(self) -> heca_grid_ui::WidgetIntent {
        use heca_grid_ui::WidgetIntent;
        match self {
            Self::Up => WidgetIntent::ScrollPageUp,
            Self::Down => WidgetIntent::ScrollPageDown,
            Self::Top => WidgetIntent::ScrollToTop,
            Self::Bottom => WidgetIntent::ScrollToBottom,
        }
    }

    /// The step a widget intent asks for, if it is a vertical scroll intent — the inverse of
    /// [`intent`](Self::intent).
    pub(crate) fn of_intent(intent: heca_grid_ui::WidgetIntent) -> Option<Self> {
        [Self::Up, Self::Down, Self::Top, Self::Bottom]
            .into_iter()
            .find(|page| page.intent() == intent)
    }
}

/// What a terminal says about its **search**, which it runs as part of itself: the bar is its own
/// child, so it hears the field and says what the user did to it. The matches are the owner's to
/// find (it holds the process); the terminal paints what it is told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Search {
    /// The query field took the keyboard (`true`) or gave it up (`false`) — a click on it, a key
    /// that opened it, Enter that left it. The owner routes typing accordingly.
    Editing(bool),
    /// The query changed: this is the whole new text. Said once per edit.
    Query(String),
    /// Move to the next (`forward`) or previous match.
    Step { forward: bool },
    /// The search was dismissed: forget the matches.
    Close,
}

/// What the terminal's handle asks of its process. Unlike [`TerminalInput`], these are things a user
/// or a script *means to do*, so the owner turns each into the action of the same name — the one way
/// a key, the palette, RPC or any extension reaches a terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TerminalCommand {
    /// Type a line, and press Enter after it if `enter`.
    Run { text: String, enter: bool },
    /// End the terminal.
    Kill,
    /// Open the ordinary rename dialog for this terminal.
    Rename,
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

/// **What dropped files paste**: each full path, quoted for a POSIX shell when it needs it, followed
/// by a space — so `/tmp/a b.txt` and `/tmp/c.txt` become `'/tmp/a b.txt' /tmp/c.txt ` and the
/// command line reads them as two words, ready for the next one.
pub(crate) fn dropped_paths(paths: &[std::path::PathBuf]) -> String {
    paths.iter().map(|path| format!("{} ", shell_quoted(&path.to_string_lossy()))).collect()
}

/// `text` as one word for a POSIX shell: as it is when every character is safe, otherwise in single
/// quotes, a single quote inside written `'\''`.
fn shell_quoted(text: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "/._-+,:=@%~".contains(c);
    if !text.is_empty() && text.chars().all(safe) {
        return text.to_owned();
    }
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(test)]
mod dropped_paths_tests {
    use super::dropped_paths;
    use std::path::PathBuf;

    #[test]
    fn dropped_paths_paste_as_shell_words_followed_by_a_space() {
        let paste = |paths: &[&str]| dropped_paths(&paths.iter().map(PathBuf::from).collect::<Vec<_>>());
        assert_eq!(paste(&["/tmp/c.txt"]), "/tmp/c.txt ");
        assert_eq!(paste(&["/tmp/a b.txt", "/tmp/c.txt"]), "'/tmp/a b.txt' /tmp/c.txt ");
        assert_eq!(paste(&["/tmp/it's"]), r"'/tmp/it'\''s' ", "a quote inside a quoted word");
        assert_eq!(paste(&["/tmp/$HOME (1)"]), "'/tmp/$HOME (1)' ", "nothing is expanded");
    }
}
