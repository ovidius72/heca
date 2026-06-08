//! Shared pane-operation helpers.
//!
//! These helpers collect repeated low-level move/swap mechanics so handlers
//! and mouse paths can delegate to one implementation.

use heca_core::layout::animation::AnimationConfig;
use heca_core::layout::workspace::Workspace;

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

    col.panes[first_pi].animate_move_y_from(up_offset, AnimationConfig::default());
    col.panes[second_pi].animate_move_y_from(down_offset, AnimationConfig::default());
    col.panes.swap(first_pi, second_pi);
    col.active_pane_idx = second_pi;
    col.compute_pane_sizes(ws.scrolling.working_area.size.h, ws.scrolling.options.gaps);
    true
}
