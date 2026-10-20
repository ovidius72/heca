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

/// The index of the workspace a view shows, resolved from the id it holds — current even after a
/// workspace before it was removed. A view not yet pointed at one shows the first.
fn shown_idx(session: &Session, view: &WindowView) -> usize {
    view.active_workspace
        .and_then(|id| session.index_of(id))
        .unwrap_or(0)
}

impl<'a> Layout<'a> {
    /// Where everything is in every workspace for this window, to hold against where it is after a
    /// change ([`LayoutMut::show_change`]).
    pub fn before(&self) -> Before {
        Before(
            self.session
                .workspaces
                .iter()
                .map(|ws| {
                    (
                        ws.id,
                        ws.scrolling.through(self.view.scroll(ws.id)).positions(),
                    )
                })
                .collect(),
        )
    }

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

    /// Index of the workspace the window shows — resolved from its id, so it is always current
    /// even after another window removed a workspace before it. Falls back to the first.
    pub fn active_workspace_idx(&self) -> usize {
        shown_idx(self.session, self.view)
    }

    /// The workspace at `idx`, as the window sees it.
    pub fn workspace(&self, idx: usize) -> Option<WorkspaceRef<'a>> {
        let ws = self.session.workspaces.get(idx)?;
        Some(ws.through(self.view.scroll(ws.id)))
    }

    /// The workspace the window shows.
    pub fn active_workspace(&self) -> Option<WorkspaceRef<'a>> {
        self.workspace(self.active_workspace_idx())
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
    /// one workspace is on screen at a time.
    pub fn workspace_geometries(&self) -> Vec<(usize, Rectangle)> {
        if self.active_workspace().is_some() {
            vec![(
                self.active_workspace_idx(),
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
    /// Workspace `idx`'s columns, to change **with no view**: what a server does. Show the change
    /// to a window with [`show_change`](Self::show_change).
    pub fn columns_mut(&mut self, idx: usize) -> Option<&mut super::ScrollingSpace> {
        self.session
            .workspaces
            .get_mut(idx)
            .map(|ws| &mut ws.scrolling)
    }

    /// **Show a change to workspace `idx`'s columns and panes in this window**: the window's own
    /// reaction to what the content did, given where things were before ([`Layout::before`]). A
    /// workspace the picture does not hold — one made by the change — has nothing to show.
    pub fn show_change(
        &mut self,
        idx: usize,
        effects: &[super::scrolling::SpaceEffect],
        before: &Before,
    ) {
        let Some(ws) = self.session.workspaces.get(idx) else {
            return;
        };
        if let Some(positions) = before.of(ws.id) {
            self.view
                .scroll_mut(ws.id)
                .react_to(&ws.scrolling, effects, &positions);
        }
    }

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
        let idx = shown_idx(self.session, self.view);
        self.workspace_mut(idx)
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

    /// **Workspace `idx` is gone from the session** ([`Session::remove_workspace`]): this window
    /// forgets how it looked at it, and keeps showing the workspace it showed — numbered one lower
    /// when it was after the one that went. A window showing the one that went shows the one now in
    /// its place, or the new last one. With no workspace left it shows nothing until one is made.
    pub fn workspace_removed(&mut self, idx: usize) {
        let session = &*self.session;
        self.view.forget_all_but(|id| session.index_of(id).is_some());
        // The shown workspace is an id: if it still exists, its index shifts by itself and there is
        // nothing to do. Only when the shown workspace is the one that went does the window move —
        // to the workspace now in its place, or the last, or nothing when none are left.
        let shown_gone = self
            .view
            .active_workspace
            .is_some_and(|id| session.index_of(id).is_none());
        if shown_gone {
            let len = session.workspaces.len();
            self.view.active_workspace = (len > 0).then(|| session.workspaces[idx.min(len - 1)].id);
        }
    }

    /// Switch to a workspace by index with animation.
    pub fn switch_to_workspace(&mut self, idx: usize) {
        let from_idx = shown_idx(self.session, self.view);
        if idx >= self.session.workspaces.len() || idx == from_idx {
            return;
        }

        self.view.switch = WorkspaceSwitch::Animation {
            from_idx,
            to_idx: idx,
            progress: Animated::Animating {
                animation: Animation::new(0.0, 1.0, AnimationConfig::default()),
                from: 0.0,
                to: 1.0,
            },
        };
        self.view.active_workspace = Some(self.session.workspaces[idx].id);
    }

    /// Switch workspace up (to previous workspace).
    pub fn switch_workspace_up(&mut self) -> bool {
        let cur = shown_idx(self.session, self.view);
        if cur == 0 {
            return false;
        }
        self.switch_to_workspace(cur - 1);
        true
    }

    /// Switch workspace down (to next workspace).
    pub fn switch_workspace_down(&mut self) -> bool {
        let next = shown_idx(self.session, self.view) + 1;
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
        if self
            .active_workspace_mut()
            .is_some_and(|mut ws| ws.focus_up())
        {
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

    /// Reconcile this window's floating view in **every** workspace with the content: a float that
    /// went (a pane closed, unfloated or moved away) is forgotten, and the window leaves the
    /// floating domain in a workspace that has no float left. Called after a change that can remove
    /// a float; idempotent and cheap, so it need not know which workspace held the float.
    pub fn reconcile_floats(&mut self) {
        for idx in 0..self.session.workspaces.len() {
            if let Some(mut ws) = self.workspace_mut(idx) {
                ws.reconcile_floats();
            }
        }
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

/// Where everything was in each workspace for one window, taken before a change, so the window
/// can show the change from where it was ([`LayoutMut::show_change`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Before(Vec<(WorkspaceId, super::scrolling::Positions)>);

impl Before {
    /// Where things were in workspace `id`, with the panes of every other workspace as they would
    /// sit in it — so a pane that came from another workspace slides in from where it was. `None`
    /// when the workspace did not exist yet.
    pub fn of(&self, id: WorkspaceId) -> Option<super::scrolling::Positions> {
        let mut positions = self
            .0
            .iter()
            .find_map(|(ws, positions)| (*ws == id).then_some(positions))?
            .clone();
        for (_, others) in self.0.iter().filter(|(ws, _)| *ws != id) {
            positions.add_panes_from(others);
        }
        Some(positions)
    }
}
#[cfg(test)]
mod tests;
