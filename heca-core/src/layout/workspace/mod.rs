use super::scrolling::ScrollingSpace;
use super::types::*;

/// Which layout domain has keyboard focus.
///
/// When `Floating`, only pane-local actions (close, rename) operate on the
/// active floating pane. Tiled-layout mutations (resize, zoom, swap, move,
/// column navigation) are no-op. Navigation between floating panes is
/// deferred to a future phase.
///
/// The floating domain is **modal** — mouse clicks, keyboard shortcuts, and
/// sidebar selection cannot switch to a tiled pane while a floating pane is
/// focused. Use `prefix+f` (unfloat), closing the floating pane, or
/// `prefix+i` (toggle) to return to `Tiled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusDomain {
    #[default]
    Tiled,
    Floating,
}

/// A workspace contains a scrolling layout and optionally floating panes.
///
/// This is heca's equivalent of NIRI's `Workspace<W>`, which contains
/// both a `ScrollingSpace` (tiling) and a `FloatingSpace` (floating windows).
#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: Option<String>,
    /// The scrollable-tiling layout.
    pub scrolling: ScrollingSpace,
    /// Floating panes (future feature).
    pub floating_panes: Vec<FloatingPane>,
    /// Which layout domain has keyboard focus.
    pub focus_domain: FocusDomain,
}

/// **Where a floating pane came from**, so unfloating can put it back.
///
/// The column is remembered by **identity**, never by number: a number points at whatever column
/// sits there now, and the column a pane floated out of vanishes when that pane was its last one,
/// which slides the next column into its place. `position` is only for that case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatOrigin {
    /// The column it floated out of.
    pub column: ColumnId,
    /// Where that column sat, used to make a new column in the same place if it is gone.
    pub position: usize,
    /// The pane's row within the column.
    pub row: usize,
}

/// A floating pane with position and size.
#[derive(Debug, Clone)]
pub struct FloatingPane {
    pub pane: super::column::Pane,
    pub position: Point,
    pub size: Size,
    pub is_active: bool,
    /// Where it floated out of; `None` for a pane that never tiled (one spawned floating).
    pub origin: Option<FloatOrigin>,
}

impl Workspace {
    pub fn new(id: WorkspaceId, options: LayoutOptions) -> Self {
        let scrolling = ScrollingSpace::new(options);
        Self {
            id,
            name: None,
            scrolling,
            floating_panes: Vec::new(),
            focus_domain: FocusDomain::default(),
        }
    }

    pub fn has_panes(&self) -> bool {
        !self.scrolling.is_empty() || !self.floating_panes.is_empty()
    }

    /// Clear active state from all floating panes.
    pub fn deactivate_floating_panes(&mut self) {
        for float in &mut self.floating_panes {
            float.is_active = false;
        }
    }

    /// Activate exactly one floating pane by id.
    pub fn activate_floating_pane(&mut self, pane_id: PaneId) -> bool {
        let mut found = false;
        for float in &mut self.floating_panes {
            let is_target = float.pane.id == pane_id;
            float.is_active = is_target;
            found |= is_target;
        }
        self.focus_domain = if found {
            FocusDomain::Floating
        } else {
            FocusDomain::Tiled
        };
        found
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

pub use handle::{Detached, WorkspaceMut, WorkspaceRef};

#[cfg(test)]
mod tests;
