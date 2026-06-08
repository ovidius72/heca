//! Shared pane-operation helpers.
//!
//! These helpers collect repeated low-level move/swap mechanics so handlers
//! and mouse paths can delegate to one implementation.

use heca_core::layout::animation::AnimationConfig;
use heca_core::layout::workspace::Workspace;
use heca_core::layout::{Column, ColumnId, ColumnWidth, Pane};
use heca_core::layout::types::PaneInsertTarget;

/// Information about a pane removed from a workspace.
#[derive(Debug)]
pub(crate) struct RemovedPane {
    pub pane: Pane,
}

/// Remove a pane from a workspace by pane id.
pub(crate) fn remove_pane_by_id(ws: &mut Workspace, pane_id: u64) -> Option<RemovedPane> {
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        if let Some(pi) = col.panes.iter().position(|p| p.id.0 == pane_id) {
            let pane = ws.scrolling.remove_pane(ci, pi)?;
            return Some(RemovedPane { pane });
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
