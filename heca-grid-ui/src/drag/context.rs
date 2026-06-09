//! Drag context — top-level coordinator for multi-surface drags.
//!
//! [`DragContext`] owns a map of per-surface [`SurfaceDragState`]s and tracks
//! which surface is actively dragging. Since there is only one mouse, only
//! one surface can be active at a time.
//!
//! All surfaces that are NOT active still update their `hover_item` during
//! a drag — so both left and right sidebars can show drop targets
//! simultaneously while the cursor moves.

use std::collections::HashMap;

use crate::drag::{DragSurfaceId, SurfaceDragState};

/// Top-level drag coordinator. Routes events to the active surface.
///
/// Owns no layout state — just tracks *which* surface is dragging
/// and per-surface hover/source/ghost state.
#[derive(Clone, Debug)]
pub struct DragContext {
    /// Which surface is currently being dragged from, if any.
    pub active_surface: Option<DragSurfaceId>,
    /// Per-surface drag state.
    pub surfaces: HashMap<DragSurfaceId, SurfaceDragState>,
}

impl Default for DragContext {
    fn default() -> Self {
        let mut surfaces = HashMap::new();
        // Pre-populate known surfaces so lookups never miss.
        surfaces.insert(DragSurfaceId::LeftSidebar, SurfaceDragState::default());
        Self {
            active_surface: None,
            surfaces,
        }
    }
}

impl DragContext {
    /// Create a new drag context with default state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the drag state for a specific surface.
    pub fn surface(&self, id: DragSurfaceId) -> Option<&SurfaceDragState> {
        self.surfaces.get(&id)
    }

    /// Get mutable drag state for a specific surface.
    pub fn surface_mut(&mut self, id: DragSurfaceId) -> Option<&mut SurfaceDragState> {
        self.surfaces.get_mut(&id)
    }

    /// Returns true if any surface is actively dragging.
    pub fn is_dragging(&self) -> bool {
        self.active_surface.is_some()
    }

    /// Returns the active surface's drag state, if any.
    pub fn active(&self) -> Option<&SurfaceDragState> {
        self.active_surface.and_then(|id| self.surfaces.get(&id))
    }

    /// Returns the mutable active surface's drag state, if any.
    pub fn active_mut(&mut self) -> Option<&mut SurfaceDragState> {
        if let Some(id) = self.active_surface {
            self.surfaces.get_mut(&id)
        } else {
            None
        }
    }

    /// Cancel all drag state, resetting every surface to Idle.
    pub fn cancel_all(&mut self) {
        for state in self.surfaces.values_mut() {
            state.reset();
        }
        self.active_surface = None;
    }

    /// Start a drag on the given surface. The surface's phase should already
    /// be set to `Starting` or `Dragging` by the caller.
    pub fn set_active(&mut self, id: DragSurfaceId) {
        self.active_surface = Some(id);
    }

    /// Clear the active surface (drag ended or cancelled).
    pub fn clear_active(&mut self) {
        self.active_surface = None;
    }
}