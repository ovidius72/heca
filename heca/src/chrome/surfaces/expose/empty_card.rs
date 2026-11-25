//! **An empty workspace's card** — what the map shows for a workspace with no panes, so the cursor
//! has somewhere to stop and the workspace can still be chosen.
//!
//! A workspace with nothing in it is still a row of the map (`model`): without a card its row was
//! a gap the cursor could walk into and find nothing to light, after which the map had no
//! selection at all. **A workspace is always selectable, in the map too.** The card is an empty
//! screen: the same look as a pane's card, no words (the map labels nothing — see the picture
//! tests), and choosing it — a click, Enter, its letter — takes you to that workspace.

use heca_grid_ui::Component;
use heca_grid_ui::builders::{EventExt, HintExt, IdentityExt, LayoutExt};
use heca_grid_ui::widgets::{GridCell, HintPlacement, Row, RowVariant};

use super::pane_card::ExposeCallbacks;

/// The cursor key of an empty workspace's card — `ws:<index>`, apart from every pane's number.
pub(crate) fn workspace_key(ws_idx: usize) -> String {
    format!("ws:{ws_idx}")
}

/// The workspace a cursor key names, when it names one ([`workspace_key`]).
pub(crate) fn workspace_of(key: &str) -> Option<usize> {
    key.strip_prefix("ws:")?.parse().ok()
}

/// One empty workspace in the map. Properties in, a card and its cursor cell out.
pub(crate) struct EmptyCard<'a> {
    /// Which workspace, by its place in the session.
    pub(crate) ws_idx: usize,
    pub(crate) cb: &'a ExposeCallbacks,
}

impl EmptyCard<'_> {
    /// Build the card and the cell the grid navigates it by — the same shape as a pane's card, so
    /// the row sizes it the same way.
    pub(crate) fn build(self) -> (Row, GridCell) {
        let card = Row::new();
        let cell =
            GridCell::new(workspace_key(self.ws_idx), card.nav_state()).hovered(card.hovered());
        let mut card = card
            .variant(RowVariant::Filled)
            .justify("center")
            .key(workspace_key(self.ws_idx));
        // The card the cursor is on holds the keyboard, as a pane's does, so the nav keys it does
        // not take bubble up to the grid.
        let nav = card.nav_state();
        card.base_mut().follow_focus(nav);
        let choose = self.cb.choose_workspace.clone();
        let ws_idx = self.ws_idx;
        let card = card
            .on_click({
                let choose = choose.clone();
                move |_| choose(ws_idx)
            })
            .on_hint(move || choose(ws_idx))
            .hint_placement(HintPlacement::TopLeft);
        (card, cell)
    }
}
