//! Shared pane-operation helpers.
//!
//! These helpers collect repeated low-level move/swap mechanics so handlers
//! and mouse paths can delegate to one implementation.

use heca_core::layout::animation::AnimationConfig;
use heca_core::layout::workspace::Workspace;
use heca_core::layout::types::{Point, Rectangle};
use heca_core::layout::{Column, ColumnId, ColumnWidth, Pane};
use heca_core::layout::types::PaneInsertTarget;

/// Information about a pane removed from a workspace.
#[derive(Debug)]
pub(crate) struct RemovedPane {
    pub pane: Pane,
}

/// Extended removal info including column context.
///
/// Used by cross-workspace swaps that need to know whether the removal
/// deleted the column and what the column ID was, so they can recreate
/// it at the target position.
#[derive(Debug)]
pub(crate) struct RemovedPaneInfo {
    pub pane: Pane,
    /// True if the removed pane was the only pane in its column,
    /// meaning the column was deleted by the removal.
    pub was_only_in_column: bool,
    /// The ID of the column the pane was in before removal.
    pub column_id: ColumnId,
}

/// Remove a pane from a workspace by pane id.
///
/// Returns basic removal info (just the pane).
pub(crate) fn remove_pane_by_id(ws: &mut Workspace, pane_id: u64) -> Option<RemovedPane> {
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        if let Some(pi) = col.panes.iter().position(|p| p.id.0 == pane_id) {
            let pane = ws.scrolling.remove_pane(ci, pi)?;
            return Some(RemovedPane { pane });
        }
    }
    None
}

/// Remove a pane from a workspace by pane id, returning extended context.
///
/// Returns the removed pane, whether it was the only pane in its column
/// (meaning the column was deleted), and the column's ID before removal.
pub(crate) fn remove_pane_by_id_with_info(
    ws: &mut Workspace,
    pane_id: u64,
) -> Option<RemovedPaneInfo> {
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        if let Some(pi) = col.panes.iter().position(|p| p.id.0 == pane_id) {
            let col_id = col.id;
            let was_only = col.panes.len() == 1;
            let pane = ws.scrolling.remove_pane(ci, pi)?;
            return Some(RemovedPaneInfo {
                pane,
                was_only_in_column: was_only,
                column_id: col_id,
            });
        }
    }
    None
}

/// Insert a pane into a workspace using an existing column target or a new column fallback.
pub(crate) fn insert_pane_at_position(
    ws: &mut Workspace,
    pane: Pane,
    target: PaneInsertTarget,
    new_col_id: ColumnId,
    new_col_width: ColumnWidth,
    activate: bool,
) -> bool {
    match target {
        PaneInsertTarget::NewColumn(col_idx) => {
            let insert_idx = col_idx.min(ws.scrolling.columns.len());
            let col = Column::new(new_col_id, pane, new_col_width);
            ws.scrolling.add_column(Some(insert_idx), col, activate);
            true
        }
        PaneInsertTarget::InColumn { col_idx, pane_idx } => {
            if col_idx < ws.scrolling.columns.len() {
                let insert_idx = pane_idx.min(ws.scrolling.columns[col_idx].panes.len());
                ws.scrolling
                    .add_pane_to_column(col_idx, Some(insert_idx), pane, activate);
            } else {
                let insert_idx = col_idx.min(ws.scrolling.columns.len());
                let col = Column::new(new_col_id, pane, new_col_width);
                ws.scrolling.add_column(Some(insert_idx), col, activate);
            }
            true
        }
    }
}

/// Compute a clamped move offset for animation.
///
/// Given the old and new rectangles, computes `(old - new)` and clamps
/// each component to `max_offset` fraction of the viewport dimensions.
pub(crate) fn clamped_move_offset(
    old_rect: Rectangle,
    new_rect: Rectangle,
    max_dx: f64,
    max_dy: f64,
) -> Point {
    let mut dx = old_rect.loc.x - new_rect.loc.x;
    let mut dy = old_rect.loc.y - new_rect.loc.y;
    if dx > max_dx {
        dx = max_dx;
    } else if dx < -max_dx {
        dx = -max_dx;
    }
    if dy > max_dy {
        dy = max_dy;
    } else if dy < -max_dy {
        dy = -max_dy;
    }
    Point::new(dx, dy)
}

/// Find a pane's (column_index, pane_index) within a workspace by its pane ID.
pub(crate) fn find_pane_indices_in_workspace(
    ws: &Workspace,
    pane_id: u64,
) -> Option<(usize, usize)> {
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        if let Some(pi) = col.panes.iter().position(|p| p.id.0 == pane_id) {
            return Some((ci, pi));
        }
    }
    None
}

/// Swap two panes inside the same column, including the local vertical motion
/// animation and pane-size recomputation.
///
/// Returns `true` when the swap was applied.
pub(crate) fn swap_panes_same_column(
    ws: &mut Workspace,
    col_idx: usize,
    first_pi: usize,
    second_pi: usize,
) -> bool {
    if col_idx >= ws.scrolling.columns.len() {
        return false;
    }
    if first_pi == second_pi {
        return false;
    }

    let col = match ws.scrolling.columns.get_mut(col_idx) {
        Some(col) => col,
        None => return false,
    };
    if first_pi >= col.panes.len() || second_pi >= col.panes.len() {
        return false;
    }

    // After the swap, the pane that was at `first_pi` moves to `second_pi`
    // and vice versa. If the active pane is one of the swapped positions, its
    // new index is the other; otherwise it stays.
    let new_active = if col.active_pane_idx == first_pi {
        second_pi
    } else if col.active_pane_idx == second_pi {
        first_pi
    } else {
        col.active_pane_idx
    };

    let (first_pi, second_pi) = if first_pi < second_pi {
        (first_pi, second_pi)
    } else {
        (second_pi, first_pi)
    };

    let h_above = col.pane_sizes.get(first_pi).map(|s| s.h).unwrap_or(0.0);
    let h_below = col.pane_sizes.get(second_pi).map(|s| s.h).unwrap_or(0.0);
    let gap = ws.scrolling.options.gaps;
    let up_offset = h_above + gap;
    let down_offset = -(h_below + gap);

    col.panes[first_pi].animate_move_y_from(down_offset, AnimationConfig::default());
    col.panes[second_pi].animate_move_y_from(up_offset, AnimationConfig::default());
    col.panes.swap(first_pi, second_pi);
    col.active_pane_idx = new_active;
    col.compute_pane_sizes(ws.scrolling.working_area.size.h, ws.scrolling.options.gaps);
    true
}

/// Swap two panes in different columns of the same workspace.
///
/// Uses a placeholder approach: insert placeholder panes at both target
/// positions, remove the originals, then replace the placeholders with
/// the real panes and animate from old to new positions.
///
/// This avoids column deletion (which would cause a slide animation).
///
/// Callers must pre-generate the two placeholder pane IDs and two
/// column IDs via `session.next_id()` before calling this function.
pub(crate) struct SwapDiffColumnsArgs<'a> {
    pub ws: &'a mut Workspace,
    pub a_id: u64,
    pub b_id: u64,
    pub a_col: usize,
    pub a_pi: usize,
    pub b_col: usize,
    pub b_pi: usize,
    pub placeholder_a_id: u64,
    pub placeholder_b_id: u64,
    pub new_col_for_a: ColumnId,
    pub new_col_for_b: ColumnId,
    pub pane_name_fn: &'a dyn Fn(u64) -> String,
    pub viewport_w: f64,
    pub viewport_h: f64,
}

pub(crate) fn swap_panes_diff_columns(args: SwapDiffColumnsArgs<'_>) {
    let SwapDiffColumnsArgs {
        ws,
        a_id,
        b_id,
        a_col,
        a_pi,
        b_col,
        b_pi,
        placeholder_a_id,
        placeholder_b_id,
        new_col_for_a,
        new_col_for_b,
        pane_name_fn,
        viewport_w,
        viewport_h,
    } = args;
    let a_pid = a_id;
    let b_pid = b_id;

    // Capture old positions (immutable borrow) for animation.
    let old_rects = ws.scrolling.panes_with_positions();
    let old_a_rect = old_rects
        .iter()
        .find(|(pid, _)| *pid == heca_core::layout::PaneId(a_pid))
        .map(|(_, r)| *r);
    let old_b_rect = old_rects
        .iter()
        .find(|(pid, _)| *pid == heca_core::layout::PaneId(b_pid))
        .map(|(_, r)| *r);

    // Insert placeholders in descending column index order to avoid shifting
    // column indices when creating new columns.
    // A should land at B's slot, and B should land at A's slot.
    let mut inserts = vec![
        (b_col, b_pi, placeholder_a_id, new_col_for_a),
        (a_col, a_pi, placeholder_b_id, new_col_for_b),
    ];
    inserts.sort_by_key(|b| std::cmp::Reverse(b.0));

    for (col_pos, pane_idx, ph_pid, new_cid) in inserts {
        if col_pos < ws.scrolling.columns.len() {
            let insert_idx = pane_idx.min(ws.scrolling.columns[col_pos].panes.len());
            let placeholder = Pane::new(heca_core::layout::PaneId(ph_pid), pane_name_fn(ph_pid));
            ws.scrolling
                .add_pane_to_column(col_pos, Some(insert_idx), placeholder, true);
        } else {
            let pos = col_pos.min(ws.scrolling.columns.len());
            let placeholder = Pane::new(heca_core::layout::PaneId(ph_pid), pane_name_fn(ph_pid));
            ws.scrolling.add_column(
                Some(pos),
                Column::new(new_cid, placeholder, ColumnWidth::Proportion(0.5)),
                true,
            );
        }
    }

    // After placeholders exist, find and remove the original panes by id.
    let mut found_a = None;
    let mut found_b = None;
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        for (pi, pane) in col.panes.iter().enumerate() {
            if pane.id == heca_core::layout::PaneId(a_pid) {
                found_a = Some((ci, pi));
            }
            if pane.id == heca_core::layout::PaneId(b_pid) {
                found_b = Some((ci, pi));
            }
        }
    }

    // Remove in descending column index order to avoid index invalidation.
    let mut removes = vec![];
    if let Some((ci, pi)) = found_a {
        removes.push((ci, pi, a_pid));
    }
    if let Some((ci, pi)) = found_b {
        removes.push((ci, pi, b_pid));
    }
    removes.sort_by_key(|y| std::cmp::Reverse(y.0));

    let mut removed_a: Option<Pane> = None;
    let mut removed_b: Option<Pane> = None;

    for (ci, pi, pid) in removes {
        if ci < ws.scrolling.columns.len()
            && let Some(removed) = ws.scrolling.remove_pane(ci, pi)
        {
            if pid == a_pid {
                removed_a = Some(removed);
            } else if pid == b_pid {
                removed_b = Some(removed);
            }
        }
    }

    // Replace placeholders with the removed panes and animate from old positions.
    let max_dx = viewport_w * 0.9;
    let max_dy = viewport_h * 0.9;

    if let Some(a_pane) = removed_a {
        replace_placeholder_and_animate(
            ws,
            placeholder_a_id,
            a_pane,
            old_a_rect,
            max_dx,
            max_dy,
        );
    }
    if let Some(b_pane) = removed_b {
        replace_placeholder_and_animate(
            ws,
            placeholder_b_id,
            b_pane,
            old_b_rect,
            max_dx,
            max_dy,
        );
    }
}

/// Replace a placeholder pane with the real pane, recompute sizes, and
/// animate from the old position to the new one.
fn replace_placeholder_and_animate(
    ws: &mut Workspace,
    placeholder_id: u64,
    new_pane: Pane,
    old_rect: Option<Rectangle>,
    max_dx: f64,
    max_dy: f64,
) {
    let found = find_pane_indices_in_workspace(ws, placeholder_id);
    if let Some((ci, pi)) = found {
        ws.scrolling.columns[ci].panes[pi] = new_pane;
        ws.scrolling.columns[ci].active_pane_idx = pi;
        ws.scrolling.columns[ci].compute_pane_sizes(
            ws.scrolling.working_area.size.h,
            ws.scrolling.options.gaps,
        );
        ws.scrolling.update_all_column_widths();

        // Animate from old position to new.
        if let Some(old_rect) = old_rect {
            let new_rects = ws.scrolling.panes_with_positions();
            let pane_id = ws.scrolling.columns[ci].panes[pi].id;
            if let Some((_, new_rect)) = new_rects.into_iter().find(|(pid, _)| *pid == pane_id) {
                let offset = clamped_move_offset(old_rect, new_rect, max_dx, max_dy);
                ws.scrolling.columns[ci].panes[pi]
                    .animate_move_from(offset, AnimationConfig::default());
            }
        }
    }
}

/// Swap two panes across different workspaces.
///
/// Removes both panes from their workspaces, then inserts each at the
/// other's original position, with animation. If a pane was the only pane
/// in its column, the column is recreated for the incoming pane.
pub(crate) struct SwapCrossWorkspaceArgs<'a> {
    pub session: &'a mut heca_core::layout::session::Session,
    pub a_id: u64,
    pub b_id: u64,
    pub a_ws: usize,
    pub a_col: usize,
    pub a_pi: usize,
    pub b_ws: usize,
    pub b_col: usize,
    pub b_pi: usize,
}

pub(crate) fn swap_panes_cross_workspace(args: SwapCrossWorkspaceArgs<'_>) {
    let SwapCrossWorkspaceArgs {
        session,
        a_id,
        b_id,
        a_ws,
        a_col,
        a_pi,
        b_ws,
        b_col,
        b_pi,
    } = args;
    let vw = session.viewport_size.w;
    let vh = session.viewport_size.h;
    let max_dx = vw * 0.9;
    let max_dy = vh * 0.9;

    // Capture working area info for column recreation.
    let a_wa = session.workspaces.get(a_ws).map(|ws| ws.scrolling.working_area);
    let b_wa = session.workspaces.get(b_ws).map(|ws| ws.scrolling.working_area);
    let a_gaps = session
        .workspaces
        .get(a_ws)
        .map(|ws| ws.scrolling.options.gaps)
        .unwrap_or(0.0);
    let b_gaps = session
        .workspaces
        .get(b_ws)
        .map(|ws| ws.scrolling.options.gaps)
        .unwrap_or(0.0);

    // Capture old pane render positions for animation.
    let old_a_rect = session.workspaces.get(a_ws).and_then(|ws| {
        ws.scrolling
            .panes_with_positions()
            .into_iter()
            .find(|(pid, _)| *pid == heca_core::layout::PaneId(a_id))
            .map(|(_, r)| r)
    });
    let old_b_rect = session.workspaces.get(b_ws).and_then(|ws| {
        ws.scrolling
            .panes_with_positions()
            .into_iter()
            .find(|(pid, _)| *pid == heca_core::layout::PaneId(b_id))
            .map(|(_, r)| r)
    });

    // Step 1: Remove A from its workspace.
    let RemovedPaneInfo {
        pane: removed_a,
        was_only_in_column: a_was_only,
        column_id: a_original_col_id,
    } = {
        let ws = match session.workspaces.get_mut(a_ws) {
            Some(ws) => ws,
            None => return,
        };
        match remove_pane_by_id_with_info(ws, a_id) {
            Some(info) => info,
            None => return,
        }
    };

    // Step 2: Remove B from its workspace.
    let RemovedPaneInfo {
        pane: removed_b,
        was_only_in_column: b_was_only,
        column_id: b_original_col_id,
    } = {
        let ws = match session.workspaces.get_mut(b_ws) {
            Some(ws) => ws,
            None => return,
        };
        match remove_pane_by_id_with_info(ws, b_id) {
            Some(info) => info,
            None => return,
        }
    };

    // Step 3: Insert A into B's old position in B's workspace.
    if let Some(ws_b) = session.workspaces.get_mut(b_ws) {
        if b_was_only {
            // B's column was deleted when B was removed. Recreate it with A.
            let insert_pos = b_col.min(ws_b.scrolling.columns.len());
            let mut new_col = Column::new(b_original_col_id, removed_a, ColumnWidth::Proportion(0.5));
            if let Some(wa) = b_wa {
                new_col.compute_pane_sizes(wa.size.h, b_gaps);
            }
            ws_b.scrolling.add_column(Some(insert_pos), new_col, true);
        } else {
            // B's column still exists. Find it by ID and insert A at the same index.
            let target_ci = ws_b
                .scrolling
                .columns
                .iter()
                .position(|c| c.id == b_original_col_id)
                .unwrap_or(b_col.min(ws_b.scrolling.columns.len().saturating_sub(1)));
            let insert_idx = b_pi.min(ws_b.scrolling.columns[target_ci].panes.len());
            ws_b.scrolling
                .add_pane_to_column(target_ci, Some(insert_idx), removed_a, true);
        }
    }

    // Animate A from old position to new.
    if let Some(old_rect) = old_a_rect
        && let Some(ws_b) = session.workspaces.get_mut(b_ws)
        && let Some((new_ci, new_pi)) = find_pane_indices_in_workspace(ws_b, a_id)
    {
        let new_rects = ws_b.scrolling.panes_with_positions();
        if let Some((_, new_rect)) = new_rects
            .into_iter()
            .find(|(pid, _)| *pid == heca_core::layout::PaneId(a_id))
        {
            let offset = clamped_move_offset(old_rect, new_rect, max_dx, max_dy);
            ws_b.scrolling.columns[new_ci].panes[new_pi]
                .animate_move_from(offset, AnimationConfig::default());
        }
    }

    // Step 4: Insert B into A's old position in A's workspace.
    if let Some(ws_a) = session.workspaces.get_mut(a_ws) {
        if a_was_only {
            // A's column was deleted when A was removed. Recreate it with B.
            let insert_pos = a_col.min(ws_a.scrolling.columns.len());
            let mut new_col = Column::new(a_original_col_id, removed_b, ColumnWidth::Proportion(0.5));
            if let Some(wa) = a_wa {
                new_col.compute_pane_sizes(wa.size.h, a_gaps);
            }
            ws_a.scrolling.add_column(Some(insert_pos), new_col, true);
        } else {
            // A's column still exists. Find it by ID and insert B at the same index.
            let target_ci = ws_a
                .scrolling
                .columns
                .iter()
                .position(|c| c.id == a_original_col_id)
                .unwrap_or(a_col.min(ws_a.scrolling.columns.len().saturating_sub(1)));
            let insert_idx = a_pi.min(ws_a.scrolling.columns[target_ci].panes.len());
            ws_a.scrolling
                .add_pane_to_column(target_ci, Some(insert_idx), removed_b, true);
        }
    }

    // Animate B from old position to new.
    if let Some(old_rect) = old_b_rect
        && let Some(ws_a) = session.workspaces.get_mut(a_ws)
        && let Some((new_ci, new_pi)) = find_pane_indices_in_workspace(ws_a, b_id)
    {
        let new_rects = ws_a.scrolling.panes_with_positions();
        if let Some((_, new_rect)) = new_rects
            .into_iter()
            .find(|(pid, _)| *pid == heca_core::layout::PaneId(b_id))
        {
            let offset = clamped_move_offset(old_rect, new_rect, max_dx, max_dy);
            ws_a.scrolling.columns[new_ci].panes[new_pi]
                .animate_move_from(offset, AnimationConfig::default());
        }
    }
}
