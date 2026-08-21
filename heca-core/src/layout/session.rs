use super::animation::{Animated, Animation, AnimationConfig};
use super::column::Pane;
use super::types::*;
use super::workspace::Workspace;

/// A session manages all workspaces and workspace switching.
///
/// This is the top-level layout container — NIRI's `Layout<W>` (monitors + workspaces).
///
/// **It has no overview state.** heca's overview is the exposé, a *layer* built out of widgets
/// (`heca/src/chrome/expose/`) that draws its own map and never asks the session to zoom. The
/// pre-layer overview that lived here — `OverviewState`, `toggle_overview`, `overview_zoom`,
/// `overview_workspace_geometries` — was unreachable code by the time it was removed: nothing
/// outside this file ever set it active (F003/P082/T422).
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub workspaces: Vec<Workspace>,
    pub active_workspace_idx: usize,
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
    pub fn new(id: SessionId, viewport_size: Size, scale: f64, options: LayoutOptions) -> Self {
        let working_area = Rectangle::new(Point::new(0.0, 0.0), viewport_size);

        let mut session = Self {
            id,
            workspaces: Vec::new(),
            active_workspace_idx: 0,
            workspace_switch: WorkspaceSwitch::None,
            viewport_size,
            scale,
            options,
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
    /// Remove the workspace at `idx`. Returns `false` only for an out-of-range index.
    /// Removing the **last** workspace is allowed and leaves the session empty —
    /// `active_workspace()` then returns `None` until a new workspace is created
    /// (e.g. via `add_workspace`); the app renders blank and stays recoverable.
    pub fn remove_workspace(&mut self, idx: usize) -> bool {
        if idx >= self.workspaces.len() {
            return false;
        }
        self.workspaces.remove(idx);
        if self.workspaces.is_empty() {
            // Session emptied — keep the index in a benign state (`get` → `None`).
            self.active_workspace_idx = 0;
        } else if self.active_workspace_idx > idx {
            // The active workspace was after the removed one; shift down.
            self.active_workspace_idx -= 1;
        } else if self.active_workspace_idx >= self.workspaces.len() {
            // The removed workspace was the last one; clamp to the new last.
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

    /// Get workspace geometries for rendering.
    ///
    /// The active workspace at `(0, 0)`, the size of the viewport — and nothing else, because only
    /// one workspace is on screen at a time. There used to be a second branch here that stacked
    /// every workspace as a thumbnail for the overview; it went with the overview itself, which
    /// nothing had been able to reach since the exposé became a layer (F003/P082/T422).
    pub fn workspace_geometries(&self) -> Vec<(usize, Rectangle)> {
        if self.active_workspace().is_some() {
            vec![(
                self.active_workspace_idx,
                Rectangle::new(Point::default(), self.viewport_size),
            )]
        } else {
            vec![]
        }
    }

    /// Advance all animations in the session.
    pub fn advance_animations(&mut self) {
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
        matches!(self.workspace_switch, WorkspaceSwitch::Animation { .. })
            || self.workspaces.iter().any(|w| w.are_animations_ongoing())
    }

    /// Add a pane to the active workspace.
    pub fn add_pane(&mut self, pane: Pane, column_idx: Option<usize>, activate: bool) {
        let width = self
            .options
            .default_column_width
            .unwrap_or(ColumnWidth::Proportion(0.85));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session_with_workspaces(count: usize) -> Session {
        let viewport = Size::new(1280.0, 800.0);
        let mut session = Session::new(SessionId(1), viewport, 2.0, LayoutOptions::default());
        // Session::new creates one workspace; add more if needed.
        for _ in 1..count {
            let wa = session
                .active_workspace()
                .map(|ws| {
                    Rectangle::new(
                        ws.scrolling.working_area.loc,
                        ws.scrolling.working_area.size,
                    )
                })
                .unwrap_or_else(|| Rectangle::new(Point::default(), viewport));
            session.add_workspace(wa);
        }
        session
    }

    #[test]
    fn test_remove_workspace_active_after_removed() {
        // 3 workspaces; active is 2. Remove ws 0.
        // active_workspace_idx (2) > removed (0) → shift down to 1.
        let mut session = make_session_with_workspaces(3);
        session.active_workspace_idx = 2;
        assert!(session.remove_workspace(0));
        assert_eq!(session.workspaces.len(), 2);
        assert_eq!(session.active_workspace_idx, 1);
    }

    #[test]
    fn test_remove_workspace_active_is_last() {
        // 3 workspaces; active is 2. Remove ws 2 (last).
        // active_workspace_idx (2) >= len (2) after removal → clamp to 1.
        let mut session = make_session_with_workspaces(3);
        session.active_workspace_idx = 2;
        assert!(session.remove_workspace(2));
        assert_eq!(session.workspaces.len(), 2);
        assert_eq!(session.active_workspace_idx, 1);
    }

    #[test]
    fn test_remove_workspace_active_before_removed() {
        // 3 workspaces; active is 0. Remove ws 1.
        // active_workspace_idx (0) is not > removed (1) and not >= len (2)
        // → unchanged at 0.
        let mut session = make_session_with_workspaces(3);
        session.active_workspace_idx = 0;
        assert!(session.remove_workspace(1));
        assert_eq!(session.workspaces.len(), 2);
        assert_eq!(session.active_workspace_idx, 0);
    }

    #[test]
    fn test_remove_workspace_can_remove_last_leaving_empty() {
        // Removing the sole workspace is now allowed; the session is left empty and
        // `active_workspace()` returns `None` (no underflow on the index clamp).
        let mut session = make_session_with_workspaces(1);
        assert!(session.remove_workspace(0));
        assert_eq!(session.workspaces.len(), 0);
        assert_eq!(session.active_workspace_idx, 0);
        assert!(session.active_workspace().is_none());
    }

    #[test]
    fn test_remove_workspace_out_of_bounds() {
        let mut session = make_session_with_workspaces(2);
        assert!(!session.remove_workspace(5));
        assert_eq!(session.workspaces.len(), 2);
    }

    #[test]
    fn test_remove_workspace_active_is_removed() {
        // 3 workspaces; active is 1. Remove ws 1.
        // active_workspace_idx (1) is not > removed (1) and not >= len (2)
        // → unchanged at 1, which now points to the former workspace 2.
        let mut session = make_session_with_workspaces(3);
        session.active_workspace_idx = 1;
        assert!(session.remove_workspace(1));
        assert_eq!(session.workspaces.len(), 2);
        assert_eq!(session.active_workspace_idx, 1);
    }
}
