//! A workspace **as one window sees it** — the part of the workspace that cannot answer without
//! knowing where that window is looking.
//!
//! The workspace itself is shared content. Which column has the focus, how far the strip is
//! scrolled and the box it is laid out in are the window's ([`ScrollView`]), so anything that needs
//! them goes through [`WorkspaceRef`] (reads) or [`WorkspaceMut`] (moves). Both reach everything
//! the workspace itself has, so `ws.floating_panes` and `ws.name` read the same through either.

use std::ops::{Deref, DerefMut};

use super::{FocusDomain, Workspace};
use crate::layout::column::Pane;
use crate::layout::scrolling::{ScrollingMut, ScrollingRef};
use crate::layout::types::{ColumnId, PaneId, Rectangle};
use crate::layout::window_view::ScrollView;

/// A [`Workspace`] with the window's view of it: the reads.
#[derive(Clone, Copy)]
pub struct WorkspaceRef<'a> {
    ws: &'a Workspace,
    view: &'a ScrollView,
}

/// A [`Workspace`] with the window's view of it: the moves.
pub struct WorkspaceMut<'a> {
    ws: &'a mut Workspace,
    view: &'a mut ScrollView,
}

/// A workspace and the view of it, owned — a copy to look at, never to put back (see
/// [`WorkspaceRef::without_pane`]).
pub struct Detached {
    ws: Workspace,
    view: ScrollView,
}

impl Detached {
    /// The copy, as the window sees it.
    pub fn reader(&self) -> WorkspaceRef<'_> {
        self.ws.through(&self.view)
    }
}

impl Deref for Detached {
    type Target = Workspace;

    fn deref(&self) -> &Workspace {
        &self.ws
    }
}

impl Workspace {
    /// This workspace as the window holding `view` sees it.
    pub fn through<'a>(&'a self, view: &'a ScrollView) -> WorkspaceRef<'a> {
        WorkspaceRef { ws: self, view }
    }

    /// This workspace, moved by the window holding `view`.
    pub fn through_mut<'a>(&'a mut self, view: &'a mut ScrollView) -> WorkspaceMut<'a> {
        WorkspaceMut { ws: self, view }
    }
}

impl Deref for WorkspaceRef<'_> {
    type Target = Workspace;

    fn deref(&self) -> &Workspace {
        self.ws
    }
}

impl Deref for WorkspaceMut<'_> {
    type Target = Workspace;

    fn deref(&self) -> &Workspace {
        self.ws
    }
}

impl DerefMut for WorkspaceMut<'_> {
    fn deref_mut(&mut self) -> &mut Workspace {
        self.ws
    }
}

impl<'a> WorkspaceRef<'a> {
    /// The shared content, for a read that must outlive this handle.
    pub fn content(&self) -> &'a Workspace {
        self.ws
    }

    /// The scrolling strip, as the window sees it.
    pub fn scroll(&self) -> ScrollingRef<'a> {
        self.ws.scrolling.through(self.view)
    }

    /// The pane that has the focus: the active floating pane while floating has it, else the
    /// active pane of the active column.
    pub fn active_pane(&self) -> Option<&'a Pane> {
        if self.ws.focus_domain == FocusDomain::Floating {
            self.ws
                .floating_panes
                .iter()
                .find(|p| p.is_active)
                .map(|p| &p.pane)
        } else {
            self.scroll().active_pane()
        }
    }

    /// Where a pane floats when nothing says where: centred, [`float_size`] of the working area.
    ///
    /// [`float_size`]: crate::layout::types::LayoutOptions::float_size
    pub fn default_float_rect(&self) -> Rectangle {
        self.view
            .area
            .centred_fraction(self.ws.scrolling.options.float_size)
    }

    /// A copy of this workspace, as this window sees it, with `pane_id` already taken out — what
    /// a caller counts indices on when the pane is about to move: placing it next to a card in the
    /// column it came from would otherwise land one row off. Nothing here is changed.
    pub fn without_pane(&self, pane_id: PaneId) -> Detached {
        let mut ws = self.ws.clone();
        let mut view = self.view.clone();
        ws.through_mut(&mut view).take_pane(pane_id);
        Detached { ws, view }
    }

    /// Whether anything in this workspace is animating for the window.
    pub fn are_animations_ongoing(&self) -> bool {
        self.scroll().are_animations_ongoing()
    }
}

impl WorkspaceMut<'_> {
    /// The same workspace, for the reads that only look.
    pub fn reader(&self) -> WorkspaceRef<'_> {
        WorkspaceRef {
            ws: self.ws,
            view: self.view,
        }
    }

    /// The pane that has the focus — see [`WorkspaceRef::active_pane`].
    pub fn active_pane(&self) -> Option<&Pane> {
        self.reader().active_pane()
    }

    /// Where a pane floats when nothing says where — see [`WorkspaceRef::default_float_rect`].
    pub fn default_float_rect(&self) -> Rectangle {
        self.reader().default_float_rect()
    }

    /// The scrolling strip, as the window sees it.
    pub fn scroll(&self) -> ScrollingRef<'_> {
        self.ws.scrolling.through(self.view)
    }

    /// The scrolling strip, moved by the window.
    pub fn scroll_mut(&mut self) -> ScrollingMut<'_> {
        self.ws.scrolling.through_mut(self.view)
    }

    /// Update working area (called on resize).
    pub fn update_working_area(&mut self, working_area: Rectangle) {
        let old = self.view.area;
        self.scroll_mut().update_working_area(working_area);
        // Scale floating panes proportionally so they keep their relative position
        // + coverage when the working area changes (window resize, chrome toggle).
        // Without this, a float spawned at 95% keeps its absolute pixel size while
        // the window grows/shrinks around it — drifting off-screen or looking
        // stranded. Position is stored relative to the working-area origin (see
        // `handle_float` + the render path), so a pure scale by the size ratio is
        // correct (plus an origin shift in case `loc` ever moves).
        let sx = working_area.size.w / old.size.w.max(1.0);
        let sy = working_area.size.h / old.size.h.max(1.0);
        if (sx - 1.0).abs() > 1e-6 || (sy - 1.0).abs() > 1e-6 {
            for float in &mut self.ws.floating_panes {
                float.position.x = working_area.loc.x + (float.position.x - old.loc.x) * sx;
                float.position.y = working_area.loc.y + (float.position.y - old.loc.y) * sy;
                float.size.w *= sx;
                float.size.h *= sy;
            }
        }
    }

    /// Advance all animations in this workspace.
    pub fn advance_animations(&mut self) {
        self.scroll_mut().advance_animations();
    }

    /// Add a pane to the scrolling layout.
    ///
    /// `new_column_id` is spent only when a column is actually created (`column_idx` is `None`).
    /// It is handed in rather than derived here because a [`ColumnId`] must be **allocated**: see
    /// [`Session::next_id`](crate::layout::Session::next_id), the one counter panes and workspaces
    /// already draw from.
    pub fn add_pane(
        &mut self,
        pane: Pane,
        column_idx: Option<usize>,
        activate: bool,
        new_column_id: ColumnId,
    ) {
        let mut scroll = self.scroll_mut();
        if let Some(idx) = column_idx {
            // Add to existing column.
            scroll.add_pane_to_column(idx, None, pane, activate);
        } else {
            // Create new column.
            scroll.add_new_column(None, new_column_id, pane, activate);
        }
    }

    /// Focus left in the scrolling layout.
    pub fn focus_left(&mut self) -> bool {
        if self.ws.focus_domain == FocusDomain::Floating {
            false // TODO: floating focus
        } else {
            self.scroll_mut().focus_left()
        }
    }

    /// Focus right in the scrolling layout.
    pub fn focus_right(&mut self) -> bool {
        if self.ws.focus_domain == FocusDomain::Floating {
            false // TODO: floating focus
        } else {
            self.scroll_mut().focus_right()
        }
    }

    /// Focus up (previous pane in column, or previous workspace).
    pub fn focus_up(&mut self) -> bool {
        if self.ws.focus_domain == FocusDomain::Floating {
            return false; // TODO
        }
        let mut scroll = self.scroll_mut();
        let Some(col) = scroll.active_column_mut() else {
            return false;
        };
        if col.focus_up() {
            return true;
        }
        // Wrap to previous column's last pane.
        let col_idx = scroll.reader().active_column_idx();
        if col_idx == 0 {
            return false;
        }
        scroll.activate_column(col_idx - 1);
        if let Some(new_col) = scroll.active_column_mut() {
            let last_idx = new_col.panes.len().saturating_sub(1);
            new_col.activate_pane(last_idx);
        }
        true
    }

    /// Focus down (next pane in column, or next workspace).
    pub fn focus_down(&mut self) -> bool {
        if self.ws.focus_domain == FocusDomain::Floating {
            return false; // TODO
        }
        let mut scroll = self.scroll_mut();
        let Some(col) = scroll.active_column_mut() else {
            return false;
        };
        if col.focus_down() {
            return true;
        }
        // Wrap to next column's first pane.
        let col_idx = scroll.reader().active_column_idx();
        if col_idx + 1 >= scroll.columns.len() {
            return false;
        }
        scroll.activate_column(col_idx + 1);
        if let Some(new_col) = scroll.active_column_mut() {
            new_col.activate_pane(0);
        }
        true
    }
}
