//! **What a window does about what the server says changed** — over the window's own parts only,
//! so it runs, and is tested, with a session and a view and no window.
//!
//! The server reports past facts about the content ([`Change`]). Which workspace is shown, which
//! column is active and where the window came from are the window's own decisions, made here once.

use heca_core::layout::{LayoutMut, PaneId, Positions};

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
    /// Start a shell in this pane of this workspace. The terminals are still the window's, so a
    /// pane the server made is given its shell here; it goes when they move behind the server.
    pub(crate) start_shell: Option<(PaneId, usize)>,
    /// End the terminal in this pane, which is gone. Like the shell start, the terminals are
    /// still the window's, so this goes when they move behind the server.
    pub(crate) stop_terminals: Vec<PaneId>,
    /// Names changed: whatever shows them is out of date.
    pub(crate) names_changed: bool,
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
    before: Option<&(usize, Positions)>,
) -> Reaction {
    let mut reaction = Reaction { redraw: true, ..Reaction::default() };
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
        Change::ColumnsChanged { workspace, effect } => {
            // Show the change the way this window sees it — only for the workspace it was looking
            // at when it asked, where it took the picture of where things were.
            if let Some((asked_in, positions)) = before
                && *asked_in == workspace
            {
                layout.show_column_change(workspace, effect, positions);
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
        Change::PaneAdded { pane, workspace, .. } => {
            if asked.is_some() {
                show_workspace(layout, tracking.last_visited_ws, workspace);
            }
            reaction.start_shell = Some((pane, workspace));
        }
        Change::NamesChanged => reaction.names_changed = true,
        Change::PaneRemoved { pane } => reaction.stop_terminals.push(pane),
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
mod tests;
