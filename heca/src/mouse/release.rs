//! What dropping a column means, once the gesture that produced it has ended — reached from the
//! drop the framework hands back, not from the pointer.

use crate::app::interaction::InteractionSource;
use crate::app_state::AppState;
use crate::chrome::ChromeDragItem;
use crate::input::WmAction;
use heca_grid_ui::drag::DropSide;

/// **What dropping a column means** — told what it landed on, rather than hunting for it under the
/// pointer at the moment of release (F003/P097/T496). The rules are unchanged: onto another column
/// it inserts before or after (or swaps, on Shift); onto a workspace it moves to the end of that
/// workspace, where a swap is meaningless.
pub(crate) fn column_drop(
    state: &mut AppState,
    src_ws: usize,
    src_col: usize,
    swap: bool,
    item: ChromeDragItem,
    side: DropSide,
) {
    let action = match item {
        ChromeDragItem::Column {
            ws: dst_ws,
            col: dst_col,
        } => {
            if swap {
                if dst_ws == src_ws && dst_col == src_col {
                    return; // swap with self
                }
                WmAction::SwapColumns {
                    a_ws: src_ws,
                    a_col: src_col,
                    b_ws: dst_ws,
                    b_col: dst_col,
                }
            } else {
                if dst_ws == src_ws && dst_col == src_col {
                    return; // dropped on itself
                }
                let before = side == DropSide::Before; // Onto/After both insert after
                let dst_idx = column_move_dst_idx(src_ws, src_col, dst_ws, dst_col, before);
                WmAction::MoveColumn {
                    src_ws,
                    src_col,
                    dst_ws,
                    dst_idx,
                    focus: true,
                }
            }
        }
        // Drop on a workspace → move the column to the end of that workspace
        // (usize::MAX clamps to the end inside the handler). Swap is meaningless here.
        ChromeDragItem::Workspace { ws: dst_ws } => WmAction::MoveColumn {
            src_ws,
            src_col,
            dst_ws,
            dst_idx: usize::MAX,
            focus: true,
        },
        // The source-aware filter never yields a pane target for a column drag.
        ChromeDragItem::Pane(_) => return,
    };

    super::dispatch_drop(state, InteractionSource::MouseLeftSidebar, action);
}

/// Final insert index for a column **move** onto target column `dst_col` in `dst_ws`,
/// given a `before` (vs after) drop side. Within the same workspace the source column
/// is removed first, so the target's index shifts left by one when the source sat to
/// its left — `MoveColumn`'s within-ws path (`reorder_column`) inserts at this final
/// index. Cross-workspace leaves `dst_ws` untouched, so it's a plain before/after.
/// Assumes `(src_ws, src_col) != (dst_ws, dst_col)` (self-drops are handled earlier).
fn column_move_dst_idx(
    src_ws: usize,
    src_col: usize,
    dst_ws: usize,
    dst_col: usize,
    before: bool,
) -> usize {
    if src_ws == dst_ws {
        let target_final = if src_col < dst_col {
            dst_col - 1
        } else {
            dst_col
        };
        if before {
            target_final
        } else {
            target_final + 1
        }
    } else if before {
        dst_col
    } else {
        dst_col + 1
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::column_move_dst_idx;

    #[test]
    fn within_ws_before_after_account_for_source_removal_shift() {
        // [A,B,C,D] (src=A@0). "Before C@2" → A lands at index 1 → [B,A,C,D].
        assert_eq!(column_move_dst_idx(0, 0, 0, 2, true), 1);
        // "After C@2" → A lands at index 2 → [B,C,A,D].
        assert_eq!(column_move_dst_idx(0, 0, 0, 2, false), 2);
        // src to the RIGHT of target: drag D@3 "Before B@1" → index 1 (no left shift).
        assert_eq!(column_move_dst_idx(0, 3, 0, 1, true), 1);
        assert_eq!(column_move_dst_idx(0, 3, 0, 1, false), 2);
    }

    #[test]
    fn cross_ws_is_a_plain_before_after_insert() {
        // Different workspace: dst is untouched by the source removal.
        assert_eq!(column_move_dst_idx(0, 1, 1, 2, true), 2);
        assert_eq!(column_move_dst_idx(0, 1, 1, 2, false), 3);
    }
}
