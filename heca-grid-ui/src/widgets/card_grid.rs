//! [`CardGrid`] — rows of columns of selectable cards, with a two-axis cursor.
//!
//! The generic half of a "map" surface: a workspace overview, a file grid, an emoji picker. It owns
//! **where the selection is** and nothing about what the cards mean — a caller supplies the cards
//! and reads back which one was chosen.
//!
//! It answers the shared navigation vocabulary rather than inventing keys:
//! [`WidgetIntent::ItemPrevious`]/[`ItemNext`](WidgetIntent::ItemNext) are the horizontal axis,
//! [`MenuUp`](WidgetIntent::MenuUp)/[`MenuDown`](WidgetIntent::MenuDown) the vertical one — the same
//! split `[keys.widgets]` already configures, so a host binds nothing new.
//!
//! **The selection moves by writing a signal per card**, never by rebuilding: each card is handed a
//! `Signal<bool>` it lights itself from, the way [`CommandPalette`](super::CommandPalette) moves its
//! match marks.

use crate::component::{Base, Component, Event, Handled, WidgetIntent};
use crate::reactive::{Signal, SignalGet, SignalUpdate};

/// One selectable cell: the caller's key for it, and the signal that lights it.
///
/// Deliberately **not** [`Card`](super::Card), which is a visual surface. This is the cursor's unit
/// of selection; what it looks like is whatever component the caller puts in `body`.
pub struct GridCell {
    key: String,
    selected: Signal<bool>,
    /// The card's own hover signal, when the caller wired one — see [`GridCell::hovered`].
    hovered: Option<Signal<bool>>,
}

impl GridCell {
    /// A card identified by `key`, lighting `selected` when the cursor is on it.
    ///
    /// The key is the caller's own — a pane id, a path, a codepoint. `CardGrid` never interprets
    /// it; it hands it back on activation, which is the whole of what the caller needs.
    /// A cell **carries no body**: the caller draws the card inside the row layout it passes to
    /// [`CardGrid::row`], so there is one place the visuals live rather than two that can disagree.
    pub fn new(key: impl Into<String>, selected: Signal<bool>) -> Self {
        Self { key: key.into(), selected, hovered: None }
    }

    /// Wire the card's **hover** signal, so pointing at a card moves the cursor onto it.
    ///
    /// The cursor stays single-valued: hovering *moves* it rather than raising a second claim
    /// beside it, so the keyboard and the mouse can never disagree about where you are — and
    /// anything watching the cursor (a scroll region centring it, a preview pane) follows the mouse
    /// for free, with nothing wired per surface.
    ///
    /// Optional: a grid whose caller passes no hover signal behaves exactly as before.
    pub fn hovered(mut self, hovered: Signal<bool>) -> Self {
        self.hovered = Some(hovered);
        self
    }
}

/// One column's cells, top to bottom: their key, the signal that lights them, and their hover.
type Cells = Vec<(String, Signal<bool>, Option<Signal<bool>>)>;
/// One row's columns, left to right.
type Columns = Vec<Cells>;

/// What a caller runs when a cell is chosen — it is handed the caller's own key.
type OnActivate = Box<dyn Fn(&str)>;

/// A two-axis grid of [`GridCell`]s with a cursor over them.
pub struct CardGrid {
    base: Base,
    /// Per row, the cards left to right: their keys and their light signals.
    rows: Vec<Columns>,
    /// Per row, an optional signal lit while the cursor is **anywhere in that row**. See
    /// [`row_cursor`](CardGrid::row_cursor).
    row_cursors: Vec<Option<Signal<bool>>>,
    /// (row, column, index within the column) — three axes, because a map has three.
    cursor: (usize, usize, usize),
    /// Per row, the `(column, cell)` the cursor was last on there.
    ///
    /// **Leaving a row and coming back returns to where you were in it.** Without this the cursor
    /// carries its column index across and clamps, so stepping through a row of one column and back
    /// lands on the first card every time — the position is quietly destroyed by the trip.
    row_marks: Vec<(usize, usize)>,
    on_activate: Option<OnActivate>,
    /// Told the key the cursor moved onto, every time it moves. See [`on_move`](CardGrid::on_move).
    on_move: Option<OnActivate>,
    on_dismiss: Option<Box<dyn Fn()>>,
}

#[heca_grid_ui_macros::props]
impl CardGrid {
    /// An empty grid.
    pub fn new() -> Self {
        let mut base = Base::new();
        // Rows stack **vertically** — they are rows. Without saying so the default direction lays
        // them side by side, which turned a stack of workspaces into a line of them.
        base.style.layout.direction = crate::style::Direction::Column;
        Self {
            base,
            rows: Vec::new(),
            row_marks: Vec::new(),
            row_cursors: Vec::new(),
            cursor: (0, 0, 0),
            on_activate: None,
            on_move: None,
            on_dismiss: None,
        }
    }

    /// Add a row of cards, laid out left to right by `layout`.
    ///
    /// The caller builds the row's own container — a `Flex::row`, a labelled strip, whatever the
    /// surface needs — because how a row *looks* is the caller's, while where the cursor *is* is
    /// this widget's. `cards` must be in the same left-to-right order the layout draws them, or the
    /// cursor and the picture disagree.
    #[heca_grid_ui_macros::host_only("a row of composed columns, not a scalar")]
    pub fn row(mut self, columns: Vec<Vec<GridCell>>, layout: impl Component + 'static) -> Self {
        self.rows.push(
            columns
                .iter()
                .map(|col| col.iter().map(|c| (c.key.clone(), c.selected, c.hovered)).collect())
                .collect(),
        );
        self.base.children.push(Box::new(layout));
        self.row_marks.push((0, 0));
        self
    }

    /// Bind a signal lit while the cursor is anywhere in the **row just added**.
    ///
    /// The per-cell signals answer "is the cursor on *this card*"; a surface often needs the
    /// coarser question — which row you are in — to light the row, position a map on it, or decide
    /// which of several regions is the one that matters. Deriving that from the cell signals means
    /// every caller writing the same fold, and getting the empty-row case wrong: a row with no
    /// cards is still a row you can stand on, and no cell signal would ever say so.
    ///
    /// Chained after [`row`](Self::row) rather than passed to it, so the common case stays two
    /// arguments and the signal can be created before the row's layout that binds it.
    #[heca_grid_ui_macros::host_only("a live signal, not a scalar")]
    pub fn row_cursor(mut self, lit: Signal<bool>) -> Self {
        if let Some(slot) = self.row_cursors.last_mut() {
            *slot = Some(lit);
        }
        self.sync();
        self
    }

    /// Called with the card's key **each time the cursor moves onto it** — keyboard, hover, or a
    /// programmatic selection.
    ///
    /// What a host uses to keep the position somewhere that outlives the widget. A surface rebuilt
    /// from scratch every time it opens has no memory of its own, so "open where I left it" is
    /// something only the host can answer; this is how the host hears the answer.
    #[heca_grid_ui_macros::host_only("a callback, not a scalar")]
    pub fn on_move(mut self, f: impl Fn(&str) + 'static) -> Self {
        self.on_move = Some(Box::new(f));
        self
    }

    /// Called with the selected card's key when the user activates it.
    #[heca_grid_ui_macros::host_only("a callback, not a scalar — behaviour crosses as an Intent")]
    pub fn on_activate(mut self, f: impl Fn(&str) + 'static) -> Self {
        self.on_activate = Some(Box::new(f));
        self
    }

    /// Called when the user dismisses the grid.
    #[heca_grid_ui_macros::host_only("a callback, not a scalar")]
    pub fn on_dismiss(mut self, f: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    /// Start the cursor on the card with this key, if it is present.
    ///
    /// What a caller uses to open the surface **where the user already is** rather than at the
    /// corner. An unknown key leaves the cursor at the start, so a stale id cannot break opening.
    #[heca_grid_ui_macros::prop]
    pub fn selected(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        if let Some(at) = self.rows.iter().enumerate().find_map(|(r, row)| {
            row.iter().enumerate().find_map(|(c, col)| {
                col.iter().position(|(k, ..)| *k == key).map(|i| (r, c, i))
            })
        }) {
            self.cursor = at;
        }
        self.sync();
        self
    }

    /// The key the cursor is on, if the grid has any cards.
    pub fn selected_key(&self) -> Option<&str> {
        self.rows
            .get(self.cursor.0)
            .and_then(|r| r.get(self.cursor.1))
            .and_then(|c| c.get(self.cursor.2))
            .map(|(k, ..)| k.as_str())
    }

    /// Light the card under the cursor, darken the rest, and tell the host where the cursor now is.
    ///
    /// One signal write per card, never a rebuild.
    fn sync(&self) {
        if let (Some(f), Some(key)) = (&self.on_move, self.selected_key()) {
            f(key);
        }
        self.sync_lights();
    }

    fn sync_lights(&self) {
        for (r, lit) in self.row_cursors.iter().enumerate() {
            let Some(lit) = lit else { continue };
            let on = r == self.cursor.0;
            if lit.get_untracked() != on {
                lit.set(on);
            }
        }
        for (r, row) in self.rows.iter().enumerate() {
            for (c, col) in row.iter().enumerate() {
                for (i, (_, sig, _)) in col.iter().enumerate() {
                    let on = (r, c, i) == self.cursor;
                    if sig.get_untracked() != on {
                        sig.set(on);
                    }
                }
            }
        }
    }

    /// Move the cursor on one of the **three axes**, each clamped at its ends.
    ///
    /// No wrap-around: a map you can fall off the end of is disorienting, and every list widget
    /// here stops at its ends for the same reason. Moving between rows or columns keeps the
    /// position on the other axes where the new place is long enough, which is what makes a grid
    /// feel like a grid rather than a reset.
    /// Move down (`1`) or up (`-1`) on whichever vertical axis has depth here: the cells of the
    /// current column when it holds more than one, the rows otherwise.
    pub fn step_vertical(&mut self, d: isize) {
        let cells = self
            .rows
            .get(self.cursor.0)
            .and_then(|r| r.get(self.cursor.1))
            .map_or(0, Vec::len);
        // **At the end of a column it falls through to the next row.** Walking the cells of a split
        // column and walking the rows are the same gesture — "further down" — so stopping dead at
        // the last pane of a column makes the key do nothing when there is obviously somewhere to
        // go. A column of one has no cells to walk, so it moves by rows immediately.
        let at_end = match d < 0 {
            true => self.cursor.2 == 0,
            false => self.cursor.2 + 1 >= cells,
        };
        match cells > 1 && !at_end {
            true => self.step(0, d, 0),
            false => {
                let before = self.cursor;
                self.step(0, 0, d);
                // The row did not move (already the first or last), so stay where we are rather
                // than silently landing on a different cell of the same column.
                if self.cursor.0 == before.0 {
                    self.cursor = before;
                    self.sync();
                    return;
                }
                // **Enter the new column at the edge you arrived from**: coming down, its first
                // pane; coming up, its last. Keeping the old index instead drops you into the
                // middle of a split column and reads as a jump rather than a step.
                let landed = self
                    .rows
                    .get(self.cursor.0)
                    .and_then(|r| r.get(self.cursor.1))
                    .map_or(0, Vec::len);
                self.cursor.2 = match d < 0 {
                    true => landed.saturating_sub(1),
                    false => 0,
                };
                self.sync();
            }
        }
    }

    /// Move the cursor onto whichever card reports itself hovered, if any.
    ///
    /// Runs on pointer moves. A move that leaves every card unhovered changes nothing — the cursor
    /// stays where it was rather than snapping back to a corner, because "the mouse is over
    /// nothing" is not a choice the user made.
    fn follow_hover(&mut self) {
        let at = self.rows.iter().enumerate().find_map(|(r, row)| {
            row.iter().enumerate().find_map(|(c, col)| {
                col.iter()
                    .position(|(_, _, hov)| hov.is_some_and(|h| h.get_untracked()))
                    .map(|i| (r, c, i))
            })
        });
        if let Some(at) = at.filter(|at| *at != self.cursor) {
            self.cursor = at;
            self.sync();
        }
    }

    pub fn step(&mut self, dcol: isize, dcell: isize, drow: isize) {
        if self.rows.is_empty() {
            return;
        }
        let (mut r, mut c, mut i) = self.cursor;
        // Leaving this row: remember where in it we were, so coming back lands there.
        if let Some(mark) = self.row_marks.get_mut(r) {
            *mark = (c, i);
        }
        r = ((r as isize + drow).clamp(0, self.rows.len() as isize - 1)) as usize;
        // Arriving in a different row: resume from its own remembered place rather than carrying
        // this row's column index across and clamping it to something that means nothing there.
        if r != self.cursor.0
            && let Some(&(mc, mi)) = self.row_marks.get(r)
        {
            c = mc;
            i = mi;
        }
        let cols = self.rows[r].len();
        if cols == 0 {
            // A row with no columns is still a row you can stand on — reachable, not skipped.
            self.cursor = (r, 0, 0);
            self.sync();
            return;
        }
        c = ((c.min(cols - 1) as isize + dcol).clamp(0, cols as isize - 1)) as usize;
        let cells = self.rows[r][c].len();
        if cells == 0 {
            self.cursor = (r, c, 0);
            self.sync();
            return;
        }
        i = ((i.min(cells - 1) as isize + dcell).clamp(0, cells as isize - 1)) as usize;
        self.cursor = (r, c, i);
        self.sync();
    }
}

// A grid is a container: the caller sets its gap, padding and sizing like any other, rather than
// the widget baking in spacing it cannot know the right value for.
impl crate::builders::LayoutExt for CardGrid {}


impl Default for CardGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CardGrid {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        // **Pointing at a card moves the cursor onto it**, so the mouse and the keyboard share one
        // position instead of each having their own. Read from the cards' own hover signals — they
        // already hit-test themselves, so this needs no geometry of its own and cannot disagree
        // with what the card is drawing. Not consumed: the card still gets its own hover.
        if matches!(ev, Event::PointerMove(_) | Event::PointerEnter(_)) {
            self.follow_hover();
            return Handled::No;
        }
        let Event::Widget(intent) = ev else {
            return Handled::No;
        };
        match intent {
            // Three axes on the shared vocabulary, no keys of this widget's own:
            //   item_previous/next  → COLUMN (Ctrl+h / Ctrl+l)
            //   menu_up/down        → the cell within that column (Ctrl+k / Ctrl+j)
            //   menu_history_up/down → ROW (Ctrl+p / Ctrl+n)
            //
            // The history pair is deliberately reused rather than given new keys: it means "the
            // outer axis" on a surface that has one, and a search surface's past queries on one
            // that does not. Two widgets, one pair of keys, never both on screen.
            WidgetIntent::ItemPrevious => self.step(-1, 0, 0),
            WidgetIntent::ItemNext => self.step(1, 0, 0),
            // **Vertical goes as deep as there is depth.** In a column holding more than one cell
            // it walks the cells; in a column holding one it moves to the next row instead, so the
            // key always does the useful thing rather than nothing. The outer axis stays reachable
            // unconditionally on `menu_history_*`.
            WidgetIntent::MenuUp => self.step_vertical(-1),
            WidgetIntent::MenuDown => self.step_vertical(1),
            WidgetIntent::MenuHistoryUp => self.step(0, 0, -1),
            WidgetIntent::MenuHistoryDown => self.step(0, 0, 1),
            WidgetIntent::Activate => {
                // Read the key first: the callback may tear the surface down.
                let key = self.selected_key().map(str::to_string);
                if let (Some(f), Some(key)) = (&self.on_activate, key) {
                    f(&key);
                }
            }
            WidgetIntent::Dismiss => {
                if let Some(f) = &self.on_dismiss {
                    f();
                }
            }
            _ => return Handled::No,
        }
        Handled::Yes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactive::signal;

    /// Two rows: the first a split column of two panes, the second a single pane.
    fn grid() -> CardGrid {
        let cell = |k: &str| GridCell::new(k, signal(false));
        CardGrid::new()
            .row(vec![vec![cell("a"), cell("b")]], crate::widgets::Flex::row())
            .row(vec![vec![cell("c")]], crate::widgets::Flex::row())
    }

    /// **The vertical axis is one gesture.** Walking the panes of a split column and walking the
    /// workspaces are both "further down", so stopping dead at the last pane of a column makes the
    /// key do nothing when there is plainly somewhere to go.
    #[test]
    fn moving_down_past_the_last_pane_of_a_column_reaches_the_next_row() {
        let mut g = grid().selected("a");
        g.step_vertical(1);
        assert_eq!(g.selected_key(), Some("b"), "within the split column first");
        g.step_vertical(1);
        assert_eq!(g.selected_key(), Some("c"), "then through to the next row");
    }

    /// And back up the same way.
    #[test]
    fn moving_up_from_a_row_reaches_the_column_above() {
        let mut g = grid().selected("c");
        g.step_vertical(-1);
        assert_eq!(g.selected_key(), Some("b"), "into the row above");
        g.step_vertical(-1);
        assert_eq!(g.selected_key(), Some("a"));
    }

    /// **A row remembers where you were in it.** Without that the cursor carries its column index
    /// across and clamps, so stepping out of a row and back lands on the first card every time —
    /// the position is destroyed by the trip, not by anything the user did.
    #[test]
    fn leaving_a_row_and_coming_back_returns_to_where_you_were() {
        let cell = |k: &str| GridCell::new(k, signal(false));
        let mut g = CardGrid::new()
            .row(
                vec![vec![cell("a")], vec![cell("b")], vec![cell("c")]],
                crate::widgets::Flex::row(),
            )
            .row(vec![vec![cell("z")]], crate::widgets::Flex::row())
            .selected("c");

        g.step(0, 0, 1); // down to the second row (one column)
        assert_eq!(g.selected_key(), Some("z"));
        g.step(0, 0, -1); // back up
        assert_eq!(g.selected_key(), Some("c"), "the third column, where we left it");
    }

    /// At the very end there is nowhere to fall through to, and the cursor stays put rather than
    /// jumping to some other cell of the same column.
    #[test]
    fn the_ends_hold() {
        let mut g = grid().selected("c");
        g.step_vertical(1);
        assert_eq!(g.selected_key(), Some("c"));
        let mut g = grid().selected("a");
        g.step_vertical(-1);
        assert_eq!(g.selected_key(), Some("a"));
    }
}
