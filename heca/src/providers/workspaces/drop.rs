//! **What a drop on the workspaces dock means** — beside the rows that declare the drag.
//!
//! The rows say what they are (`draggable_as("pane")`, `accepts(["pane", "column"])`); the framework
//! runs the gesture and hands back two names, already resolved. This turns them into the action
//! the drop means — `swap`, `place_pane`, `swap_columns` or `move_column` — and nothing else: the
//! action does the move, exactly as it would from a key or a plugin.

use crate::app_state::AppState;
use crate::chrome::ChromeDragItem;
use crate::input::WmAction;
use heca_grid_ui::drag::DropSide;

/// The action dropping `source` on `target` means, or `None` when it means nothing (onto itself,
/// or a pairing the dock does not take). `swap` is the drag API's own answer for the drop.
pub(crate) fn drop_action(
    state: &AppState,
    source: ChromeDragItem,
    target: ChromeDragItem,
    side: DropSide,
    swap: bool,
) -> Option<WmAction> {
    match source {
        ChromeDragItem::Pane(pane_id) => pane_drop(state, pane_id, target, side, swap),
        ChromeDragItem::Column { ws, col } => column_drop(ws, col, target, side, swap),
        // A workspace is a drop target only — nothing picks one up — and docks are the host's.
        ChromeDragItem::Workspace { .. }
        | ChromeDragItem::Dock(_)
        | ChromeDragItem::DockGap { .. } => None,
    }
}

/// A pane dropped on a pane card swaps with it (on swap) or lands beside it — before it on its
/// first half, after it on its second. Dropped on a workspace's header or empty area it moves
/// **into** that workspace, at the end: the only way to reach an empty one, which has no card to
/// aim at. A column is not a pane target.
fn pane_drop(
    state: &AppState,
    pane_id: heca_core::layout::PaneId,
    target: ChromeDragItem,
    side: DropSide,
    swap: bool,
) -> Option<WmAction> {
    let (ws_idx, col_idx, pane_idx) = match target {
        ChromeDragItem::Pane(target) if target == pane_id => return None,
        ChromeDragItem::Pane(target) if swap => {
            return Some(WmAction::Swap {
                a_id: pane_id,
                b_id: target,
            });
        }
        ChromeDragItem::Pane(target) => {
            let (ws_idx, _, _) = crate::find_pane_location(&state.session, target)?;
            let (col, row) = state
                .session
                .workspaces
                .get(ws_idx)?
                .scrolling
                .pane_indices(target)?;
            let row = match side {
                DropSide::Before => row,
                DropSide::Onto | DropSide::After => row + 1,
            };
            (ws_idx, col, Some(row))
        }
        // The end of the workspace: a gap past the last column clamps to it. A swap with a
        // workspace means nothing, so it is a plain move.
        ChromeDragItem::Workspace { ws } => {
            state.session.workspaces.get(ws)?;
            (ws, heca_core::layout::END, None)
        }
        ChromeDragItem::Column { .. }
        | ChromeDragItem::Dock(_)
        | ChromeDragItem::DockGap { .. } => return None,
    };
    Some(WmAction::PlacePane {
        pane_id,
        ws_idx,
        col_idx,
        pane_idx,
    })
}

/// A column dropped on a column inserts before or after it (or swaps, on swap); dropped on a
/// workspace it moves to the end of that workspace, where a swap is meaningless.
fn column_drop(
    src_ws: usize,
    src_col: usize,
    target: ChromeDragItem,
    side: DropSide,
    swap: bool,
) -> Option<WmAction> {
    match target {
        ChromeDragItem::Column {
            ws: dst_ws,
            col: dst_col,
        } => {
            if dst_ws == src_ws && dst_col == src_col {
                return None; // dropped on itself
            }
            if swap {
                return Some(WmAction::SwapColumns {
                    a_ws: src_ws,
                    a_col: src_col,
                    b_ws: dst_ws,
                    b_col: dst_col,
                });
            }
            let before = side == DropSide::Before; // Onto/After both insert after
            Some(WmAction::MoveColumn {
                src_ws,
                src_col,
                dst_ws,
                dst_idx: column_move_dst_idx(src_ws, src_col, dst_ws, dst_col, before),
                focus: true,
            })
        }
        ChromeDragItem::Workspace { ws: dst_ws } => Some(WmAction::MoveColumn {
            src_ws,
            src_col,
            dst_ws,
            dst_idx: heca_core::layout::END,
            focus: true,
        }),
        // The source-aware filter never yields a pane or dock target for a column drag.
        ChromeDragItem::Pane(_) | ChromeDragItem::Dock(_) | ChromeDragItem::DockGap { .. } => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A column dropped on itself, or a pane on a column, means nothing; a column on a workspace
    /// moves to its end whatever the modifier says.
    #[test]
    fn a_column_drop_means_one_action() {
        let col = |ws, col| ChromeDragItem::Column { ws, col };
        assert_eq!(column_drop(0, 1, col(0, 1), DropSide::Before, false), None);
        assert_eq!(
            column_drop(
                0,
                1,
                ChromeDragItem::Pane(heca_core::layout::PaneId(3)),
                DropSide::Onto,
                false
            ),
            None
        );
        assert!(matches!(
            column_drop(
                0,
                1,
                ChromeDragItem::Workspace { ws: 2 },
                DropSide::Onto,
                true
            ),
            Some(WmAction::MoveColumn {
                dst_ws: 2,
                dst_idx: heca_core::layout::END,
                ..
            })
        ));
        assert!(matches!(
            column_drop(0, 1, col(1, 0), DropSide::After, true),
            Some(WmAction::SwapColumns { .. })
        ));
    }
}
