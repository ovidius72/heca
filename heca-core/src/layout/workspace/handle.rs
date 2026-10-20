//! A workspace **as one window sees it** — the part of the workspace that cannot answer without
//! knowing where that window is looking.
//!
//! The workspace itself is shared content. Which column has the focus, how far the strip is
//! scrolled and the box it is laid out in are the window's ([`ScrollView`]), so anything that needs
//! them goes through [`WorkspaceRef`] (reads) or [`WorkspaceMut`] (moves). Both reach everything
//! the workspace itself has, so `ws.floating_panes` and `ws.name` read the same through either.

use std::ops::{Deref, DerefMut};

use super::Workspace;
use crate::layout::column::Pane;
use crate::layout::scrolling::{ScrollingMut, ScrollingRef};
use crate::layout::types::{ColumnId, PaneId, Rectangle};
use crate::layout::window_view::{FocusDomain, ScrollView};

/// A floating pane as one window sees it: the shared pane, where this window draws it, and whether
/// it is this window's active float. The geometry is the window's own (per F012, decision
/// 463ccd07) — a render or exposé pass asks for it this way so it never reads a window's rect off
/// the shared content.
#[derive(Clone, Copy)]
pub struct FloatPlacement<'a> {
    pub pane: &'a Pane,
    pub rect: Rectangle,
    pub active: bool,
}

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
        if self.view.focus_domain == FocusDomain::Floating {
            let id = self.view.active_float?;
            self.ws
                .floating_panes
                .iter()
                .find(|f| f.pane.id == id)
                .map(|f| &f.pane)
        } else {
            self.scroll().active_pane()
        }
    }

    /// Whether this window is focused on the tiling or on a floating pane in this workspace.
    pub fn focus_domain(&self) -> FocusDomain {
        self.view.focus_domain
    }

    /// The floating pane this window has active, if any.
    pub fn active_float(&self) -> Option<PaneId> {
        self.view.active_float
    }

    /// Every floating pane with where this window draws it and whether it is this window's active
    /// float. A float this window has not placed falls back to the centred default, so a render or
    /// exposé pass never has to reach for geometry the shared content does not carry.
    pub fn floating(&self) -> Vec<FloatPlacement<'a>> {
        self.ws
            .floating_panes
            .iter()
            .map(|f| FloatPlacement {
                pane: &f.pane,
                rect: self
                    .view
                    .float_rect(f.pane.id)
                    .unwrap_or_else(|| self.ws.float_rect_in(self.view.area)),
                active: self.view.active_float == Some(f.pane.id),
            })
            .collect()
    }

    /// Where a pane floats when nothing says where: centred, [`float_size`] of the working area.
    ///
    /// [`float_size`]: crate::layout::types::LayoutOptions::float_size
    pub fn default_float_rect(&self) -> Rectangle {
        self.ws.float_rect_in(self.view.area)
    }

    /// A copy of this workspace, as this window sees it, with `pane_id` already taken out — what
    /// a caller counts indices on when the pane is about to move: placing it next to a card in the
    /// column it came from would otherwise land one row off. Nothing here is changed.
    pub fn without_pane(&self, pane_id: PaneId) -> Detached {
        let mut ws = self.ws.clone();
        let mut view = self.view.clone();
        let before = ws.scrolling.through(&view).positions();
        if let Some((_, Some(effect))) = ws.take_pane(pane_id) {
            view.react_to(&ws.scrolling, &[effect], &before);
        }
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
        // Scale this window's floating panes proportionally so they keep their relative position
        // + coverage when the working area changes (window resize, chrome toggle). Without this, a
        // float spawned at 95% keeps its absolute pixel size while the window grows/shrinks around
        // it — drifting off-screen or looking stranded. The geometry is this window's own (per
        // window, decision 463ccd07), so it is the window's view that is rescaled; a float this
        // window never placed has no stored rect and tracks the area through the centred default.
        // Position is stored relative to the working-area origin, so a pure scale by the size ratio
        // is correct (plus an origin shift in case `loc` ever moves).
        let sx = working_area.size.w / old.size.w.max(1.0);
        let sy = working_area.size.h / old.size.h.max(1.0);
        if (sx - 1.0).abs() > 1e-6 || (sy - 1.0).abs() > 1e-6 {
            for rect in self.view.floats.values_mut() {
                rect.loc.x = working_area.loc.x + (rect.loc.x - old.loc.x) * sx;
                rect.loc.y = working_area.loc.y + (rect.loc.y - old.loc.y) * sy;
                rect.size.w *= sx;
                rect.size.h *= sy;
            }
        }
    }

    /// Whether this window is focused on the tiling or on a floating pane — see
    /// [`WorkspaceRef::focus_domain`].
    pub fn focus_domain(&self) -> FocusDomain {
        self.view.focus_domain
    }

    /// Focus the tiling or a floating pane in this workspace for this window.
    pub fn set_focus_domain(&mut self, domain: FocusDomain) {
        self.view.focus_domain = domain;
    }

    /// This window gives up its active floating pane; it keeps looking where it looks.
    pub fn deactivate_floating_panes(&mut self) {
        self.view.active_float = None;
    }

    /// Make `pane` this window's active floating pane and focus the floats, when the workspace has
    /// it floating; otherwise focus the tiling. `true` when it is floating here.
    pub fn activate_floating_pane(&mut self, pane: PaneId) -> bool {
        let found = self.ws.floating_panes.iter().any(|f| f.pane.id == pane);
        self.view.active_float = found.then_some(pane);
        self.view.focus_domain = if found {
            FocusDomain::Floating
        } else {
            FocusDomain::Tiled
        };
        found
    }

    /// This window floats `pane` at `rect` and focuses it — the window's reaction to a `Floated`
    /// fact. The pane is already floating content; this sets only the window's own view of it.
    pub fn float_in_view(&mut self, pane: PaneId, rect: Rectangle) {
        self.view.set_float_rect(pane, rect);
        self.view.active_float = Some(pane);
        self.view.focus_domain = FocusDomain::Floating;
    }

    /// Reconcile this window's floating view with the content after a float appeared or went:
    /// forget the geometry of floats the workspace no longer has, drop the active float if it is
    /// gone, and return to the tiling only when no float is left — the window stays in the floating
    /// domain while any float remains, matching the pre-split behaviour.
    pub fn reconcile_floats(&mut self) {
        let present: Vec<PaneId> = self.ws.floating_panes.iter().map(|f| f.pane.id).collect();
        self.view.retain_floats(|id| present.contains(&id));
        if present.is_empty() {
            self.view.focus_domain = FocusDomain::Tiled;
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

    /// Make `pane_id` the active pane of this workspace — tiled (its column active, scrolled into
    /// view) or floating. `false` when it is not here.
    pub fn activate_pane(&mut self, pane_id: PaneId) -> bool {
        if let Some((col, row)) = self.scrolling.pane_indices(pane_id) {
            self.view.active_float = None;
            self.view.focus_domain = FocusDomain::Tiled;
            let mut scroll = self.scroll_mut();
            scroll.activate_column(col);
            scroll.focus_pane_in(col, row);
            return true;
        }
        self.activate_floating_pane(pane_id)
    }

    /// Focus left in the scrolling layout.
    pub fn focus_left(&mut self) -> bool {
        if self.view.focus_domain == FocusDomain::Floating {
            false // TODO: floating focus
        } else {
            self.scroll_mut().focus_left()
        }
    }

    /// Focus right in the scrolling layout.
    pub fn focus_right(&mut self) -> bool {
        if self.view.focus_domain == FocusDomain::Floating {
            false // TODO: floating focus
        } else {
            self.scroll_mut().focus_right()
        }
    }

    /// Focus up (previous pane in column, or previous workspace).
    pub fn focus_up(&mut self) -> bool {
        if self.view.focus_domain == FocusDomain::Floating {
            return false; // TODO
        }
        let mut scroll = self.scroll_mut();
        if scroll.focus_up() {
            return true;
        }
        // Wrap to the previous column's last pane.
        let col_idx = scroll.active_column_idx();
        if col_idx == 0 {
            return false;
        }
        let prev = col_idx - 1;
        scroll.activate_column(prev);
        let last = scroll.columns.get(prev).map(|c| c.panes.len().saturating_sub(1));
        if let Some(last) = last {
            scroll.focus_pane_in(prev, last);
        }
        true
    }

    /// Focus down (next pane in column, or next workspace).
    pub fn focus_down(&mut self) -> bool {
        if self.view.focus_domain == FocusDomain::Floating {
            return false; // TODO
        }
        let mut scroll = self.scroll_mut();
        if scroll.focus_down() {
            return true;
        }
        // Wrap to the next column's first pane.
        let col_idx = scroll.active_column_idx();
        if col_idx + 1 >= scroll.columns.len() {
            return false;
        }
        scroll.activate_column(col_idx + 1);
        scroll.focus_pane_in(col_idx + 1, 0);
        true
    }
}
