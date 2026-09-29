//! **What the session is made of** — which workspaces, columns and panes exist, and in what order.

use super::session::Session;
use super::types::{ColumnId, PaneId, WorkspaceId};

/// The session's structure, and nothing else: which workspaces, columns and panes exist, in order,
/// and which panes float.
///
/// Two shapes are equal when nothing appeared, went or moved. Focus, sizes, names and scroll
/// position do not count — they change a picture, not the list of things in it.
///
/// Compared before and after an action, it answers "did the structure change?" for whatever lists
/// the session (the exposé, the workspaces tree) without every action having to say so — an action
/// that forgot would leave those lists stale, and nothing would notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionShape(Vec<WorkspaceShape>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceShape {
    id: WorkspaceId,
    columns: Vec<(ColumnId, Vec<PaneId>)>,
    floating: Vec<PaneId>,
}

impl Session {
    /// The session's current [`SessionShape`].
    pub fn shape(&self) -> SessionShape {
        SessionShape(
            self.workspaces
                .iter()
                .map(|ws| WorkspaceShape {
                    id: ws.id,
                    columns: ws
                        .scrolling
                        .columns
                        .iter()
                        .map(|col| (col.id, col.panes.iter().map(|p| p.id).collect()))
                        .collect(),
                    floating: ws.floating_panes.iter().map(|f| f.pane.id).collect(),
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::layout::types::{LayoutOptions, SessionId, Size};
    use crate::layout::{Pane, PaneId, Session};

    /// One workspace (the one `Session::new` makes) holding `n` panes, each in its own column.
    fn session_with_panes(n: u64) -> Session {
        let size = Size::new(1000.0, 800.0);
        let mut s = Session::new(SessionId(1), size, 1.0, LayoutOptions::default());
        for id in 0..n {
            s.add_pane(Pane::new(PaneId(100 + id), ""), None, true);
        }
        s
    }

    #[test]
    fn focus_moving_is_not_a_change_of_shape() {
        let mut s = session_with_panes(2);
        let before = s.shape();
        s.focus_left();
        assert_eq!(s.shape(), before);
    }

    #[test]
    fn a_pane_appearing_is_a_change_of_shape() {
        let mut s = session_with_panes(1);
        let before = s.shape();
        s.add_pane(Pane::new(PaneId(7), ""), None, true);
        assert_ne!(s.shape(), before);
    }

    #[test]
    fn two_columns_trading_places_is_a_change_of_shape() {
        let mut s = session_with_panes(2);
        let before = s.shape();
        s.workspaces[0].scrolling.columns.swap(0, 1);
        assert_ne!(s.shape(), before);
    }
}
