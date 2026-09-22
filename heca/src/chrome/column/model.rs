//! One column, reduced to the plain data its shell renders from.
//!
//! Nothing here knows about `AppState`, a window or a GPU: [`super`] gathers these from the session
//! and hands them down, which is what lets every component below be tested headless (AGENTS.md
//! § 0b-bis rule 4).

use heca_core::layout::{ColumnId, PaneId};

/// Everything the column shell needs, for one column.
///
/// The rect is **given**, never computed — `heca-core`'s scrolling engine places columns and this
/// only reflects the result. A component that computed its own geometry would be taking over the
/// layout engine's job (AGENTS.md § 0b).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ColumnShellModel {
    /// The column's own identity — stable across a split, which is what a key is built from
    /// (F003/P082/T458).
    pub(crate) col_id: ColumnId,
    /// Absolute logical-pixel rect the scrolling engine placed this column at.
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    /// **Which pane a pick on this column goes to** — its active one.
    ///
    /// Picking a column means "take me there", and where a column *is* is the pane you would land
    /// on. It reuses `focus_pane` rather than adding a focus-column action: the column has no
    /// keyboard of its own, so a new verb would resolve to this one anyway.
    pub(crate) focus_pane: Option<PaneId>,
    /// **The panes in this column**, in the order the layout engine placed them, each carrying the
    /// absolute rect it was given.
    ///
    /// A column is where a pane lives, so the panes are its children — which is what lets one move
    /// between containers the way a div does. Their rects stay **given**: the WM owns pane
    /// geometry, and a column that stacked them would lose the space the WM left between them.
    pub(crate) panes: Vec<crate::chrome::pane::PaneShellModel>,
}

impl ColumnShellModel {
    /// **What the built tree depends on**, so the retained tree is rebuilt only when it changes.
    ///
    /// The rect is deliberately absent: a moved or resized column is repositioned every frame,
    /// which is far cheaper than rebuilding, and rebuilding on every pixel of a drag would throw
    /// away the widget signals mid-gesture. Same split the pane shell already makes.
    pub(crate) fn key(&self) -> String {
        format!("{}|{:?}", self.col_id.0, self.focus_pane.map(|p| p.0))
    }

    /// **Who the column's children are**, in order — the identity each pane answers to.
    ///
    /// This is what the children are reconciled against, so a pane that is still here keeps the
    /// widget it had: its letter, its gesture in flight, its animation. Deliberately not part of
    /// [`key`](Self::key): adding a pane must not throw away the other panes' trees.
    pub(crate) fn pane_keys(&self) -> Vec<String> {
        self.panes
            .iter()
            .map(|p| crate::chrome::pane_key(p.pane_id))
            .collect()
    }
}
