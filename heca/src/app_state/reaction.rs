//! **What a window does about what the server says changed** — over the window's own parts only,
//! so it runs, and is tested, with a session and a view and no window.
//!
//! The server reports past facts about the content ([`Change`]). Which workspace is shown, which
//! column is active and where the window came from are the window's own decisions, made here once.

use heca_core::layout::{LayoutMut, PaneId};

use crate::input::WmAction;
use crate::server::{Change, Refusal};

/// What a window remembers about where it has been, by workspace — the parts of its state a
/// fact about the layout can make stale.
pub(crate) struct Tracking<'a> {
    /// The workspace shown before this one.
    pub(crate) last_visited_ws: &'a mut Option<usize>,
    /// The pane focused before this one, per workspace.
    pub(crate) last_visited_pane_per_ws: &'a mut Vec<Option<PaneId>>,
    /// Where the exposé's cursor was, per workspace.
    pub(crate) expose_cursor_per_ws: &'a mut Vec<Option<PaneId>>,
}

impl Tracking<'_> {
    /// Forget workspace `idx`, which is gone: what pointed at it points at nothing, and what
    /// pointed past it moves down one.
    pub(crate) fn forget_workspace(&mut self, idx: usize) {
        match *self.last_visited_ws {
            Some(i) if i == idx => *self.last_visited_ws = None,
            Some(i) if i > idx => *self.last_visited_ws = Some(i - 1),
            _ => {}
        }
        if idx < self.last_visited_pane_per_ws.len() {
            self.last_visited_pane_per_ws.remove(idx);
        }
        if idx < self.expose_cursor_per_ws.len() {
            self.expose_cursor_per_ws.remove(idx);
        }
    }
}

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

/// What is left for the rest of the window to do after a fact.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Reaction {
    /// Draw again.
    pub(crate) redraw: bool,
    /// Say, in the status bar, why the action asked for did nothing.
    pub(crate) refusal: Option<Refusal>,
}

/// The words for why the server did nothing, to follow the action's own label in the status bar.
pub(crate) fn refusal_words(reason: Refusal) -> &'static str {
    match reason {
        Refusal::OnlyPaneInColumn => "it is already the only pane in its column",
    }
}

/// Whether the window that sent `asked` goes where the column it moved went. A column moved with
/// `focus` off stays where it was shown; every other move is followed.
fn follows_column(asked: Option<&WmAction>) -> bool {
    !matches!(
        asked,
        Some(
            WmAction::MoveColumnToWorkspace { focus: false, .. }
                | WmAction::MoveColumn { focus: false, .. }
        )
    )
}

/// The pane the action this window sent names as the one it acts on, when it names one.
fn named_pane(asked: Option<&WmAction>) -> Option<PaneId> {
    match asked {
        Some(WmAction::MovePaneLeft { pane_id } | WmAction::MovePaneRight { pane_id }) => *pane_id,
        _ => None,
    }
}

/// Do what a window does about one fact about the layout, `asked` being the action this window
/// sent (none when the fact is not an answer to one). Another window that gets the same fact
/// passes none of its own and follows nothing.
pub(crate) fn window_reacts(
    layout: &mut LayoutMut<'_>,
    tracking: &mut Tracking<'_>,
    change: &Change,
    asked: Option<&WmAction>,
) -> Reaction {
    let mut reaction = Reaction { redraw: true, refusal: None };
    match *change {
        Change::LayoutChanged => {
            // An action that named a pane focuses it even when nothing moved.
            if let Some(pane) = named_pane(asked)
                && let Some((workspace, ..)) = layout.reader().session().pane_location(pane)
            {
                show_workspace(layout, tracking.last_visited_ws, workspace);
                if let Some(mut ws) = layout.workspace_mut(workspace) {
                    ws.activate_pane(pane);
                }
            }
        }
        Change::ColumnZoomed { workspace, column } => {
            // The window shows what it zoomed: the workspace, and the column, which also brings
            // it into view.
            show_workspace(layout, tracking.last_visited_ws, workspace);
            if let Some(mut ws) = layout.workspace_mut(workspace) {
                ws.scroll_mut().activate_column(column);
            }
        }
        Change::PaneMoved { workspace, .. } => {
            // A pane moved by this window is shown where it landed.
            if asked.is_some() {
                show_workspace(layout, tracking.last_visited_ws, workspace);
            }
        }
        Change::ColumnMoved { workspace, .. } => {
            if asked.is_some() && follows_column(asked) {
                show_workspace(layout, tracking.last_visited_ws, workspace);
            }
        }
        Change::WorkspaceRemoved { index } => tracking.forget_workspace(index),
        Change::Refused(reason) => reaction.refusal = Some(reason),
        Change::NotificationsChanged => reaction.redraw = false,
    }
    reaction
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::testing::Windowed;
    use heca_core::layout::{Pane, PaneId, Size};

    /// What a window remembers, owned, for a test to hold.
    #[derive(Default)]
    struct Tracked {
        last_visited_ws: Option<usize>,
        last_visited_pane_per_ws: Vec<Option<PaneId>>,
        expose_cursor_per_ws: Vec<Option<PaneId>>,
    }

    impl Tracked {
        fn parts(&mut self) -> Tracking<'_> {
            Tracking {
                last_visited_ws: &mut self.last_visited_ws,
                last_visited_pane_per_ws: &mut self.last_visited_pane_per_ws,
                expose_cursor_per_ws: &mut self.expose_cursor_per_ws,
            }
        }
    }

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
        let mut tracked = Tracked::default();
        let reaction = window_reacts(
            &mut window.m(),
            &mut tracked.parts(),
            &Change::ColumnZoomed { workspace: 1, column: 0 },
            None,
        );
        assert!(reaction.redraw);
        assert_eq!(window.l().active_workspace_idx(), 1);
        assert_eq!(tracked.last_visited_ws, Some(0));
        let ws = window.l().active_workspace().expect("a workspace");
        assert_eq!(ws.scroll().active_column_idx(), 0);
    }

    /// Zooming in the workspace already shown does not make it its own "previous" workspace.
    #[test]
    fn a_zoom_here_does_not_record_a_visit() {
        let mut window = two_workspaces_of_two_columns();
        let mut tracked = Tracked::default();
        window_reacts(
            &mut window.m(),
            &mut tracked.parts(),
            &Change::ColumnZoomed { workspace: 0, column: 0 },
            None,
        );
        assert_eq!(tracked.last_visited_ws, None);
    }

    /// A fact that is not about the layout asks for no redraw of it.
    #[test]
    fn a_notification_is_not_the_layouts_business() {
        let mut window = two_workspaces_of_two_columns();
        let mut tracked = Tracked::default();
        let reaction =
            window_reacts(&mut window.m(), &mut tracked.parts(), &Change::NotificationsChanged, None);
        assert!(!reaction.redraw);
    }

    fn react(
        window: &mut Windowed,
        tracked: &mut Tracked,
        change: Change,
        asked: Option<&WmAction>,
    ) -> Reaction {
        window_reacts(&mut window.m(), &mut tracked.parts(), &change, asked)
    }

    fn landed_in_workspace_1() -> Change {
        Change::PaneMoved { pane: PaneId(1), workspace: 1, column: 0 }
    }

    /// The window that moved a pane is taken to where it landed; one that only hears of it is not.
    #[test]
    fn only_the_asking_window_follows_a_moved_pane() {
        let ask = WmAction::MovePaneToWorkspace { pane_id: PaneId(1), ws_idx: 1 };
        let mut window = two_workspaces_of_two_columns();
        let mut tracked = Tracked::default();
        react(&mut window, &mut tracked, landed_in_workspace_1(), None);
        assert_eq!(window.l().active_workspace_idx(), 0);
        react(&mut window, &mut tracked, landed_in_workspace_1(), Some(&ask));
        assert_eq!(window.l().active_workspace_idx(), 1);
        assert_eq!(tracked.last_visited_ws, Some(0));
    }

    /// A column moved with `focus` off stays out of sight; with it on, it is followed.
    #[test]
    fn a_moved_column_is_followed_only_when_asked_to() {
        let landed = Change::ColumnMoved { workspace: 1, column: 0 };
        let quiet = WmAction::MoveColumnToWorkspace { col_idx: 0, ws_idx: 1, focus: false };
        let loud = WmAction::MoveColumnToWorkspace { col_idx: 0, ws_idx: 1, focus: true };
        let mut window = two_workspaces_of_two_columns();
        let mut tracked = Tracked::default();
        react(&mut window, &mut tracked, landed, Some(&quiet));
        assert_eq!(window.l().active_workspace_idx(), 0);
        react(&mut window, &mut tracked, landed, Some(&loud));
        assert_eq!(window.l().active_workspace_idx(), 1);
    }

    /// A workspace that is gone is forgotten: what pointed at it points at nothing, and what
    /// pointed past it moves down one.
    #[test]
    fn a_removed_workspace_is_forgotten_by_the_window() {
        let mut window = two_workspaces_of_two_columns();
        let mut tracked = Tracked {
            last_visited_ws: Some(2),
            last_visited_pane_per_ws: vec![Some(PaneId(1)), Some(PaneId(2)), Some(PaneId(3))],
            expose_cursor_per_ws: vec![None, Some(PaneId(2)), None],
        };
        react(&mut window, &mut tracked, Change::WorkspaceRemoved { index: 1 }, None);
        assert_eq!(tracked.last_visited_ws, Some(1));
        assert_eq!(tracked.last_visited_pane_per_ws, [Some(PaneId(1)), Some(PaneId(3))]);
        assert_eq!(tracked.expose_cursor_per_ws, [None, None]);
        react(&mut window, &mut tracked, Change::WorkspaceRemoved { index: 2 }, None);
        assert_eq!(tracked.last_visited_ws, Some(1), "an earlier one is untouched");
        let mut tracked = Tracked { last_visited_ws: Some(1), ..Tracked::default() };
        react(&mut window, &mut tracked, Change::WorkspaceRemoved { index: 1 }, None);
        assert_eq!(tracked.last_visited_ws, None, "the one it pointed at is gone");
    }

    /// A refusal is handed on to be said, and changes nothing the window shows.
    #[test]
    fn a_refusal_is_handed_on_to_be_said() {
        let mut window = two_workspaces_of_two_columns();
        let mut tracked = Tracked::default();
        let reaction = react(
            &mut window,
            &mut tracked,
            Change::Refused(Refusal::OnlyPaneInColumn),
            Some(&WmAction::MovePaneToNewColumn),
        );
        assert_eq!(reaction.refusal, Some(Refusal::OnlyPaneInColumn));
        assert_eq!(window.l().active_workspace_idx(), 0);
        assert!(!refusal_words(Refusal::OnlyPaneInColumn).is_empty());
    }

    /// A move that changed nothing still takes the asking window to the pane it named.
    #[test]
    fn a_named_pane_is_focused_even_when_it_did_not_move() {
        let ask = WmAction::MovePaneLeft { pane_id: Some(PaneId(11)) };
        let mut window = two_workspaces_of_two_columns();
        let mut tracked = Tracked::default();
        react(&mut window, &mut tracked, Change::LayoutChanged, None);
        assert_eq!(window.l().active_workspace_idx(), 0, "another window's change is not followed");
        react(&mut window, &mut tracked, Change::LayoutChanged, Some(&WmAction::MovePaneLeft { pane_id: None }));
        assert_eq!(window.l().active_workspace_idx(), 0, "no pane named, nothing to follow");
        react(&mut window, &mut tracked, Change::LayoutChanged, Some(&ask));
        assert_eq!(window.l().active_workspace_idx(), 1);
        assert_eq!(window.l().workspace(1).map(|ws| ws.scroll().active_column_idx()), Some(0));
    }
}
