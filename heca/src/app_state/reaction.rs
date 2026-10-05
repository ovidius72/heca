//! **What a window does about what the server says changed** — over the window's own parts only,
//! so it runs, and is tested, with a session and a view and no window.
//!
//! The server reports past facts about the content ([`Change`]). Which workspace is shown, which
//! column is active and where the window came from are the window's own decisions, made here once.

use heca_core::layout::LayoutMut;

use crate::server::Change;

/// Show workspace `idx`, remembering the one the window leaves so it can toggle back. The single
/// way a window changes workspace; it does not touch the per-workspace pane history, which belongs
/// to the same-workspace pane toggle.
pub(crate) fn show_workspace(
    layout: &mut LayoutMut<'_>,
    last_visited_ws: &mut Option<usize>,
    idx: usize,
) {
    let current = layout.reader().active_workspace_idx();
    if current == idx {
        return;
    }
    *last_visited_ws = Some(current);
    layout.switch_to_workspace(idx);
}

/// Do what a window does about one fact about the layout. Returns whether the window has to
/// redraw, which every layout fact asks for; a fact that is not about the layout is not its
/// business and asks for nothing.
pub(crate) fn window_reacts(
    layout: &mut LayoutMut<'_>,
    last_visited_ws: &mut Option<usize>,
    change: &Change,
) -> bool {
    match *change {
        Change::LayoutChanged => true,
        Change::ColumnZoomed { workspace, column } => {
            // The window shows what it zoomed: the workspace, and the column, which also brings
            // it into view.
            show_workspace(layout, last_visited_ws, workspace);
            if let Some(mut ws) = layout.workspace_mut(workspace) {
                ws.scroll_mut().activate_column(column);
            }
            true
        }
        Change::NotificationsChanged => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::testing::Windowed;
    use heca_core::layout::{Pane, PaneId, Size};

    fn two_workspaces_of_two_columns() -> Windowed {
        let mut window = Windowed::new(Size::new(1000.0, 800.0), 1.0);
        window.m().add_workspace();
        for idx in 0..2 {
            window.show(idx);
            for n in 0..2u64 {
                let id = PaneId(10 * idx as u64 + n + 1);
                window.m().add_pane(Pane::new(id, "p"), None, true);
            }
        }
        window.show(0);
        window
    }

    /// A column zoomed in another workspace is shown: the window goes there, remembers where it
    /// was, and has that column active.
    #[test]
    fn a_zoom_elsewhere_shows_that_workspace_and_column() {
        let mut window = two_workspaces_of_two_columns();
        let mut last_visited = None;
        let redraw = window_reacts(
            &mut window.m(),
            &mut last_visited,
            &Change::ColumnZoomed { workspace: 1, column: 0 },
        );
        assert!(redraw);
        assert_eq!(window.l().active_workspace_idx(), 1);
        assert_eq!(last_visited, Some(0));
        let ws = window.l().active_workspace().expect("a workspace");
        assert_eq!(ws.scroll().active_column_idx(), 0);
    }

    /// Zooming in the workspace already shown does not make it its own "previous" workspace.
    #[test]
    fn a_zoom_here_does_not_record_a_visit() {
        let mut window = two_workspaces_of_two_columns();
        let mut last_visited = None;
        window_reacts(
            &mut window.m(),
            &mut last_visited,
            &Change::ColumnZoomed { workspace: 0, column: 0 },
        );
        assert_eq!(last_visited, None);
    }

    /// A fact that is not about the layout asks for no redraw of it.
    #[test]
    fn a_notification_is_not_the_layouts_business() {
        let mut window = two_workspaces_of_two_columns();
        let mut last_visited = None;
        assert!(!window_reacts(&mut window.m(), &mut last_visited, &Change::NotificationsChanged));
    }
}
