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
}

impl GridCell {
    /// A card identified by `key`, lighting `selected` when the cursor is on it.
    ///
    /// The key is the caller's own — a pane id, a path, a codepoint. `CardGrid` never interprets
    /// it; it hands it back on activation, which is the whole of what the caller needs.
    /// A cell **carries no body**: the caller draws the card inside the row layout it passes to
    /// [`CardGrid::row`], so there is one place the visuals live rather than two that can disagree.
    pub fn new(key: impl Into<String>, selected: Signal<bool>) -> Self {
        Self { key: key.into(), selected }
    }
}

/// One row's cells: their keys and the signals that light them.
type Cells = Vec<(String, Signal<bool>)>;

/// What a caller runs when a cell is chosen — it is handed the caller's own key.
type OnActivate = Box<dyn Fn(&str)>;

/// A two-axis grid of [`GridCell`]s with a cursor over them.
pub struct CardGrid {
    base: Base,
    /// Per row, the cards left to right: their keys and their light signals.
    rows: Vec<Cells>,
    /// (row, index within the row).
    cursor: (usize, usize),
    on_activate: Option<OnActivate>,
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
            cursor: (0, 0),
            on_activate: None,
            on_dismiss: None,
        }
    }

    /// Add a row of cards, laid out left to right by `layout`.
    ///
    /// The caller builds the row's own container — a `Flex::row`, a labelled strip, whatever the
    /// surface needs — because how a row *looks* is the caller's, while where the cursor *is* is
    /// this widget's. `cards` must be in the same left-to-right order the layout draws them, or the
    /// cursor and the picture disagree.
    #[heca_grid_ui_macros::host_only("a row of composed cards, not a scalar")]
    pub fn row(mut self, cards: Vec<GridCell>, layout: impl Component + 'static) -> Self {
        self.rows
            .push(cards.iter().map(|c| (c.key.clone(), c.selected)).collect());
        self.base.children.push(Box::new(layout));
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
            row.iter().position(|(k, _)| *k == key).map(|c| (r, c))
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
            .map(|(k, _)| k.as_str())
    }

    /// Light the card under the cursor and darken the rest — one signal write per card, never a
    /// rebuild.
    fn sync(&self) {
        for (r, row) in self.rows.iter().enumerate() {
            for (c, (_, sig)) in row.iter().enumerate() {
                let on = (r, c) == self.cursor;
                if sig.get_untracked() != on {
                    sig.set(on);
                }
            }
        }
    }

    /// Move by `dx` within the row and `dy` between rows, **clamped at the edges**.
    ///
    /// No wrap-around: a map you can fall off the end of is disorienting, and every list widget
    /// here stops at its ends for the same reason. Changing row keeps the column where the new row
    /// is long enough, which is what makes a grid feel like a grid.
    pub fn step(&mut self, dx: isize, dy: isize) {
        if self.rows.is_empty() {
            return;
        }
        let last_row = self.rows.len() as isize - 1;
        let r = (self.cursor.0 as isize + dy).clamp(0, last_row) as usize;
        let len = self.rows[r].len() as isize;
        if len == 0 {
            // A row with no cards is still a row you can move onto; the cursor parks there rather
            // than skipping it, so an empty row is reachable instead of invisible.
            self.cursor = (r, 0);
            self.sync();
            return;
        }
        let from = match dy {
            0 => self.cursor.1 as isize,
            _ => (self.cursor.1 as isize).min(len - 1),
        };
        self.cursor = (r, (from + dx).clamp(0, len - 1) as usize);
        self.sync();
    }
}

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
        let Event::Widget(intent) = ev else {
            return Handled::No;
        };
        match intent {
            WidgetIntent::ItemPrevious => self.step(-1, 0),
            WidgetIntent::ItemNext => self.step(1, 0),
            WidgetIntent::MenuUp => self.step(0, -1),
            WidgetIntent::MenuDown => self.step(0, 1),
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
