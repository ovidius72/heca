//! **The layout as one window sees it** — the one door to everything that needs both the shared
//! content ([`Session`]) and where this window is looking ([`WindowView`]).
//!
//! A caller holds a [`Layout`] to read and a [`LayoutMut`] to move, and never has to know a view
//! exists: `layout.focus_right()` moves the focus *and* the scroll that follows it, in the window
//! that asked.
//!
//! ```
//! use heca_core::layout::{LayoutOptions, Pane, PaneId, Session, SessionId, Size, WindowView};
//!
//! let mut session = Session::new(SessionId(1), LayoutOptions::default());
//! let mut view = WindowView::new(Size::new(1280.0, 800.0), 2.0);
//! let mut layout = session.through_mut(&mut view);
//! layout.add_pane(Pane::new(PaneId(1), "a"), None, true);
//! layout.add_pane(Pane::new(PaneId(2), "b"), None, true);
//! layout.focus_left();
//! let x = layout.reader().workspace(0).map(|ws| ws.scroll().view_pos());
//! assert!(x.is_some());
//! ```

use super::animation::{Animated, Animation, AnimationConfig};
use super::column::Pane;
use super::session::Session;
use super::types::{ColumnId, Point, Rectangle, Size, WorkspaceId};
use super::window_view::{WindowView, WorkspaceSwitch};
use super::workspace::{WorkspaceMut, WorkspaceRef};

/// The session with one window's view of it: the reads.
#[derive(Clone, Copy)]
pub struct Layout<'a> {
    session: &'a Session,
    view: &'a WindowView,
}

/// The session with one window's view of it: the moves.
pub struct LayoutMut<'a> {
    session: &'a mut Session,
    view: &'a mut WindowView,
}

impl Session {
    /// This session as the window holding `view` sees it.
    pub fn through<'a>(&'a self, view: &'a WindowView) -> Layout<'a> {
        Layout {
            session: self,
            view,
        }
    }

    /// This session, moved by the window holding `view`.
    pub fn through_mut<'a>(&'a mut self, view: &'a mut WindowView) -> LayoutMut<'a> {
        LayoutMut {
            session: self,
            view,
        }
    }
}

impl<'a> Layout<'a> {
    /// The shared content.
    pub fn session(&self) -> &'a Session {
        self.session
    }

    /// The window's size.
    pub fn viewport(&self) -> Size {
        self.view.viewport()
    }

    /// The window's scale factor.
    pub fn scale(&self) -> f64 {
        self.view.scale()
    }

    /// Index of the workspace the window shows.
    pub fn active_workspace_idx(&self) -> usize {
        self.view.active_workspace
    }

    /// The workspace at `idx`, as the window sees it.
    pub fn workspace(&self, idx: usize) -> Option<WorkspaceRef<'a>> {
        let ws = self.session.workspaces.get(idx)?;
        Some(ws.through(self.view.scroll(ws.id)))
    }

    /// The workspace the window shows.
    pub fn active_workspace(&self) -> Option<WorkspaceRef<'a>> {
        self.workspace(self.view.active_workspace)
    }

    /// Every workspace, in order, as the window sees them.
    pub fn workspaces(&self) -> impl Iterator<Item = WorkspaceRef<'a>> + use<'a> {
        let (session, view) = (self.session, self.view);
        session
            .workspaces
            .iter()
            .map(move |ws| ws.through(view.scroll(ws.id)))
    }

    /// Workspace geometries for rendering.
    ///
    /// The active workspace at `(0, 0)`, the size of the viewport — and nothing else, because only
    /// one workspace is on screen at a time. There used to be a second branch here that stacked
    /// every workspace as a thumbnail for the overview; it went with the overview itself, which
    /// nothing had been able to reach since the exposé became a layer (F003/P082/T422).
    pub fn workspace_geometries(&self) -> Vec<(usize, Rectangle)> {
        if self.active_workspace().is_some() {
            vec![(
                self.view.active_workspace,
                Rectangle::new(Point::default(), self.view.viewport()),
            )]
        } else {
            vec![]
        }
    }

    /// Check if any animations are ongoing.
    pub fn are_animations_ongoing(&self) -> bool {
        matches!(self.view.switch, WorkspaceSwitch::Animation { .. })
            || self.workspaces().any(|w| w.are_animations_ongoing())
    }
}

impl LayoutMut<'_> {
    /// The same layout, for the reads that only look.
    pub fn reader(&self) -> Layout<'_> {
        Layout {
            session: self.session,
            view: self.view,
        }
    }

    /// The workspace at `idx`, moved by the window.
    pub fn workspace_mut(&mut self, idx: usize) -> Option<WorkspaceMut<'_>> {
        let ws = self.session.workspaces.get_mut(idx)?;
        let scroll = self.view.scroll_mut(ws.id);
        Some(ws.through_mut(scroll))
    }

    /// The workspace the window shows, moved by the window.
    pub fn active_workspace_mut(&mut self) -> Option<WorkspaceMut<'_>> {
        self.workspace_mut(self.view.active_workspace)
    }

    /// The next identity from the session's one counter — for the column a move may have to
    /// create, allocated before the workspace is borrowed.
    pub fn next_id(&mut self) -> u64 {
        self.session.next_id()
    }

    /// Create a new workspace and append it.
    pub fn add_workspace(&mut self) -> WorkspaceId {
        self.session.add_workspace()
    }

    /// Remove the workspace at `idx`. Returns `false` only for an out-of-range index.
    /// Removing the **last** workspace is allowed and leaves the session empty —
    /// `active_workspace()` then returns `None` until a new workspace is created
    /// (e.g. via `add_workspace`); the app renders blank and stays recoverable.
    pub fn remove_workspace(&mut self, idx: usize) -> bool {
        let Some(ws) = self.session.workspaces.get(idx) else {
            return false;
        };
        self.view.forget(ws.id);
        self.session.workspaces.remove(idx);
        let shown = &mut self.view.active_workspace;
        let remaining = self.session.workspaces.len();
        if remaining == 0 {
            // Session emptied — keep the index in a benign state (`get` → `None`).
            *shown = 0;
        } else if *shown > idx {
            // The active workspace was after the removed one; shift down.
            *shown -= 1;
        } else if *shown >= remaining {
            // The removed workspace was the last one; clamp to the new last.
            *shown = remaining - 1;
        }
        true
    }

    /// Switch to a workspace by index with animation.
    pub fn switch_to_workspace(&mut self, idx: usize) {
        if idx >= self.session.workspaces.len() || idx == self.view.active_workspace {
            return;
        }

        let from_idx = self.view.active_workspace;
        self.view.switch = WorkspaceSwitch::Animation {
            from_idx,
            to_idx: idx,
            progress: Animated::Animating {
                animation: Animation::new(0.0, 1.0, AnimationConfig::default()),
                from: 0.0,
                to: 1.0,
            },
        };
        self.view.active_workspace = idx;
    }

    /// Switch workspace up (to previous workspace).
    pub fn switch_workspace_up(&mut self) -> bool {
        if self.view.active_workspace == 0 {
            return false;
        }
        self.switch_to_workspace(self.view.active_workspace - 1);
        true
    }

    /// Switch workspace down (to next workspace).
    pub fn switch_workspace_down(&mut self) -> bool {
        let next = self.view.active_workspace + 1;
        if next >= self.session.workspaces.len() {
            // Create a new empty workspace if at the end.
            self.session.add_workspace();
        }
        self.switch_to_workspace(next);
        true
    }

    /// Advance all animations in the window's layout.
    pub fn advance_animations(&mut self) {
        // Advance workspace switch animation.
        match &mut self.view.switch {
            WorkspaceSwitch::Animation {
                progress: Animated::Animating { animation, .. },
                ..
            } if animation.is_done() => {
                self.view.switch = WorkspaceSwitch::None;
            }
            WorkspaceSwitch::Gesture { .. } => {
                // Gestures are driven by input events.
            }
            _ => {}
        }

        // Advance workspace animations.
        for ws in &mut self.session.workspaces {
            let scroll = self.view.scroll_mut(ws.id);
            ws.through_mut(scroll).advance_animations();
        }
    }

    /// Add a pane to the active workspace.
    pub fn add_pane(&mut self, pane: Pane, column_idx: Option<usize>, activate: bool) {
        // Allocated here, before the workspace is borrowed, and spent only if a column is created.
        let new_column_id = ColumnId(self.session.next_id());
        if let Some(mut ws) = self.active_workspace_mut() {
            ws.add_pane(pane, column_idx, activate, new_column_id);
        }
    }

    /// Focus left in the active workspace.
    pub fn focus_left(&mut self) -> bool {
        self.active_workspace_mut()
            .is_some_and(|mut ws| ws.focus_left())
    }

    /// Focus right in the active workspace.
    pub fn focus_right(&mut self) -> bool {
        self.active_workspace_mut()
            .is_some_and(|mut ws| ws.focus_right())
    }

    /// Focus up (previous pane or workspace).
    pub fn focus_up(&mut self) -> bool {
        if self.active_workspace_mut().is_some_and(|mut ws| ws.focus_up()) {
            return true;
        }
        self.switch_workspace_up()
    }

    /// Focus down (next pane or workspace).
    pub fn focus_down(&mut self) -> bool {
        if self
            .active_workspace_mut()
            .is_some_and(|mut ws| ws.focus_down())
        {
            return true;
        }
        self.switch_workspace_down()
    }

    /// The window was resized (e.g., on window resize): every workspace is laid out in the new
    /// size.
    pub fn update_viewport(&mut self, size: Size) {
        self.view.set_viewport(size);
        let working_area = Rectangle::new(Point::default(), size);
        for ws in &mut self.session.workspaces {
            let scroll = self.view.scroll_mut(ws.id);
            ws.through_mut(scroll).update_working_area(working_area);
        }
    }
}

mod moves;
#[cfg(test)]
mod move_tests;
mod swap;
pub use moves::{Added, Moved};
#[cfg(test)]
mod tests;
