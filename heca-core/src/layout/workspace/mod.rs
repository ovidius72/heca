use super::scrolling::ScrollingSpace;
use super::types::*;

/// A workspace contains a scrolling layout and optionally floating panes.
///
/// This is heca's equivalent of NIRI's `Workspace<W>`, which contains
/// both a `ScrollingSpace` (tiling) and a `FloatingSpace` (floating windows).
///
/// It is **shared content** (F012, decision 463ccd07): which panes float is the same in every
/// window. Where each window draws a float, which float it has active and whether it is focused on
/// the tiling or the floats are per window and live in its
/// [`ScrollView`](super::window_view::ScrollView).
#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: Option<String>,
    /// **Its own number**, given when it was made and kept wherever it moves: the `N` of the
    /// "Workspace N" it is called until it is named. Never reused within a session — the next
    /// workspace takes the next number even after one is removed. `0` is "not numbered yet".
    pub number: u32,
    /// The scrollable-tiling layout.
    pub scrolling: ScrollingSpace,
    /// Floating panes (future feature).
    pub floating_panes: Vec<FloatingPane>,
}

/// **Where a floating pane came from**, so unfloating can put it back.
///
/// The column is remembered by **identity**, never by number: a number points at whatever column
/// sits there now, and the column a pane floated out of vanishes when that pane was its last one,
/// which slides the next column into its place. `position` is only for that case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FloatOrigin {
    /// The column it floated out of.
    pub column: ColumnId,
    /// Where that column sat, used to make a new column in the same place if it is gone.
    pub position: usize,
    /// The pane's row within the column.
    pub row: usize,
}

/// A floating pane: the pane and where it floated out of. **Content only** — where a window draws
/// it, how big and whether it is the window's active float are per window and live in the window's
/// [`ScrollView`](super::window_view::ScrollView), keyed by [`PaneId`].
#[derive(Debug, Clone)]
pub struct FloatingPane {
    pub pane: super::column::Pane,
    /// Where it floated out of; `None` for a pane that never tiled (one spawned floating).
    pub origin: Option<FloatOrigin>,
}

impl Workspace {
    pub fn new(id: WorkspaceId, options: LayoutOptions) -> Self {
        let scrolling = ScrollingSpace::new(options);
        Self {
            id,
            name: None,
            number: 0,
            scrolling,
            floating_panes: Vec::new(),
        }
    }

    /// **What it is called**: the name it was given, or "Workspace" and its own number. The one
    /// rule every surface shows — the dock, the exposé, the palette, the rename prompt — so a
    /// workspace keeps its name wherever it moves in the list.
    pub fn label(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("Workspace {}", self.number))
    }

    pub fn has_panes(&self) -> bool {
        !self.scrolling.is_empty() || !self.floating_panes.is_empty()
    }

    /// **Every pane in this workspace** — the columns' panes in order, then the floating ones. A
    /// caller that lists, syncs or searches panes walks this, so none can forget the floats.
    pub fn panes(&self) -> impl Iterator<Item = &super::column::Pane> {
        self.scrolling
            .columns
            .iter()
            .flat_map(|col| col.panes.iter())
            .chain(self.floating_panes.iter().map(|float| &float.pane))
    }

    /// [`panes`](Self::panes), for writing.
    pub fn panes_mut(&mut self) -> impl Iterator<Item = &mut super::column::Pane> {
        self.scrolling
            .columns
            .iter_mut()
            .flat_map(|col| col.panes.iter_mut())
            .chain(self.floating_panes.iter_mut().map(|float| &mut float.pane))
    }

    /// Find any pane by ID across both scrolling and floating.
    pub fn find_pane(&self, pane_id: PaneId) -> Option<&super::column::Pane> {
        self.panes().find(|pane| pane.id == pane_id)
    }

    pub fn find_pane_mut(&mut self, pane_id: PaneId) -> Option<&mut super::column::Pane> {
        self.panes_mut().find(|pane| pane.id == pane_id)
    }
}

mod handle;

pub use handle::{Detached, FloatPlacement, WorkspaceMut, WorkspaceRef};

#[cfg(test)]
mod tests;
