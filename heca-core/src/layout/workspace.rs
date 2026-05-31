use super::animation::{Animation, AnimationConfig};
use super::scrolling::ScrollingSpace;
use super::types::*;
use super::view_offset::ViewOffset;

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
    /// Whether the floating layout is active.
    pub floating_is_active: bool,
    /// Whether floating panes should render on top even when not active.
    pub floating_visible: bool,
    /// Whether this workspace should be kept even when empty.
    pub is_pinned: bool,
    /// Whether this workspace is currently visible on any output.
    pub is_visible: bool,
}

/// A floating pane with position and size.
#[derive(Debug, Clone)]
pub struct FloatingPane {
    pub pane: super::column::Pane,
    pub position: Point,
    pub size: Size,
    pub is_active: bool,
    /// Original column index when floated (for restore).
    pub original_column_idx: Option<usize>,
    /// Original pane index within the column when floated.
    pub original_pane_idx: Option<usize>,
}

impl Workspace {
    pub fn new(id: WorkspaceId, working_area: Rectangle, scale: f64, options: LayoutOptions) -> Self {
        let scrolling = ScrollingSpace::new(working_area, scale, options);
        Self {
            id,
            name: None,
            scrolling,
            floating_panes: Vec::new(),
            floating_is_active: false,
            floating_visible: true,
            is_pinned: false,
            is_visible: false,
        }
    }

    pub fn has_panes(&self) -> bool {
        !self.scrolling.is_empty() || !self.floating_panes.is_empty()
    }

    pub fn has_windows_or_name(&self) -> bool {
        self.has_panes() || self.name.is_some()
    }

    pub fn active_pane(&self) -> Option<&super::column::Pane> {
        if self.floating_is_active {
            self.floating_panes.iter().find(|p| p.is_active).map(|p| &p.pane)
        } else {
            self.scrolling.active_pane()
        }
    }

    /// Find any pane by ID across both scrolling and floating.
    pub fn find_pane(&self, pane_id: PaneId) -> Option<&super::column::Pane> {
        for col in &self.scrolling.columns {
            for pane in &col.panes {
                if pane.id == pane_id {
                    return Some(pane);
                }
            }
        }
        for float in &self.floating_panes {
            if float.pane.id == pane_id {
                return Some(&float.pane);
            }
        }
        None
    }

    /// Update working area (called on resize).
    pub fn update_working_area(&mut self, working_area: Rectangle) {
        self.scrolling.update_working_area(working_area);
        // TODO: update floating pane bounds
    }

    /// Advance all animations in this workspace.
    pub fn advance_animations(&mut self) {
        self.scrolling.advance_animations();
    }

    pub fn are_animations_ongoing(&self) -> bool {
        self.scrolling.are_animations_ongoing()
    }

    /// Add a pane to the scrolling layout.
    pub fn add_pane(
        &mut self,
        pane: super::column::Pane,
        column_idx: Option<usize>,
        activate: bool,
        width: ColumnWidth,
    ) {
        use super::column::Column;

        if let Some(idx) = column_idx {
            // Add to existing column.
            self.scrolling.add_pane_to_column(idx, None, pane, activate);
        } else {
            // Create new column.
            let col = Column::new(
                ColumnId(self.id.0 * 1000 + self.scrolling.columns.len() as u64),
                pane,
                width,
            );
            self.scrolling.add_column(None, col, activate);
        }
    }

    /// Focus left in the scrolling layout.
    pub fn focus_left(&mut self) -> bool {
        if self.floating_is_active {
            false // TODO: floating focus
        } else {
            self.scrolling.focus_left()
        }
    }

    /// Focus right in the scrolling layout.
    pub fn focus_right(&mut self) -> bool {
        if self.floating_is_active {
            false // TODO: floating focus
        } else {
            self.scrolling.focus_right()
        }
    }

    /// Focus up (previous pane in column, or previous workspace).
    pub fn focus_up(&mut self) -> bool {
        if self.floating_is_active {
            false // TODO
        } else if let Some(col) = self.scrolling.active_column_mut() {
            if col.focus_up() {
                true
            } else {
                // Wrap to previous column's last pane.
                let col_idx = self.scrolling.active_column_idx;
                if col_idx > 0 {
                    self.scrolling.activate_column(col_idx - 1);
                    if let Some(new_col) = self.scrolling.active_column_mut() {
                        let last_idx = new_col.panes.len().saturating_sub(1);
                        new_col.activate_pane(last_idx);
                    }
                    true
                } else {
                    false
                }
            }
        } else {
            false
        }
    }

    /// Focus down (next pane in column, or next workspace).
    pub fn focus_down(&mut self) -> bool {
        if self.floating_is_active {
            false // TODO
        } else if let Some(col) = self.scrolling.active_column_mut() {
            if col.focus_down() {
                true
            } else {
                // Wrap to next column's first pane.
                let col_idx = self.scrolling.active_column_idx;
                if col_idx + 1 < self.scrolling.columns.len() {
                    self.scrolling.activate_column(col_idx + 1);
                    if let Some(new_col) = self.scrolling.active_column_mut() {
                        new_col.activate_pane(0);
                    }
                    true
                } else {
                    false
                }
            }
        } else {
            false
        }
    }
}
