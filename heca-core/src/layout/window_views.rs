//! Session-specific layout views kept by one window, independently of other windows.

use std::{collections::HashMap, ops::{Deref, DerefMut}};
use super::{Session, SessionId, WindowView, ColumnId, WorkspaceId};

/// The active layout view and this window's remembered views of other sessions.
/// Switching content must not turn a return visit into a new window.
#[derive(Debug, Clone)]
pub struct WindowViews {
    current: WindowView,
    saved: HashMap<SessionId, SavedView>,
}

#[derive(Debug, Clone)]
struct SavedView {
    view: WindowView,
    columns: HashMap<WorkspaceId, ColumnId>,
}

impl SavedView {
    fn capture(session: &Session, view: WindowView) -> Self {
        let columns = session.workspaces.iter().filter_map(|ws| ws.scrolling.columns
            .get(view.scroll(ws.id).active_column).map(|column| (ws.id, column.id))).collect();
        Self { view, columns }
    }

    fn reconcile(&mut self, session: &Session) {
        self.view.forget_all_but(|id| session.index_of(id).is_some());
        for ws in &session.workspaces {
            let scroll = self.view.scroll_mut(ws.id);
            scroll.active_column = self.columns.get(&ws.id)
                .and_then(|id| ws.scrolling.columns.iter().position(|column| column.id == *id))
                .unwrap_or_else(|| scroll.active_column.min(ws.scrolling.columns.len().saturating_sub(1)));
            scroll.retain_columns(|id| ws.scrolling.columns.iter().any(|column| column.id == id));
        }
    }
}

impl WindowViews {
    /// Start with the view already used by this window.
    pub fn new(view: WindowView) -> Self {
        Self { current: view, saved: HashMap::new() }
    }

    /// Select content, restoring this window's view at its current size and display scale.
    /// Pane/workspace identities are resolved against the current content, not old row indices.
    pub fn replace_session(&mut self, session: &mut Session, replacement: Session, scale: f64) {
        let viewport = self.current.viewport();
        let previous = SavedView::capture(session, std::mem::replace(&mut self.current, WindowView::new(viewport, scale)));
        let mut next = if session.id == replacement.id { previous } else {
            self.saved.insert(session.id, previous);
            self.saved.remove(&replacement.id).unwrap_or_else(|| SavedView::capture(&replacement, self.current.clone()))
        };
        *session = replacement;
        next.reconcile(session);
        let resized = next.view.viewport() != viewport;
        next.view.update_scale(scale);
        self.current = next.view;
        let mut layout = session.through_mut(&mut self.current);
        if resized { layout.update_viewport(viewport); }
        layout.reconcile_floats();
    }

    /// Forget cached views of sessions removed from the server's directory.
    pub fn retain_sessions(&mut self, keep: impl Fn(SessionId) -> bool) {
        self.saved.retain(|id, _| keep(*id));
    }
}

impl Deref for WindowViews {
    type Target = WindowView;
    fn deref(&self) -> &WindowView { &self.current }
}

impl DerefMut for WindowViews {
    fn deref_mut(&mut self) -> &mut WindowView { &mut self.current }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Pane, PaneId, Size};

    fn session(id: u64) -> Session {
        let mut session = Session::new(SessionId(id), Default::default());
        let mut view = WindowView::new(Size::new(900.0, 600.0), 1.0);
        for id in 1..=3 { session.through_mut(&mut view).add_pane(Pane::new(PaneId(id), "shell"), None, true); }
        session
    }

    fn focused(session: &Session, views: &WindowViews) -> PaneId {
        session.through(views).active_workspace().unwrap().active_pane().unwrap().id
    }

    #[test]
    fn returning_to_a_session_restores_this_windows_pane_and_scroll_not_another_windows() {
        let (mut a, b) = (session(1), session(2));
        let mut views = WindowViews::new(WindowView::new(Size::new(900.0, 600.0), 1.0));
        let original_a = a.clone();
        let other = WindowView::new(Size::new(900.0, 600.0), 1.0);
        a.through_mut(&mut views).active_workspace_mut().unwrap().activate_pane(PaneId(3));
        let before = a.through(&views).active_workspace().unwrap().scroll().panes_with_positions();
        views.replace_session(&mut a, b.clone(), 1.0);
        a.through_mut(&mut views).active_workspace_mut().unwrap().activate_pane(PaneId(2));
        views.replace_session(&mut a, original_a, 1.0);
        assert_eq!(focused(&a, &views), PaneId(3));
        assert_eq!(a.through(&views).active_workspace().unwrap().scroll().panes_with_positions(), before);
        assert_eq!(a.through(&other).active_workspace().unwrap().active_pane().unwrap().id, PaneId(1));
        views.replace_session(&mut a, b, 1.0);
        assert_eq!(focused(&a, &views), PaneId(2));
    }

    #[test]
    fn returning_after_a_resize_uses_the_current_window_size_and_scale() {
        let (mut a, b) = (session(1), session(2));
        let mut views = WindowViews::new(WindowView::new(Size::new(900.0, 600.0), 1.0));
        let original_a = a.clone();
        a.through_mut(&mut views).active_workspace_mut().unwrap().activate_pane(PaneId(3));
        views.replace_session(&mut a, b, 1.0);
        let size = Size::new(1200.0, 800.0);
        a.through_mut(&mut views).update_viewport(size);
        views.replace_session(&mut a, original_a, 2.0);
        assert_eq!(focused(&a, &views), PaneId(3));
        assert_eq!(a.through(&views).viewport(), size);
        assert_eq!(a.through(&views).scale(), 2.0);
    }

    #[test]
    fn retained_views_resolve_removed_panes_and_forget_deleted_sessions() {
        let (mut a, b) = (session(1), session(2));
        let mut views = WindowViews::new(WindowView::new(Size::new(900.0, 600.0), 1.0));
        let mut original_a = a.clone();
        a.through_mut(&mut views).active_workspace_mut().unwrap().activate_pane(PaneId(3));
        views.replace_session(&mut a, b.clone(), 1.0);
        original_a.remove_pane(PaneId(3)).unwrap();
        views.replace_session(&mut a, original_a, 1.0);
        assert_ne!(focused(&a, &views), PaneId(3));
        assert!(a.workspace_holding(focused(&a, &views)).is_some());
        views.replace_session(&mut a, b.clone(), 1.0);
        views.retain_sessions(|id| id == b.id);
        assert!(views.saved.is_empty());
    }

    #[test]
    fn column_insertions_while_away_do_not_change_the_remembered_pane() {
        let (mut a, b) = (session(1), session(2));
        let mut views = WindowViews::new(WindowView::new(Size::new(900.0, 600.0), 1.0));
        let mut original_a = a.clone();
        a.through_mut(&mut views).active_workspace_mut().unwrap().activate_pane(PaneId(3));
        views.replace_session(&mut a, b, 1.0);
        original_a.add_column_with(0, 0, Pane::new(PaneId(99), "new"));
        views.replace_session(&mut a, original_a, 1.0);
        assert_eq!(focused(&a, &views), PaneId(3));
    }

    #[test]
    fn the_workspace_selection_is_remembered_separately_for_each_session() {
        let (mut a, b) = (session(1), session(2));
        a.add_workspace();
        a.add_column_with(1, 0, Pane::new(PaneId(99), "other workspace"));
        let original_a = a.clone();
        let mut views = WindowViews::new(WindowView::new(Size::new(900.0, 600.0), 1.0));
        a.through_mut(&mut views).switch_to_workspace(1);
        views.replace_session(&mut a, b, 1.0);
        assert_eq!(a.through(&views).active_workspace_idx(), 0);
        views.replace_session(&mut a, original_a, 1.0);
        assert_eq!(a.through(&views).active_workspace_idx(), 1);
        assert_eq!(focused(&a, &views), PaneId(99));
    }
}
