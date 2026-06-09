//! Per-surface drag state and phase.
//!
//! [`SurfaceDragPhase`] models the lifecycle of a drag within a single surface:
//! `Idle → Starting → Dragging → Idle`. The `Starting` phase captures the
//! threshold state before a click becomes a drag.
//!
//! App-specific click actions (`WmAction`) are tracked at the app layer
//! (`MouseState`), not here — this keeps the framework dependency-free.

use crate::drag::{DragItemId, DragItemKind};

/// Phase of a surface-local drag.
///
/// Transitions:
/// ```text
/// Idle → Starting  (mouse press on an item)
/// Starting → Dragging  (threshold exceeded)
/// Starting → Idle  (released without exceeding threshold — app dispatches click)
/// Dragging → Idle  (drop or cancel)
/// ```
#[derive(Clone, Debug)]
pub enum SurfaceDragPhase {
    /// Not dragging.
    Idle,
    /// Threshold phase: mouse pressed, waiting to see if it's a drag or click.
    Starting {
        /// The item being pressed.
        kind: DragItemKind,
        /// Pane ID of the pressed item, if applicable.
        pane_id: Option<u64>,
        /// Original workspace index of the pressed item.
        original_ws: usize,
        /// Cursor position at press time.
        start_pos: (f32, f32),
        /// Squared pixel threshold for drag start.
        threshold_sq: f32,
        /// If true, drop performs a swap instead of a move.
        swap: bool,
    },
    /// Active drag: threshold exceeded, cursor is being tracked.
    Dragging {
        /// The item kind being dragged.
        kind: DragItemKind,
        /// Pane ID of the dragged item, if applicable.
        pane_id: Option<u64>,
        /// Original workspace index.
        original_ws: usize,
        /// If true, drop performs a swap instead of a move.
        swap: bool,
    },
}



/// Per-surface drag state. Each surface owns one independently.
#[derive(Clone, Debug)]
pub struct SurfaceDragState {
    /// Current drag phase for this surface.
    pub phase: SurfaceDragPhase,
    /// Which item the cursor is hovering over (drop target highlight).
    pub hover_item: Option<DragItemId>,
    /// Which item is the drag source (visual dim/strike-through).
    pub source_item: Option<DragItemId>,
    /// Ghost label for drag-over rendering.
    pub ghost_label: Option<DragLabel>,
}

impl Default for SurfaceDragState {
    fn default() -> Self {
        Self {
            phase: SurfaceDragPhase::Idle,
            hover_item: None,
            source_item: None,
            ghost_label: None,
        }
    }
}

/// Visual info for a drag ghost label.
///
/// Used by the renderer to draw the name of the dragged item following
/// the cursor during a sidebar drag.
#[derive(Clone, Debug)]
pub struct DragLabel {
    /// Display text for the ghost label.
    pub text: String,
    /// X position of the label (logical pixels).
    pub x: f32,
    /// Y position of the label (logical pixels).
    pub y: f32,
    /// Width of the label background (logical pixels).
    pub width: f32,
    /// Height of the label background (logical pixels).
    pub height: f32,
}

/// Convenience: return the active drag surface from the phase, if any.
impl SurfaceDragState {
    /// Returns true if this surface is in an active drag (Starting or Dragging).
    pub fn is_dragging(&self) -> bool {
        !matches!(self.phase, SurfaceDragPhase::Idle)
    }

    /// Returns the pane_id of the item being dragged, if applicable.
    pub fn dragged_pane_id(&self) -> Option<u64> {
        match &self.phase {
            SurfaceDragPhase::Starting { pane_id, .. } => *pane_id,
            SurfaceDragPhase::Dragging { pane_id, .. } => *pane_id,
            SurfaceDragPhase::Idle => None,
        }
    }

    /// Returns the original workspace index of the drag, if active.
    pub fn original_ws(&self) -> Option<usize> {
        match &self.phase {
            SurfaceDragPhase::Starting { original_ws, .. } => Some(*original_ws),
            SurfaceDragPhase::Dragging { original_ws, .. } => Some(*original_ws),
            SurfaceDragPhase::Idle => None,
        }
    }

    /// Returns whether the drag is in swap mode.
    pub fn is_swap(&self) -> bool {
        match &self.phase {
            SurfaceDragPhase::Starting { swap, .. } => *swap,
            SurfaceDragPhase::Dragging { swap, .. } => *swap,
            SurfaceDragPhase::Idle => false,
        }
    }

    /// Reset to Idle, clearing all associated state.
    pub fn reset(&mut self) {
        self.phase = SurfaceDragPhase::Idle;
        self.hover_item = None;
        self.source_item = None;
        self.ghost_label = None;
    }
}