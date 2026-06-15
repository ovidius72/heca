//! Per-surface drag state and phase, generic over an app-defined payload `P`.
//!
//! [`DragPhase`] models the lifecycle of a drag within a single surface:
//! `Idle → Starting → Dragging → Idle`. The `Starting` phase captures the
//! threshold state before a click becomes a drag.
//!
//! The framework carries no knowledge of *what* is being dragged: the payload
//! `P` is defined by the app (e.g. a pane id + origin workspace + swap flag) and
//! threaded through unchanged. This keeps the drag system domain-neutral and
//! reusable by any surface/widget.

use crate::drag::DragItemId;

/// Phase of a surface-local drag, carrying the app payload `P`.
///
/// Transitions:
/// ```text
/// Idle → Starting  (pointer press on a draggable item; payload captured)
/// Starting → Dragging  (threshold exceeded)
/// Starting → Idle  (released without exceeding threshold — app dispatches click)
/// Dragging → Idle  (drop or cancel)
/// ```
#[derive(Clone, Debug)]
pub enum DragPhase<P> {
    /// Not dragging.
    Idle,
    /// Threshold phase: pointer pressed, waiting to see if it's a drag or click.
    Starting {
        /// The app payload describing what is being dragged.
        payload: P,
        /// Cursor position at press time.
        start_pos: (f32, f32),
        /// Squared pixel threshold for drag start.
        threshold_sq: f32,
    },
    /// Active drag: threshold exceeded, cursor is being tracked.
    Dragging {
        /// The app payload describing what is being dragged.
        payload: P,
    },
}

/// Per-surface drag state. Each surface owns one independently.
#[derive(Clone, Debug)]
pub struct SurfaceDragState<P> {
    /// Current drag phase for this surface.
    pub phase: DragPhase<P>,
    /// Which item the cursor is hovering over (drop target highlight).
    pub hover_item: Option<DragItemId>,
    /// Which item is the drag source (visual dim/strike-through).
    pub source_item: Option<DragItemId>,
    /// Ghost label for drag-over rendering.
    pub ghost_label: Option<DragLabel>,
}

impl<P> Default for SurfaceDragState<P> {
    fn default() -> Self {
        Self {
            phase: DragPhase::Idle,
            hover_item: None,
            source_item: None,
            ghost_label: None,
        }
    }
}

/// Visual info for a drag ghost label.
///
/// Used by the renderer to draw the name of the dragged item following
/// the cursor during a drag. Domain-neutral: just text + a rect.
#[derive(Clone, Debug, PartialEq)]
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

impl<P> SurfaceDragState<P> {
    /// Returns true if this surface is in an active drag (Starting or Dragging).
    pub fn is_dragging(&self) -> bool {
        !matches!(self.phase, DragPhase::Idle)
    }

    /// Borrow the payload of the in-flight drag (Starting or Dragging), if any.
    pub fn payload(&self) -> Option<&P> {
        match &self.phase {
            DragPhase::Starting { payload, .. } | DragPhase::Dragging { payload } => Some(payload),
            DragPhase::Idle => None,
        }
    }

    /// Mutably borrow the payload of the in-flight drag, if any. Lets the app
    /// update drag parameters mid-gesture (e.g. toggle a swap flag when a
    /// modifier is pressed) without the framework knowing the payload's shape.
    pub fn payload_mut(&mut self) -> Option<&mut P> {
        match &mut self.phase {
            DragPhase::Starting { payload, .. } | DragPhase::Dragging { payload } => Some(payload),
            DragPhase::Idle => None,
        }
    }

    /// Reset to Idle, clearing all associated state.
    pub fn reset(&mut self) {
        self.phase = DragPhase::Idle;
        self.hover_item = None;
        self.source_item = None;
        self.ghost_label = None;
    }
}
