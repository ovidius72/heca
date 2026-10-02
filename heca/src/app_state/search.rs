//! A scrollback search, and which pane the keyboard's search acts on.

use super::*;

/// Active scrollback search: the query, its matches across the searched pane's
/// scrollback, and the currently-focused match. Lives on [`AppState`] so `n`/`N`
/// navigation works after the query overlay closes back into selection mode.
pub struct SearchState {
    /// The query field — a **real [`Input`]**, so the whole editing model comes for
    /// free and behaves exactly as every other text field in the app: selection,
    /// caret motion, word/line delete (`Ctrl+u`, `Ctrl+w`, `Alt+Backspace`),
    /// select-all, click-to-place-caret.
    ///
    /// It used to be a bare `String` that a hand-written key handler pushed
    /// characters onto — it understood Backspace and nothing else, so every editing
    /// shortcut silently did nothing here while working everywhere else.
    ///
    /// Driven manually (bounds + font set at paint) rather than living in the focus
    /// tree, because the bar is drawn as an overlay on the chrome scene. This is the
    /// same arrangement [`CommandPalette`]'s query line uses.
    pub input: std::cell::RefCell<heca_grid_ui::widgets::Input>,
    /// All matches, ascending by stable row / column.
    pub matches: Vec<heca_core::backend::SearchMatch>,
    /// Index into `matches` of the focused match, if any.
    pub current: Option<usize>,
}


/// A single follow-link candidate: the letter to press, the pane it lives in, where
/// to stamp its keycap (the link's first visible cell — `row` from the viewport top,
/// `start_col` inclusive), and the URL to open. Built from `snapshot.hyperlinks`, so
/// OSC 8 and auto-detected (linkify) links are followed identically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkHint {
    pub label: char,
    pub pane_id: PaneId,
    pub row: usize,
    pub start_col: usize,
    pub url: String,
}


impl AppState {
    /// The scrollback search for `pane`, if it has one.
    pub fn search_for(&self, pane: PaneId) -> Option<&SearchState> {
        self.searches.get(&pane)
    }

    /// The pane the keyboard's search acts on: the **selection's** pane if copy-mode
    /// owns one, else the focused pane.
    ///
    /// One definition, used by every search entry point — starting a search, editing
    /// the query, stepping matches, cancelling. They must agree: a search started for
    /// one pane while edits were applied to another would leave the query frozen,
    /// because the keystrokes would land on an entry that does not exist. The old
    /// single-search state avoided this by carrying its own `pane_id`; with per-pane
    /// storage the resolution itself has to be shared.
    pub fn search_target_pane(&self) -> Option<PaneId> {
        match self.selection.owner() {
            Some(crate::app::selection_model::SelectionOwner::Pane(id)) => Some(id),
            _ => self.focused_pane,
        }
    }

    /// The search the keyboard is driving — see [`search_target_pane`](Self::search_target_pane).
    pub fn active_search_mut(&mut self) -> Option<&mut SearchState> {
        let pane = self.search_target_pane()?;
        self.searches.get_mut(&pane)
    }

    /// Drop `pane`'s search, if any. Called when a pane closes so a dead pane cannot
    /// leave a search behind that nothing can reach or clear.
    pub fn clear_search(&mut self, pane: PaneId) {
        self.searches.remove(&pane);
    }
}
