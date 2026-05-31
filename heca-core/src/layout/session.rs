use super::animation::{Animation, AnimationConfig, Animated};
use super::column::Pane;
use super::types::*;
use super::workspace::Workspace;

/// A session manages all workspaces, the overview/expose mode, and workspace switching.
///
/// This is the top-level layout container. In NIRI terms, this combines
/// `Layout<W>` (monitors + workspaces) and adds the overview state.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub workspaces: Vec<Workspace>,
    pub active_workspace_idx: usize,
    pub overview: OverviewState,
    pub workspace_switch: WorkspaceSwitch,
    /// Viewport size (full window size).
    pub viewport_size: Size,
    /// Scale factor.
    pub scale: f64,
    /// Layout options.
    pub options: LayoutOptions,
    /// Next ID counter.
    next_id: u64,
}

/// Overview / Expose mode state.
#[derive(Debug, Clone)]
pub struct OverviewState {
    pub open: bool,
    /// 0.0 = normal view, 1.0 = fully zoomed out overview.
    pub progress: Animated<f64>,
    /// Scale factor for overview (e.g., 0.25 = 25% size).
    pub zoom: f64,
    /// Gap between workspace thumbnails.
    pub gap: f64,
}

impl Default for OverviewState {
    fn default() -> Self {
        Self {
            open: false,
            progress: Animated::Static(0.0),
            zoom: 0.25,
            gap: 16.0,
        }
    }
}

/// Workspace switching state.
#[derive(Debug, Clone, Default)]
pub enum WorkspaceSwitch {
    #[default]
    None,
    /// Animated transition between workspaces.
    Animation {
        from_idx: usize,
        to_idx: usize,
        progress: Animated<f64>,
    },
    /// Gesture-driven switch (touchpad swipe).
    Gesture {
        tracker: super::animation::SwipeTracker,
        current_idx: f64,
    },
}

impl Session {
    pub fn new(id: SessionId, viewport_size: Size, scale: f64) -> Self {
        let options = LayoutOptions::default();
        let working_area = Rectangle::new(
            Point::new(0.0, 0.0),
            viewport_size,
        );

        let mut session = Self {
            id,
            workspaces: Vec::new(),
            active_workspace_idx: 0,
            overview: OverviewState::default(),
            workspace_switch: WorkspaceSwitch::None,
            viewport_size,
            scale,
            options: options.clone(),
            next_id: 1,
        };

        // Create initial workspace.
        session.add_workspace(working_area);

        session
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Create a new workspace and append it.
    pub fn add_workspace(&mut self, working_area: Rectangle) -> WorkspaceId {
        let id = WorkspaceId(self.next_id());
        let ws = Workspace::new(id, working_area, self.scale, self.options.clone());
        self.workspaces.push(ws);
        id
    }

    /// Remove a workspace by index. Returns true if removed.
    pub fn remove_workspace(&mut self, idx: usize) -> bool {
        if idx >= self.workspaces.len() || self.workspaces.len() <= 1 {
            return false;
        }
        self.workspaces.remove(idx);
        if self.active_workspace_idx > idx {
            // The active workspace was after the removed one; shift down.
            self.active_workspace_idx -= 1;
        } else if self.active_workspace_idx >= self.workspaces.len() {
            // The removed workspace was the last one; clamp.
            self.active_workspace_idx = self.workspaces.len() - 1;
        }
        true
    }

    /// Get the active workspace.
    pub fn active_workspace(&self) -> Option<&Workspace> {
        self.workspaces.get(self.active_workspace_idx)
    }

    pub fn active_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.workspaces.get_mut(self.active_workspace_idx)
    }

    /// Switch to a workspace by index with animation.
    pub fn switch_to_workspace(&mut self, idx: usize) {
        if idx >= self.workspaces.len() || idx == self.active_workspace_idx {
            return;
        }

        let from_idx = self.active_workspace_idx;
        self.workspace_switch = WorkspaceSwitch::Animation {
            from_idx,
            to_idx: idx,
            progress: Animated::Animating {
                animation: Animation::new(0.0, 1.0, AnimationConfig::default()),
                from: 0.0,
                to: 1.0,
            },
        };
        self.active_workspace_idx = idx;
    }

    /// Switch workspace up (to previous workspace).
    pub fn switch_workspace_up(&mut self) -> bool {
        if self.active_workspace_idx == 0 {
            return false;
        }
        self.switch_to_workspace(self.active_workspace_idx - 1);
        true
    }

    /// Switch workspace down (to next workspace).
    pub fn switch_workspace_down(&mut self) -> bool {
        let next = self.active_workspace_idx + 1;
        if next >= self.workspaces.len() {
            // Create a new empty workspace if at the end.
            let working_area = Rectangle::new(Point::default(), self.viewport_size);
            self.add_workspace(working_area);
        }
        self.switch_to_workspace(next);
        true
    }

    /// Toggle the overview/expose mode.
    pub fn toggle_overview(&mut self) {
        self.overview.open = !self.overview.open;
        let from = match &self.overview.progress {
            Animated::Static(v) => *v,
            Animated::Animating { animation, .. } => animation.value(),
        };
        let to = if self.overview.open { 1.0 } else { 0.0 };
        self.overview.progress = Animated::Animating {
            animation: Animation::new(from, to, AnimationConfig::default()),
            from,
            to,
        };
    }

    /// Compute the zoom factor for the current overview progress.
    pub fn overview_zoom(&self) -> f64 {
        let progress = self.overview.progress.current();
        let scale = self.options.overview_scale;
        // Zoom from 1.0 down to scale as progress goes 0 -> 1.
        1.0 - (1.0 - scale) * progress
    }

    /// Get workspace geometries for rendering.
    ///
    /// Returns each workspace with its position and size for the current frame.
    /// In normal mode, only the active workspace is at (0, 0) with full viewport size.
    /// In overview mode, all workspaces are stacked vertically as thumbnails.
    pub fn workspace_geometries(&self) -> Vec<(usize, Rectangle)> {
        if !self.overview.is_active() {
            // Normal mode: only active workspace visible.
            if self.active_workspace().is_some() {
                vec![(self.active_workspace_idx, Rectangle::new(Point::default(), self.viewport_size))]
            } else {
                vec![]
            }
        } else {
            // Overview mode: all workspaces as thumbnails.
            self.overview_workspace_geometries()
        }
    }

    fn overview_workspace_geometries(&self) -> Vec<(usize, Rectangle)> {
        let zoom = self.overview_zoom();
        let ws_size = Size::new(
            self.viewport_size.w * zoom,
            self.viewport_size.h * zoom,
        );
        let gap = self.options.overview_gap * zoom;
        let ws_height = ws_size.h + gap;

        // Center the active workspace vertically.
        let active_y_offset = self.active_workspace_idx as f64 * ws_height;
        let total_height = self.workspaces.len() as f64 * ws_height - gap;
        let start_y = (self.viewport_size.h - total_height) / 2.0 - active_y_offset;
        let center_x = (self.viewport_size.w - ws_size.w) / 2.0;

        self.workspaces
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                let y = start_y + idx as f64 * ws_height;
                let rect = Rectangle::new(
                    Point::new(center_x, y),
                    ws_size,
                );
                (idx, rect)
            })
            .collect()
    }

    /// Advance all animations in the session.
    pub fn advance_animations(&mut self) {
        // Advance overview animation.
        if let Animated::Animating { ref animation, .. } = self.overview.progress
            && animation.is_done()
        {
            let final_value = animation.target();
            self.overview.progress = Animated::Static(final_value);
        }

        // Advance workspace switch animation.
        match &mut self.workspace_switch {
            WorkspaceSwitch::Animation {
                progress: Animated::Animating { animation, .. },
                ..
            } if animation.is_done() => {
                self.workspace_switch = WorkspaceSwitch::None;
            }
            WorkspaceSwitch::Gesture { .. } => {
                // Gestures are driven by input events.
            }
            _ => {}
        }

        // Advance workspace animations.
        for ws in &mut self.workspaces {
            ws.advance_animations();
        }
    }

    /// Check if any animations are ongoing.
    pub fn are_animations_ongoing(&self) -> bool {
        !self.overview.progress.is_done()
            || matches!(self.workspace_switch, WorkspaceSwitch::Animation { .. })
            || self.workspaces.iter().any(|w| w.are_animations_ongoing())
    }

    /// Add a pane to the active workspace.
    pub fn add_pane(&mut self, pane: Pane, column_idx: Option<usize>, activate: bool) {
        let width = self.options.default_column_width.unwrap_or(ColumnWidth::Proportion(0.85));
        if let Some(ws) = self.active_workspace_mut() {
            ws.add_pane(pane, column_idx, activate, width);
        }
    }

    /// Focus left in the active workspace.
    pub fn focus_left(&mut self) -> bool {
        if let Some(ws) = self.active_workspace_mut() {
            ws.focus_left()
        } else {
            false
        }
    }

    /// Focus right in the active workspace.
    pub fn focus_right(&mut self) -> bool {
        if let Some(ws) = self.active_workspace_mut() {
            ws.focus_right()
        } else {
            false
        }
    }

    /// Focus up (previous pane or workspace).
    pub fn focus_up(&mut self) -> bool {
        if let Some(ws) = self.active_workspace_mut()
            && ws.focus_up()
        {
            return true;
        }
        self.switch_workspace_up()
    }

    /// Focus down (next pane or workspace).
    pub fn focus_down(&mut self) -> bool {
        if let Some(ws) = self.active_workspace_mut()
            && ws.focus_down()
        {
            return true;
        }
        self.switch_workspace_down()
    }

    /// Update viewport size (e.g., on window resize).
    pub fn update_viewport(&mut self, size: Size) {
        self.viewport_size = size;
        let working_area = Rectangle::new(Point::default(), size);
        for ws in &mut self.workspaces {
            ws.update_working_area(working_area);
        }
    }

}

impl OverviewState {
    /// Whether overview is fully or partially active.
    pub fn is_active(&self) -> bool {
        match &self.progress {
            Animated::Static(v) => *v > 0.001,
            Animated::Animating { .. } => true,
        }
    }


}
