//! What a placed terminal and everyone holding it share, and what an extension declared about it.
//!
//! The pointer half of the component lives here too: it needs the shared state and nothing of the
//! node, so every view of the same terminal says the same thing.

use std::cell::{Cell, RefCell};

use heca_config::theme::ModifierKey;
use heca_core::backend::HyperlinkSpan;
use heca_core::layout::Rectangle;
use heca_grid_ui::Size;

use super::input::{Cell as GridCell, Grid, Search, TerminalInput};
use super::model::TerminalId;
use super::search::SearchSlot;
use super::viewport::{IntentSlot, Placed};
use crate::app::backend_store::Program;

/// What a placed terminal and everyone holding it share: its name, and how much room it was given.
pub(super) struct Shared {
    /// Which terminal process this shows, once whoever owns the processes has said.
    pub(super) id: Cell<Option<TerminalId>>,
    /// The size of the box the layout gave it, written each time it is laid out. `None` until the
    /// first layout.
    pub(super) size: Cell<Option<Size>>,
    /// Where it was drawn on the last frame it was on screen, in window coordinates — `None` when
    /// it was not drawn (scrolled out of view, or not in a tree). Said by whoever read the frame's
    /// surface requests.
    pub(super) placed: Cell<Option<Rectangle>>,
    /// What a click on the scrollback controls means and where input goes, said once by whoever
    /// owns the terminal.
    pub(super) intents: IntentSlot,
    /// One cell's size in logical pixels, as the last viewport said.
    pub(super) cell: Cell<(f32, f32)>,
    /// The box the terminal was last painted in — what its cells are counted from.
    pub(super) bounds: Cell<Rectangle>,
    /// The font's own cell size — what the grid is fitted from.
    pub(super) nominal: Cell<(f32, f32)>,
    /// The grid last asked of the process. A new one is asked only when this changes.
    pub(super) reported: Cell<Option<Grid>>,
    /// The scrollback controls of every node placed for this terminal.
    pub(super) controls: RefCell<Placed>,
    /// What an extension asked of it, if it placed it by name (`demo.terminal("shell")`).
    pub(super) declared: RefCell<Option<Declared>>,
    /// Its search: whether the bar is open, the query, and the matches it was told to show.
    pub(super) search: SearchSlot,
    /// The hyperlinks on screen this frame, as the backend captured them. **The one place** the
    /// question "where are the links" is answered: a click, a menu and the follow-link letters all
    /// ask the terminal.
    pub(super) links: RefCell<Vec<HyperlinkSpan>>,
    /// The stable row at the top of the screen and how many rows it shows — what turns a row the
    /// selection keeps into a row on screen.
    pub(super) view: Cell<(isize, usize)>,
    /// The key that makes a press the window's and a click on a link open it.
    pub(super) link_modifier: Cell<ModifierKey>,
}

/// **What an extension declared about a terminal it placed.** The name is the identity and never
/// changes (`demo.shell`); the title is only what the user reads, and defaults to the name.
pub(super) struct Declared {
    pub(super) name: String,
    pub(super) title: Option<String>,
    pub(super) program: Program,
    pub(super) cwd: Option<std::path::PathBuf>,
    /// Whether the host has already started (or failed to start) what was declared. Done once: a
    /// terminal that was later killed stays ended rather than starting again behind the user's back.
    pub(super) resolved: bool,
}

impl Shared {
    /// Say `input` to the terminal's owner. `false` when nobody has said where it goes.
    pub(super) fn emit(&self, input: TerminalInput) -> bool {
        match self.intents.borrow().as_ref() {
            Some(seams) => {
                (seams.input)(input);
                true
            }
            None => false,
        }
    }

    /// **Ask for the grid this box and font call for — once per change.** What it asked for last is
    /// kept, so a frame that changes nothing says nothing; a bound owner is told, an unbound one is
    /// not (and the grid stays unasked for, so it is said as soon as someone listens).
    pub(super) fn report_grid(&self) {
        let Some(room) = self.size.get() else {
            return;
        };
        let Some(grid) = Grid::fit(room, self.nominal.get()) else {
            return;
        };
        if self.reported.get() == Some(grid) {
            return;
        }
        if self.emit(TerminalInput::Resize(grid)) {
            self.reported.set(Some(grid));
        }
    }

    /// Ask the owner to step to the next or previous match. `false` when nobody listens.
    pub(super) fn step(&self, forward: bool) -> bool {
        self.emit(TerminalInput::Search(Search::Step { forward }))
    }

    /// The target of the link covering cell `(row, col)`. A span's start is inclusive and its end
    /// exclusive; capture never overlaps spans, so at most one matches.
    pub(super) fn link_at(&self, row: usize, col: usize) -> Option<String> {
        self.links
            .borrow()
            .iter()
            .find(|span| span.row == row && col >= span.start_col && col < span.end_col)
            .map(|span| span.uri.clone())
    }

    /// The target of the link under a pointer, if it is on one.
    pub(super) fn link_under(&self, p: &heca_grid_ui::PointerEvent) -> Option<String> {
        let cell = self.cell_at(p)?;
        self.link_at(cell.row, cell.col)
    }

    /// Whether the key that makes a press the window's is held.
    pub(super) fn reserved(&self, held: heca_grid_ui::Modifiers) -> bool {
        crate::modifier::held(self.link_modifier.get(), held)
    }

    /// Whether a press carries the key that makes it the window's, on the terminal itself.
    pub(super) fn held_for_window(&self, cx: &heca_grid_ui::event::EventCx<'_>) -> bool {
        cx.pointer().is_some_and(|p| {
            p.target_bounds == Some(self.bounds.get()) && self.reserved(p.modifiers)
        })
    }

    /// A click on a link, with the window's key held: ask the host to open it.
    pub(super) fn open_link(&self, cx: &mut heca_grid_ui::event::EventCx<'_>) {
        let Some(p) = cx.pointer().copied() else {
            return;
        };
        if p.target_bounds != Some(self.bounds.get()) || !self.reserved(p.modifiers) {
            return;
        }
        if let Some(url) = self.link_under(&p) {
            cx.dispatch(heca_view::Intent::new("open_link").arg("url", url.into()));
            cx.stop_propagation();
        }
    }

    /// The grid cell the pointer is on, if it is on the grid.
    pub(super) fn cell_at(&self, p: &heca_grid_ui::PointerEvent) -> Option<GridCell> {
        let (cell_w, cell_h) = self.cell.get();
        GridCell::at(
            (p.pos.x, p.pos.y),
            self.bounds.get(),
            cell_w as f64,
            cell_h as f64,
        )
    }

    /// A button or the pointer, on the terminal itself: say what it did, and where in the grid.
    /// Anything whose target is a descendant (the scrollback controls) is not the terminal's.
    pub(super) fn pointer(
        &self,
        cx: &mut heca_grid_ui::event::EventCx<'_>,
        make: impl Fn(&heca_grid_ui::PointerEvent, Option<GridCell>) -> TerminalInput,
    ) {
        if let Some(p) = cx.pointer().copied() {
            self.pointer_event(&p, make);
        }
    }

    /// [`pointer`](Self::pointer) for a pointer event already in hand.
    pub(super) fn pointer_event(
        &self,
        p: &heca_grid_ui::PointerEvent,
        make: impl Fn(&heca_grid_ui::PointerEvent, Option<GridCell>) -> TerminalInput,
    ) {
        if p.target_bounds != Some(self.bounds.get()) {
            return;
        }
        self.emit(make(p, self.cell_at(p)));
    }

    /// The wheel turned over the terminal.
    pub(super) fn wheel(&self, cx: &mut heca_grid_ui::event::EventCx<'_>) {
        let Some(p) = cx.pointer().copied() else {
            return;
        };
        let cell = self.cell_at(&p);
        let said = self.emit(TerminalInput::Wheel {
            x: p.delta_x,
            y: p.delta_y,
            pixels: p.delta_pixels,
            cell,
            modifiers: p.modifiers,
        });
        if said {
            cx.stop_propagation();
        }
    }
}
