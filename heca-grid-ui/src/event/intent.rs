//! **What a key binding asks a widget to do** — the shared vocabulary every interactive widget
//! speaks instead of hardcoding keys.

/// A **semantic widget intent** — one shared vocabulary every interactive widget speaks
/// instead of hardcoding keys (the host maps `[keys.widgets]` → these via a
/// [`Keymap`](crate::keymap::Keymap)). Split by **axis**: horizontal (`Item*`), vertical
/// (`Menu*`), the shared `Activate`/`Dismiss`, and text-field edits (`Edit*`).
///
/// A single key may resolve to **several** intents (e.g. `Ctrl+h` → `EditDeleteBack`
/// *then* `ItemPrevious`); the host delivers them in order to the focused widget, which
/// consumes the one it understands (an `Input` deletes, a `Tabs`/`Dialog` moves) — so the
/// overload disambiguates by focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetIntent {
    /// Horizontal previous (left) — `Tabs`, a `Dialog`'s button row. `item_previous`.
    ItemPrevious,
    /// Horizontal next (right) — `Tabs`, a `Dialog`'s button row. `item_next`.
    ItemNext,
    /// Vertical up — menus, `Select` lists, the command palette. `menu_up`.
    MenuUp,
    /// Vertical down — menus, `Select` lists, the command palette. `menu_down`.
    MenuDown,
    /// An **older** past query — a search surface's history, walked backwards. `menu_history_up`.
    ///
    /// Vocabulary, not a palette feature: any surface that searches has a history worth recalling.
    /// Up/down rather than previous/next because the table splits by axis (`item_*` is horizontal,
    /// `menu_*` vertical) and because beside `item_previous`, a "history_prev" reads ambiguously —
    /// an older entry, or one row up?
    MenuHistoryUp,
    /// A **newer** past query, and past the newest, the draft the walk interrupted.
    /// `menu_history_down`.
    MenuHistoryDown,
    /// **Open this terminal's search** — whichever terminal holds the keyboard answers. `find`.
    ///
    /// Vocabulary for anything that has a body of text to look through: it names the capability
    /// and leaves the key to the user (`[keys.widgets]`; unbound by default).
    Find,
    /// **The next match** of the search that is open. `find_next`.
    FindNext,
    /// **The previous match** of the search that is open. `find_previous`.
    FindPrevious,
    /// **Make the content bigger** — whichever widget holds the keyboard and can scale what it
    /// shows answers (a terminal steps its font up). One step; how big a step is, is the answer's.
    ZoomIn,
    /// **Make the content smaller** — the counterpart of [`ZoomIn`](Self::ZoomIn).
    ZoomOut,
    /// **Back to the content's own size** — undo every [`ZoomIn`](Self::ZoomIn) /
    /// [`ZoomOut`](Self::ZoomOut).
    ZoomReset,

    /// Activate / commit / submit the current entry or primary action. `activate`.
    Activate,
    /// Dismiss / cancel / close the overlay. `dismiss`.
    Dismiss,
    /// [`Input`](crate::widgets::Input): delete one char before the caret. `edit_delete_back`.
    EditDeleteBack,
    /// [`Input`](crate::widgets::Input): delete from the caret to line start. `edit_delete_to_line_start`.
    EditDeleteToLineStart,
    /// [`Input`](crate::widgets::Input): select the whole field. `edit_select_all`.
    EditSelectAll,
    /// Submit the focused text field through its own handler. Unbound by default; `edit_submit`.
    EditSubmit,

    // ── Scrolling a scroll area from the keyboard ──
    //
    // These are **semantic**, not keys: the host says "one page back", the widget decides what a
    // page is and clamps the result, because it is the only thing that knows its viewport and its
    // content. Delivering `PageUp` as a key instead would have needed `GridKey` to grow variants
    // and would have put paging arithmetic in the app.
    //
    // Both axes, because a region can be horizontal or two-axis. A region that cannot scroll the
    // axis asked for declines, so a nested one still gets its turn — the same rule the wheel
    // follows.
    /// Scroll a scroll area one page back, vertically.
    ScrollPageUp,
    /// Scroll a scroll area one page on, vertically.
    ScrollPageDown,
    /// Jump a scroll area to the top.
    ScrollToTop,
    /// Jump a scroll area to the bottom.
    ScrollToBottom,
    /// Scroll a scroll area one page back, horizontally.
    ScrollPageLeft,
    /// Scroll a scroll area one page on, horizontally.
    ScrollPageRight,
    /// Jump a scroll area to its left edge.
    ScrollToLeftEdge,
    /// Jump a scroll area to its right edge.
    ScrollToRightEdge,
}
