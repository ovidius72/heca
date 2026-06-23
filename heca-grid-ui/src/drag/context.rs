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

/// Top-level drag coordinator, generic over the app payload `P`. Routes events
/// to the active surface.
///
/// Owns no layout state — just tracks *which* surface is dragging
/// and per-surface hover/source/ghost state.
#[derive(Clone, Debug)]
pub struct DragContext<P> {
    /// Which surface is currently being dragged from, if any.
    pub active_surface: Option<DragSurfaceId>,
    /// Per-surface drag state.
    pub surfaces: HashMap<DragSurfaceId, SurfaceDragState<P>>,
}

impl<P> Default for DragContext<P> {
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

impl<P> DragContext<P> {
    /// Create a new drag context with default state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the drag state for a specific surface.
    pub fn surface(&self, id: DragSurfaceId) -> Option<&SurfaceDragState<P>> {
        self.surfaces.get(&id)
    }

    /// Get mutable drag state for a specific surface.
    pub fn surface_mut(&mut self, id: DragSurfaceId) -> Option<&mut SurfaceDragState<P>> {
        self.surfaces.get_mut(&id)
    }

    /// Returns true if any surface is actively dragging.
    pub fn is_dragging(&self) -> bool {
        self.active_surface.is_some()
    }

    /// Returns the active surface's drag state, if any.
    pub fn active(&self) -> Option<&SurfaceDragState<P>> {
        self.active_surface.and_then(|id| self.surfaces.get(&id))
    }

    /// Returns the mutable active surface's drag state, if any.
    pub fn active_mut(&mut self) -> Option<&mut SurfaceDragState<P>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drag::{DragItemId, DragPhase};

    /// A stand-in app payload — the framework never inspects it, proving the
    /// drag system is fully generic over whatever the app carries.
    #[derive(Clone, Debug, PartialEq)]
    struct TestPayload {
        id: u64,
        swap: bool,
    }

    fn dragging(id: u64, swap: bool) -> DragPhase<TestPayload> {
        DragPhase::Dragging {
            payload: TestPayload { id, swap },
        }
    }

    #[test]
    fn default_prepopulates_known_surfaces() {
        let ctx: DragContext<TestPayload> = DragContext::new();
        assert!(ctx.surface(DragSurfaceId::LeftSidebar).is_some());
        assert!(ctx.active_surface.is_none());
        assert!(!ctx.is_dragging());
    }

    #[test]
    fn set_and_clear_active_tracks_the_dragging_surface() {
        let mut ctx: DragContext<TestPayload> = DragContext::new();
        ctx.surface_mut(DragSurfaceId::LeftSidebar).unwrap().phase = dragging(7, false);
        ctx.set_active(DragSurfaceId::LeftSidebar);
        assert!(ctx.is_dragging());
        assert_eq!(
            ctx.active().and_then(|s| s.payload()).map(|p| p.id),
            Some(7)
        );
        ctx.clear_active();
        assert!(!ctx.is_dragging());
    }

    #[test]
    fn payload_round_trips_unchanged() {
        let mut ctx: DragContext<TestPayload> = DragContext::new();
        ctx.surface_mut(DragSurfaceId::LeftSidebar).unwrap().phase = dragging(42, false);
        let p = ctx
            .surface(DragSurfaceId::LeftSidebar)
            .unwrap()
            .payload()
            .unwrap();
        assert_eq!(
            *p,
            TestPayload {
                id: 42,
                swap: false
            }
        );
    }

    #[test]
    fn payload_mut_lets_the_app_toggle_a_flag_mid_drag() {
        let mut ctx: DragContext<TestPayload> = DragContext::new();
        let s = ctx.surface_mut(DragSurfaceId::LeftSidebar).unwrap();
        s.phase = DragPhase::Starting {
            payload: TestPayload { id: 1, swap: false },
            start_pos: (0.0, 0.0),
            threshold_sq: 100.0,
        };
        // Toggle swap through the generic accessor — works across Starting/Dragging
        // without the framework knowing the payload's shape.
        s.payload_mut().unwrap().swap = true;
        assert!(s.payload().unwrap().swap);
    }

    #[test]
    fn cancel_all_resets_every_surface() {
        let mut ctx: DragContext<TestPayload> = DragContext::new();
        let s = ctx.surface_mut(DragSurfaceId::LeftSidebar).unwrap();
        s.phase = dragging(3, true);
        s.hover_item = Some(DragItemId::new(2));
        s.source_item = Some(DragItemId::new(1));
        ctx.set_active(DragSurfaceId::LeftSidebar);

        ctx.cancel_all();

        let s = ctx.surface(DragSurfaceId::LeftSidebar).unwrap();
        assert!(!s.is_dragging());
        assert!(s.hover_item.is_none());
        assert!(s.source_item.is_none());
        assert!(ctx.active_surface.is_none());
    }
}
