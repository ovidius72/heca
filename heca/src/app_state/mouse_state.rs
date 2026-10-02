//! What the pointer is doing: a drag in flight, a divider being resized.

use super::*;

/// State for the interactive content-area drag (pane moved by mouse).
///
/// This is the app's own gesture, and the only one left: a dragged ROW is the framework's, which
/// runs it and hands back a drop. Interactive move stays here because it detaches a pane from the
/// layout, shows a ghost pane following the cursor and computes an insert hint — content-area
/// concepts a widget knows nothing about.
#[derive(Clone, Debug)]
pub enum InteractiveMovePhase {
    /// Phase 1: rubberband — pane still in layout, waiting for threshold.
    Starting {
        pane_id: PaneId,
        /// Workspace where the drag originated.
        original_ws: usize,
        start_mouse: (f32, f32),
        threshold_sq: f32,
        /// If true, drop performs a swap instead of a move.
        swap: bool,
    },
    /// Phase 2: detached — pane follows pointer (move mode).
    /// In swap mode, the pane stays in layout and only the insert hint is shown.
    Moving {
        pane_id: PaneId,
        /// Workspace where the drag originated.
        _original_ws: usize,
        /// Mouse offset from pane top-left at grab time.
        offset: (f32, f32),
        /// If true, drop performs a swap instead of a move.
        swap: bool,
    },
}


/// A pane that has been removed from the layout for interactive move.
#[derive(Clone, Debug)]
pub struct DetachedPane {
    pub pane: heca_core::layout::Pane,
    pub render_pos: heca_core::layout::types::Point,
    pub size: heca_core::layout::types::Size,
    pub original_ws: usize,
    pub _original_col: usize,
    pub original_col_id: heca_core::layout::ColumnId,
    pub original_pane: usize,
}


/// Which layout divider a mouse resize-drag is acting on.
///
/// Indices are into the **active workspace's** `scrolling.columns` (the same
/// indices [`crate::find_pane_location`] returns), so they map straight onto
/// [`heca_core::layout::scrolling::ScrollingSpace::resize_column`] /
/// `resize_pane_height`. A resize-drag never leaves the active workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeDivider {
    /// The vertical gap to the right of column `col` → resize that column's width.
    Column { col: usize },
    /// The horizontal gap below pane `pane` in column `col` → resize that pane's height.
    Pane { col: usize, pane: usize },
}


/// An in-flight mouse resize-drag (drag a column/pane divider). Distinct from the
/// DnD item-move surfaces: this mutates layout sizes, not pane positions.
#[derive(Clone, Copy, Debug)]
pub struct ResizeDrag {
    pub divider: ResizeDivider,
    /// Cursor position at the last applied delta; the next move resizes by the
    /// incremental difference so the divider tracks the pointer.
    pub last_pos: (f32, f32),
}


/// All mouse-related runtime state.
#[derive(Clone, Debug)]
pub struct MouseState {
    pub pos: (f32, f32),
    /// In-flight column/pane divider resize-drag (`None` when not resizing).
    pub resize: Option<ResizeDrag>,
    /// Content-area interactive move state (separate from surface drags).
    pub interactive_move: Option<InteractiveMovePhase>,
    /// Pane being dragged (detached from layout).
    pub detached_pane: Option<DetachedPane>,
    /// Computed drop target during interactive move.
    pub insert_hint: Option<heca_core::layout::types::PaneInsertTarget>,
    /// Last time edge scroll was processed (for frame-rate independence).
    pub last_edge_scroll_time: Option<std::time::Instant>,
}


impl MouseState {
    pub fn new() -> Self {
        Self {
            pos: (0.0, 0.0),
            resize: None,
            interactive_move: None,
            detached_pane: None,
            insert_hint: None,
            last_edge_scroll_time: None,
        }
    }
}

