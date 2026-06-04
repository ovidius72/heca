//! Named action handlers — one per `WmAction` variant.
//!
//! Each handler is a plain `fn(&mut AppState, &WmAction)` that performs the
//! action.  Parameterized variants destructure their fields from the enum;
//! unit variants ignore the `_action` parameter.

use crate::app_state::{AppState, InputMode, RenameTarget};
use crate::input::WmAction;
use crate::sidebar;
use crate::{
    collect_all_pane_candidates, destroy_empty_workspace, find_pane_location, focus_pane_by_id,
    move_pane_to_column, move_pane_to_workspace_column, pane_name, switch_workspace_tracked,
    sync_focus, update_session_viewport,
};
use heca_core::backend::FakeBackend;
use heca_core::layout::animation::AnimationConfig;
use heca_core::layout::types::Point;
use heca_core::layout::{Column, ColumnId, ColumnWidth, Pane as LayoutPane, PaneId};

// ── Navigation ──

pub fn handle_focus_left(state: &mut AppState, _action: &WmAction) {
    state.session.focus_left();
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_focus_right(state: &mut AppState, _action: &WmAction) {
    state.session.focus_right();
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_focus_up(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.focus_up();
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_focus_down(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.focus_down();
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_next_pane(state: &mut AppState, _action: &WmAction) {
    state.session.focus_right();
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_prev_pane(state: &mut AppState, _action: &WmAction) {
    state.session.focus_left();
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_workspace_next(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    let next = (current_ws + 1).min(state.session.workspaces.len().saturating_sub(1));
    if next != current_ws {
        switch_workspace_tracked(state, next);
        sync_focus(state);
        state.needs_redraw = true;
    }
}

pub fn handle_workspace_prev(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    let prev = current_ws.saturating_sub(1);
    if prev != current_ws {
        switch_workspace_tracked(state, prev);
        sync_focus(state);
        state.needs_redraw = true;
    }
}

pub fn handle_focus_toggle_local(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.session.active_workspace_idx;
    if let Some(prev_pane) = state
        .last_visited_pane_per_ws
        .get(ws_idx)
        .copied()
        .flatten()
    {
        if Some(prev_pane) != state.focused_pane {
            focus_pane_by_id(state, prev_pane);
        }
        state.needs_redraw = true;
    }
}

pub fn handle_focus_toggle_global(state: &mut AppState, _action: &WmAction) {
    if let Some(prev_ws) = state.last_visited_ws_idx {
        let current_ws = state.session.active_workspace_idx;
        if prev_ws != current_ws {
            switch_workspace_tracked(state, prev_ws);
            sync_focus(state);
        }
        state.needs_redraw = true;
    }
}

pub fn handle_focus_pane(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusPane { pane_id } = action else {
        return;
    };
    focus_pane_by_id(state, *pane_id);
}

pub fn handle_focus_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusWorkspace { ws_idx } = action else {
        return;
    };
    if *ws_idx < state.session.workspaces.len() {
        switch_workspace_tracked(state, *ws_idx);
        sync_focus(state);
        state.needs_redraw = true;
    }
}

// ── Layout ──

pub fn handle_split_horizontal(state: &mut AppState, _action: &WmAction) {
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
    let backend_id = next_id;
    state.session.add_pane(pane, None, true);
    state
        .backends
        .insert(backend_id, Box::new(FakeBackend::new(80, 24)));
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_split_vertical(state: &mut AppState, _action: &WmAction) {
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
    let backend_id = next_id;
    let col_idx = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.add_pane_to_column(col_idx, None, pane, true);
    }
    state
        .backends
        .insert(backend_id, Box::new(FakeBackend::new(80, 24)));
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_resize_increase(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_active_column(0.05);
    }
    state.needs_redraw = true;
}

pub fn handle_resize_decrease(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_active_column(-0.05);
    }
    state.needs_redraw = true;
}

pub fn handle_zoom_column(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.toggle_active_column_zoom();
    }
    state.needs_redraw = true;
}

pub fn handle_pane_height_increase(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.columns.get_mut(col_idx) {
            let h = ws.scrolling.working_area.size.h;
            let gaps = ws.scrolling.options.gaps;
            col.resize_active_pane_height(40.0, h, gaps);
        }
    }
    state.needs_redraw = true;
}

pub fn handle_pane_height_decrease(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.columns.get_mut(col_idx) {
            let h = ws.scrolling.working_area.size.h;
            let gaps = ws.scrolling.options.gaps;
            col.resize_active_pane_height(-40.0, h, gaps);
        }
    }
    state.needs_redraw = true;
}

pub fn handle_swap_left(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_column_left();
        ws.scrolling.align_view_to_active_column();
    }
    state.needs_redraw = true;
}

pub fn handle_swap_right(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_column_right();
        ws.scrolling.align_view_to_active_column();
    }
    state.needs_redraw = true;
}

pub fn handle_swap_up(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.active_column() {
            let pane_idx = col.active_pane_idx;
            let swap_with = pane_idx.saturating_sub(1);
            if swap_with != pane_idx
                && let Some(col) = ws.scrolling.columns.get_mut(col_idx)
            {
                let h_above = col
                    .pane_sizes
                    .get(pane_idx.min(swap_with))
                    .map(|s| s.h)
                    .unwrap_or(0.0);
                let h_below = col
                    .pane_sizes
                    .get(pane_idx.max(swap_with))
                    .map(|s| s.h)
                    .unwrap_or(0.0);
                let gap = ws.scrolling.options.gaps;
                let up_offset = h_above + gap;
                let down_offset = -(h_below + gap);
                col.panes[pane_idx].animate_move_y_from(up_offset, AnimationConfig::default());
                col.panes[swap_with].animate_move_y_from(down_offset, AnimationConfig::default());
                col.panes.swap(pane_idx, swap_with);
                col.active_pane_idx = swap_with;
                col.compute_pane_sizes(ws.scrolling.working_area.size.h, ws.scrolling.options.gaps);
            }
        }
    }
    sync_focus(state);
    if let Some(pane_id) = state.focused_pane {
        crate::focus_pane_by_id(state, pane_id);
    }
    state.needs_redraw = true;
}

pub fn handle_swap_down(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.active_column() {
            let pane_idx = col.active_pane_idx;
            let swap_with = (pane_idx + 1).min(col.panes.len().saturating_sub(1));
            if swap_with != pane_idx
                && let Some(col) = ws.scrolling.columns.get_mut(col_idx)
            {
                let h_above = col
                    .pane_sizes
                    .get(pane_idx.min(swap_with))
                    .map(|s| s.h)
                    .unwrap_or(0.0);
                let h_below = col
                    .pane_sizes
                    .get(pane_idx.max(swap_with))
                    .map(|s| s.h)
                    .unwrap_or(0.0);
                let gap = ws.scrolling.options.gaps;
                let up_offset = h_above + gap;
                let down_offset = -(h_below + gap);
                col.panes[pane_idx].animate_move_y_from(down_offset, AnimationConfig::default());
                col.panes[swap_with].animate_move_y_from(up_offset, AnimationConfig::default());
                col.panes.swap(pane_idx, swap_with);
                col.active_pane_idx = swap_with;
                col.compute_pane_sizes(ws.scrolling.working_area.size.h, ws.scrolling.options.gaps);
            }
        }
    }
    sync_focus(state);
    if let Some(pane_id) = state.focused_pane {
        crate::focus_pane_by_id(state, pane_id);
    }
    state.needs_redraw = true;
}

pub fn handle_move_pane_left(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_active_pane_left();
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_move_pane_right(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.move_active_pane_right();
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_move_column_up(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    if current_ws == 0 {
        return;
    }
    let target_ws = current_ws - 1;
    let col_idx = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    crate::move_column_to_workspace(state, col_idx, target_ws, true);
}

pub fn handle_move_column_down(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    if current_ws >= state.session.workspaces.len().saturating_sub(1) {
        return;
    }
    let target_ws = current_ws + 1;
    let col_idx = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    crate::move_column_to_workspace(state, col_idx, target_ws, true);
}

pub fn handle_move_column_to_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::MoveColumnToWorkspace {
        col_idx,
        ws_idx,
        focus,
    } = action
    else {
        return;
    };
    crate::move_column_to_workspace(state, *col_idx, *ws_idx, *focus);
}

pub fn handle_swap_param(state: &mut AppState, action: &WmAction) {
    let WmAction::Swap { a_id, b_id } = action else {
        return;
    };
    if a_id == b_id {
        return;
    }

    // Find both panes' locations.
    let a_loc = find_pane_location(&state.session, *a_id);
    let b_loc = find_pane_location(&state.session, *b_id);
    let ((aws, acol, api), (bws, bcol, bpi)) = match (a_loc, b_loc) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            return;
        }
    };

    // True swap: exchange positions of both panes.
    // Same workspace: remove both (higher index first to avoid shift), then re-insert.
    // Different workspaces: remove A, remove B, insert A at B's pos, insert B at A's pos.
    if aws == bws {
        // Same workspace.
        if acol == bcol {
            // Same column: swap panes in-place (robust & avoids index-shift pitfalls).
            let (first_pi, second_pi) = if api < bpi { (api, bpi) } else { (bpi, api) };
            if let Some(ws) = state.session.workspaces.get_mut(aws) {
                if acol >= ws.scrolling.columns.len() {
                    return;
                }
                if second_pi >= ws.scrolling.columns[acol].panes.len()
                    || first_pi >= ws.scrolling.columns[acol].panes.len()
                {
                    return;
                }
                // Animate vertical motion (approximate) then swap.
                let col = &mut ws.scrolling.columns[acol];
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
            }
        } else {
            // Different columns, same workspace: perform reinsert-first to avoid column deletion
            // (which causes a slide animation). We insert placeholders at both target spots,
            // then remove the original panes and replace placeholders with the real panes,
            // animating each pane from its old position to its new position.

            let a_pid = *a_id;
            let b_pid = *b_id;

            // Capture old positions (immutable borrow) so we can animate from them later.
            let old_rects = state
                .session
                .workspaces
                .get(aws)
                .map(|ws| ws.scrolling.panes_with_positions())
                .unwrap_or_default();
            let old_a_rect = old_rects
                .iter()
                .find(|(pid, _)| *pid == heca_core::layout::PaneId(a_pid))
                .map(|(_, r)| *r);
            let old_b_rect = old_rects
                .iter()
                .find(|(pid, _)| *pid == heca_core::layout::PaneId(b_pid))
                .map(|(_, r)| *r);

            // Pre-generate placeholder pane ids and new column ids before mutably borrowing the workspace.
            let placeholder_a_pid = state.session.next_id();
            let placeholder_b_pid = state.session.next_id();
            let new_col_for_a = ColumnId(state.session.next_id());
            let new_col_for_b = ColumnId(state.session.next_id());

            // Work on the workspace mutably.
            if let Some(ws) = state.session.workspaces.get_mut(aws) {
                // Insert placeholders in descending column index order to avoid shifting column indices
                // when creating new columns.
                // We need to insert placeholders at the TARGET positions: A should land at B's slot,
                // and B should land at A's slot. This avoids replacing the placeholder with the
                // same pane and makes the swap effective.
                let mut inserts = vec![
                    (bcol, bpi, placeholder_a_pid, new_col_for_a),
                    (acol, api, placeholder_b_pid, new_col_for_b),
                ];
                // Insert in descending col index order so earlier inserts don't shift later targets.
                inserts.sort_by_key(|b| std::cmp::Reverse(b.0));

                for (col_pos, pane_idx, ph_pid, new_cid) in inserts {
                    if col_pos < ws.scrolling.columns.len() {
                        let insert_idx = pane_idx.min(ws.scrolling.columns[col_pos].panes.len());
                        let placeholder = LayoutPane::new(PaneId(ph_pid), pane_name(ph_pid));
                        ws.scrolling.add_pane_to_column(
                            col_pos,
                            Some(insert_idx),
                            placeholder,
                            true,
                        );
                    } else {
                        let pos = col_pos.min(ws.scrolling.columns.len());
                        let placeholder = LayoutPane::new(PaneId(ph_pid), pane_name(ph_pid));
                        ws.scrolling.add_column(
                            Some(pos),
                            Column::new(new_cid, placeholder, ColumnWidth::Proportion(0.5)),
                            true,
                        );
                    }
                }

                // After placeholders exist, find and remove the original panes by id.
                // Removing order: higher column index first to avoid index invalidation.
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

                // Decide removal order by column index (descending)
                let mut removes = vec![];
                if let Some((ci, pi)) = found_a {
                    removes.push((ci, pi, a_pid));
                }
                if let Some((ci, pi)) = found_b {
                    removes.push((ci, pi, b_pid));
                }
                removes.sort_by_key(|y| std::cmp::Reverse(y.0));

                let mut removed_a: Option<LayoutPane> = None;
                let mut removed_b: Option<LayoutPane> = None;

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

                // Now replace placeholders with the removed panes and animate from old positions.
                // Helper to find placeholder by pane id. Clamp large dx/dy for cross-workspace cases.
                let vw = state.session.viewport_size.w;
                let vh = state.session.viewport_size.h;
                let max_dx = vw * 0.9;
                let max_dy = vh * 0.9;

                let replace_placeholder = |ws: &mut heca_core::layout::workspace::Workspace,
                                           ph_id: u64,
                                           new_pane: LayoutPane,
                                           old_rect_opt: Option<
                    heca_core::layout::types::Rectangle,
                >| {
                    let mut found = None;
                    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
                        for (pi, pane) in col.panes.iter().enumerate() {
                            if pane.id.0 == ph_id {
                                found = Some((ci, pi));
                                break;
                            }
                        }
                        if found.is_some() {
                            break;
                        }
                    }
                    if let Some((ci, pi)) = found {
                        ws.scrolling.columns[ci].panes[pi] = new_pane;
                        ws.scrolling.columns[ci].active_pane_idx = pi;
                        ws.scrolling.columns[ci].compute_pane_sizes(
                            ws.scrolling.working_area.size.h,
                            ws.scrolling.options.gaps,
                        );
                        ws.scrolling.update_all_column_widths();

                        if let Some(old_rect) = old_rect_opt
                            && let Some((_, new_rect)) = ws
                                .scrolling
                                .panes_with_positions()
                                .into_iter()
                                .find(|(pid, _)| *pid == ws.scrolling.columns[ci].panes[pi].id)
                        {
                            let mut dx = old_rect.loc.x - new_rect.loc.x;
                            let mut dy = old_rect.loc.y - new_rect.loc.y;
                            // Clamp extreme values so panes don't dash across the whole window when
                            // swapping between workspaces (coordinate frames may differ).
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
                            ws.scrolling.columns[ci].panes[pi]
                                .animate_move_from(Point::new(dx, dy), AnimationConfig::default());
                        }
                    }
                };

                if let Some(a_pane) = removed_a {
                    replace_placeholder(ws, placeholder_a_pid, a_pane, old_a_rect);
                }
                if let Some(b_pane) = removed_b {
                    replace_placeholder(ws, placeholder_b_pid, b_pane, old_b_rect);
                }
            }
        }
    } else {
        // Different workspaces: remove-then-insert approach.
        //
        // The previous reinsert-first (placeholder) approach had an index-shift bug:
        // after inserting a placeholder in B's column at B's index, B's actual index shifts
        // by +1, but we were still removing at the original index (removing the placeholder
        // instead of B).
        //
        // New approach: remove both panes by ID (searching for them), then insert each at
        // the other's target position. If removing a pane deletes its column (it was the only
        // pane), we recreate the column for the incoming pane.

        // Capture working area info for column recreation.
        let a_wa = state
            .session
            .workspaces
            .get(aws)
            .map(|ws| ws.scrolling.working_area);
        let b_wa = state
            .session
            .workspaces
            .get(bws)
            .map(|ws| ws.scrolling.working_area);
        let a_gaps = state
            .session
            .workspaces
            .get(aws)
            .map(|ws| ws.scrolling.options.gaps)
            .unwrap_or(0.0);
        let b_gaps = state
            .session
            .workspaces
            .get(bws)
            .map(|ws| ws.scrolling.options.gaps)
            .unwrap_or(0.0);

        // Capture old pane render positions (used to compute animation offsets).
        let old_a_rect = state.session.workspaces.get(aws).and_then(|ws| {
            ws.scrolling
                .panes_with_positions()
                .into_iter()
                .find(|(pid, _)| *pid == heca_core::layout::PaneId(*a_id))
                .map(|(_, r)| r)
        });
        let old_b_rect = state.session.workspaces.get(bws).and_then(|ws| {
            ws.scrolling
                .panes_with_positions()
                .into_iter()
                .find(|(pid, _)| *pid == heca_core::layout::PaneId(*b_id))
                .map(|(_, r)| r)
        });

        // Helper: remove pane by ID from a workspace. Returns (removed_pane, was_only_pane_in_column, column_id).
        // If the column was deleted (pane was alone), was_only_pane_in_column is true.
        let remove_pane_by_id = |session: &mut heca_core::layout::session::Session,
                                 ws_idx: usize,
                                 pane_id_val: u64|
         -> Option<(LayoutPane, bool, ColumnId)> {
            let ws = session.workspaces.get_mut(ws_idx)?;
            for (ci, col) in ws.scrolling.columns.iter().enumerate() {
                if let Some(pi) = col.panes.iter().position(|p| p.id.0 == pane_id_val) {
                    let col_id = col.id;
                    let was_only = col.panes.len() == 1;
                    let removed = ws.scrolling.remove_pane(ci, pi)?;
                    return Some((removed, was_only, col_id));
                }
            }
            None
        };

        // Step 1: Remove A from its workspace.
        let (removed_a, a_was_only, a_original_col_id) =
            match remove_pane_by_id(&mut state.session, aws, *a_id) {
                Some(r) => r,
                None => {
                    return;
                }
            };

        // Step 2: Remove B from its workspace.
        let (removed_b, b_was_only, b_original_col_id) =
            match remove_pane_by_id(&mut state.session, bws, *b_id) {
                Some(r) => r,
                None => {
                    return;
                }
            };

        // Step 3: Insert A into B's old position in B's workspace.
        // If B's column was deleted (was_only), recreate it.
        let vw = state.session.viewport_size.w;
        let vh = state.session.viewport_size.h;
        let max_dx = vw * 0.9;
        let max_dy = vh * 0.9;

        if let Some(ws_b) = state.session.workspaces.get_mut(bws) {
            if b_was_only {
                // B's column was deleted when B was removed. Recreate it with A.
                let insert_pos = bcol.min(ws_b.scrolling.columns.len());
                let mut new_col =
                    Column::new(b_original_col_id, removed_a, ColumnWidth::Proportion(0.5));
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
                    .unwrap_or(bcol.min(ws_b.scrolling.columns.len().saturating_sub(1)));
                let insert_idx = bpi.min(ws_b.scrolling.columns[target_ci].panes.len());
                ws_b.scrolling
                    .add_pane_to_column(target_ci, Some(insert_idx), removed_a, true);
            }
        }

        // Animate A from old position to new (separate borrow scope).
        if let Some(old_rect) = old_a_rect
            && let Some(ws_b) = state.session.workspaces.get_mut(bws)
        {
            // Find A's new position in its workspace.
            let mut found = None;
            for (ci, col) in ws_b.scrolling.columns.iter().enumerate() {
                for (pi, p) in col.panes.iter().enumerate() {
                    if p.id.0 == *a_id {
                        found = Some((ci, pi));
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            if let Some((new_ci, new_pi)) = found
                && let Some((_, new_rect)) = ws_b
                    .scrolling
                    .panes_with_positions()
                    .into_iter()
                    .find(|(pid, _)| *pid == heca_core::layout::PaneId(*a_id))
            {
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
                ws_b.scrolling.columns[new_ci].panes[new_pi]
                    .animate_move_from(Point::new(dx, dy), AnimationConfig::default());
            }
        }

        // Step 4: Insert B into A's old position in A's workspace.
        if let Some(ws_a) = state.session.workspaces.get_mut(aws) {
            if a_was_only {
                // A's column was deleted when A was removed. Recreate it with B.
                let insert_pos = acol.min(ws_a.scrolling.columns.len());
                let mut new_col =
                    Column::new(a_original_col_id, removed_b, ColumnWidth::Proportion(0.5));
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
                    .unwrap_or(acol.min(ws_a.scrolling.columns.len().saturating_sub(1)));
                let insert_idx = api.min(ws_a.scrolling.columns[target_ci].panes.len());
                ws_a.scrolling
                    .add_pane_to_column(target_ci, Some(insert_idx), removed_b, true);
            }
        }

        // Animate B from old position to new (separate borrow scope).
        if let Some(old_rect) = old_b_rect
            && let Some(ws_a) = state.session.workspaces.get_mut(aws)
        {
            let mut found = None;
            for (ci, col) in ws_a.scrolling.columns.iter().enumerate() {
                for (pi, p) in col.panes.iter().enumerate() {
                    if p.id.0 == *b_id {
                        found = Some((ci, pi));
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            if let Some((new_ci, new_pi)) = found
                && let Some((_, new_rect)) = ws_a
                    .scrolling
                    .panes_with_positions()
                    .into_iter()
                    .find(|(pid, _)| *pid == heca_core::layout::PaneId(*b_id))
            {
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
                ws_a.scrolling.columns[new_ci].panes[new_pi]
                    .animate_move_from(Point::new(dx, dy), AnimationConfig::default());
            }
        }
    }

    // Update AppState.focused_pane and sidebar after the swap, then log final locations.
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_move_param(state: &mut AppState, action: &WmAction) {
    let WmAction::Move {
        pane_id,
        target_col,
    } = action
    else {
        return;
    };
    if let Some((ws_idx, col_idx, _)) = find_pane_location(&state.session, *pane_id) {
        // Ensure the source workspace is active before calling move_pane_to_column,
        // which operates on the active workspace.
        if state.session.active_workspace_idx != ws_idx {
            switch_workspace_tracked(state, ws_idx);
        }
        move_pane_to_column(state, *pane_id, col_idx, *target_col);
    }
    state.needs_redraw = true;
}

pub fn handle_move_pane_to_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::MovePaneToWorkspace { pane_id, ws_idx } = action else {
        return;
    };
    if let Some((current_ws, current_col, _)) = find_pane_location(&state.session, *pane_id)
        && current_ws != *ws_idx
    {
        if state.session.active_workspace_idx != current_ws {
            switch_workspace_tracked(state, current_ws);
        }
        move_pane_to_workspace_column(state, *pane_id, *ws_idx, current_col);
    }
    state.needs_redraw = true;
}

pub fn handle_move_pane_to_column(state: &mut AppState, action: &WmAction) {
    let WmAction::MovePaneToColumn {
        pane_id,
        ws_idx,
        col_idx,
    } = action
    else {
        return;
    };
    if let Some((current_ws, current_col, _)) = find_pane_location(&state.session, *pane_id) {
        if current_ws == *ws_idx {
            if state.session.active_workspace_idx != current_ws {
                switch_workspace_tracked(state, current_ws);
            }
            move_pane_to_column(state, *pane_id, current_col, *col_idx);
        } else {
            if state.session.active_workspace_idx != current_ws {
                switch_workspace_tracked(state, current_ws);
            }
            move_pane_to_workspace_column(state, *pane_id, *ws_idx, *col_idx);
        }
    }
    state.needs_redraw = true;
}

pub fn handle_resize(state: &mut AppState, action: &WmAction) {
    let WmAction::Resize {
        target,
        axis,
        amount,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        match (target, axis) {
            (crate::input::ResizeTarget::Column, crate::input::ResizeAxis::X) => {
                let delta_f = *amount / 1000.0;
                ws.scrolling.resize_active_column(delta_f);
            }
            (crate::input::ResizeTarget::Pane, crate::input::ResizeAxis::Y) => {
                let h = ws.scrolling.working_area.size.h;
                let gaps = ws.scrolling.options.gaps;
                if let Some(col) = ws.scrolling.active_column_mut() {
                    col.resize_active_pane_height(*amount, h, gaps);
                }
            }
            _ => {} // Column-Y and Pane-X are not yet implemented
        }
    }
    state.needs_redraw = true;
}

pub fn handle_resize_to(state: &mut AppState, action: &WmAction) {
    let WmAction::ResizeTo {
        target,
        width,
        height,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        match target {
            crate::input::ResizeTarget::Column => {
                if let Some(col) = ws.scrolling.active_column_mut() {
                    col.width = ColumnWidth::Fixed(*width);
                    ws.scrolling.update_all_column_widths();
                }
            }
            crate::input::ResizeTarget::Pane => {
                let h = ws.scrolling.working_area.size.h;
                let gaps = ws.scrolling.options.gaps;
                if let Some(col) = ws.scrolling.active_column_mut() {
                    let pane_idx = col.active_pane_idx;
                    if let Some(size) = col.pane_sizes.get_mut(pane_idx) {
                        size.h = *height;
                    }
                    col.compute_pane_sizes(h, gaps);
                }
            }
        }
    }
    state.needs_redraw = true;
}

// ── Pane ──

pub fn handle_float(state: &mut AppState, _action: &WmAction) {
    if let Some(pane_id) = state.focused_pane
        && let Some(ws) = state.session.active_workspace_mut()
    {
        let wa = ws.scrolling.working_area;
        let is_floating = ws.floating_panes.iter().any(|f| f.pane.id.0 == pane_id);

        if is_floating {
            if let Some(idx) = ws
                .floating_panes
                .iter()
                .position(|f| f.pane.id.0 == pane_id)
            {
                let float = ws.floating_panes.remove(idx);
                let orig_col = float.original_column_idx;
                let orig_pane = float.original_pane_idx;
                ws.deactivate_floating_panes();
                if let Some(col_idx) = orig_col {
                    if col_idx < ws.scrolling.columns.len() {
                        let target_idx = orig_pane
                            .unwrap_or(0)
                            .min(ws.scrolling.columns[col_idx].panes.len());
                        ws.scrolling.add_pane_to_column(
                            col_idx,
                            Some(target_idx),
                            float.pane,
                            true,
                        );
                    } else {
                        ws.scrolling.add_column(
                            None,
                            Column::new(
                                ColumnId(pane_id),
                                float.pane,
                                ColumnWidth::Proportion(0.5),
                            ),
                            true,
                        );
                    }
                } else {
                    ws.scrolling.add_column(
                        None,
                        Column::new(ColumnId(pane_id), float.pane, ColumnWidth::Proportion(0.5)),
                        true,
                    );
                }
                ws.floating_is_active = false;
            }
        } else {
            let mut found = None;
            for (ci, col) in ws.scrolling.columns.iter().enumerate() {
                for (pi, pane) in col.panes.iter().enumerate() {
                    if pane.id.0 == pane_id {
                        found = Some((ci, pi));
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            if let Some((col_idx, pane_idx)) = found
                && let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx)
            {
                let fw = wa.size.w * 0.95;
                let fh = wa.size.h * 0.95;
                let fx = wa.loc.x + (wa.size.w - fw) / 2.0;
                let fy = wa.loc.y + (wa.size.h - fh) / 2.0;
                ws.deactivate_floating_panes();
                ws.floating_panes
                    .push(heca_core::layout::workspace::FloatingPane {
                        pane: removed,
                        position: heca_core::layout::types::Point::new(fx, fy),
                        size: heca_core::layout::types::Size::new(fw, fh),
                        is_active: true,
                        original_column_idx: Some(col_idx),
                        original_pane_idx: Some(pane_idx),
                    });
                ws.floating_is_active = true;
            }
        }
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_close_pane(state: &mut AppState, _action: &WmAction) {
    let current_ws = state.session.active_workspace_idx;
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.active_column() {
            let pane_idx = col.active_pane_idx;
            if let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx) {
                state.backends.remove(&removed.id.0);
            }
        }
    }

    let ws_is_empty = state
        .session
        .workspaces
        .get(current_ws)
        .map(|ws| ws.scrolling.columns.iter().all(|c| c.panes.is_empty()))
        .unwrap_or(true);

    if ws_is_empty && state.session.workspaces.len() > 1 {
        destroy_empty_workspace(state, current_ws);
        let new_idx = current_ws.min(state.session.workspaces.len().saturating_sub(1));
        state.session.switch_to_workspace(new_idx);
        sync_focus(state);
    } else if ws_is_empty {
        let next_id = state.session.next_id();
        let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
        state.session.add_pane(pane, None, true);
        state
            .backends
            .insert(next_id, Box::new(FakeBackend::new(80, 24)));
        sync_focus(state);
    } else {
        sync_focus(state);
    }
    state.needs_redraw = true;
}

pub fn handle_pane_select(state: &mut AppState, _action: &WmAction) {
    let candidates = collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSelect { candidates };
        state.needs_redraw = true;
    }
}

pub fn handle_swap_pane(state: &mut AppState, _action: &WmAction) {
    let candidates = collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSwap {
            candidates,
            focus_after: false,
        };
        state.needs_redraw = true;
    }
}

pub fn handle_swap_and_focus_pane(state: &mut AppState, _action: &WmAction) {
    let candidates = collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSwap {
            candidates,
            focus_after: true,
        };
        state.needs_redraw = true;
    }
}

pub fn handle_rename_pane(state: &mut AppState, _action: &WmAction) {
    if let Some(pane_id) = state.focused_pane {
        let current_title = state
            .session
            .active_workspace()
            .and_then(|ws| ws.find_pane(heca_core::layout::PaneId(pane_id)))
            .map(|p| p.title.clone())
            .unwrap_or_default();
        state.input_mode = InputMode::Rename {
            target: RenameTarget::Pane(pane_id),
            buffer: current_title,
        };
        state.needs_redraw = true;
    }
}

pub fn handle_rename_column(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.session.active_workspace_idx;
    if let Some(ws) = state.session.active_workspace() {
        let col_idx = ws.scrolling.active_column_idx;
        let current_name = ws
            .scrolling
            .columns
            .get(col_idx)
            .and_then(|col| col.name.clone())
            .unwrap_or_default();
        state.input_mode = InputMode::Rename {
            target: RenameTarget::Column { ws_idx, col_idx },
            buffer: current_name,
        };
        state.needs_redraw = true;
    }
}

pub fn handle_float_at(state: &mut AppState, action: &WmAction) {
    let WmAction::FloatAt {
        pane_id,
        x,
        y,
        width,
        height,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        let mut found = None;
        for (ci, col) in ws.scrolling.columns.iter().enumerate() {
            for (pi, pane) in col.panes.iter().enumerate() {
                if pane.id.0 == *pane_id {
                    found = Some((ci, pi));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        if let Some((col_idx, pane_idx)) = found
            && let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx)
        {
            ws.deactivate_floating_panes();
            ws.floating_panes
                .push(heca_core::layout::workspace::FloatingPane {
                    pane: removed,
                    position: heca_core::layout::types::Point::new(*x, *y),
                    size: heca_core::layout::types::Size::new(*width, *height),
                    is_active: true,
                    original_column_idx: Some(col_idx),
                    original_pane_idx: Some(pane_idx),
                });
            ws.floating_is_active = true;
        }
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_close_pane_by_id(state: &mut AppState, action: &WmAction) {
    let WmAction::ClosePaneById { pane_id } = action else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        let mut found = None;
        for (ci, col) in ws.scrolling.columns.iter().enumerate() {
            if let Some(pi) = col.panes.iter().position(|p| p.id.0 == *pane_id) {
                found = Some((ci, pi));
                break;
            }
        }
        if let Some((ci, pi)) = found
            && let Some(removed) = ws.scrolling.remove_pane(ci, pi)
        {
            state.backends.remove(&removed.id.0);
        }
    }
    let current_ws = state.session.active_workspace_idx;
    let ws_is_empty = state
        .session
        .workspaces
        .get(current_ws)
        .map(|ws| ws.scrolling.columns.iter().all(|c| c.panes.is_empty()))
        .unwrap_or(true);
    if ws_is_empty && state.session.workspaces.len() > 1 {
        destroy_empty_workspace(state, current_ws);
        let new_idx = current_ws.min(state.session.workspaces.len().saturating_sub(1));
        state.session.switch_to_workspace(new_idx);
        sync_focus(state);
    } else if ws_is_empty {
        let next_id = state.session.next_id();
        let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
        state.session.add_pane(pane, None, true);
        state
            .backends
            .insert(next_id, Box::new(FakeBackend::new(80, 24)));
        sync_focus(state);
    } else {
        sync_focus(state);
    }
    state.needs_redraw = true;
}

pub fn handle_rename_target(state: &mut AppState, action: &WmAction) {
    let WmAction::RenameTarget { pane_id, name } = action else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut()
        && let Some(pane) = ws.find_pane_mut(heca_core::layout::PaneId(*pane_id))
    {
        pane.title = if name.is_empty() {
            format!("pane{pane_id}")
        } else {
            name.clone()
        };
        sync_focus(state);
        state.needs_redraw = true;
    }
}

// ── Workspace ──

/// Add a pane to a specific column in a specific workspace.
/// Switches to the target workspace first.
pub fn handle_add_pane_to_column(state: &mut AppState, action: &WmAction) {
    let WmAction::AddPaneToColumn { ws_idx, col_idx } = action else {
        return;
    };
    let target_ws = *ws_idx;
    if target_ws >= state.session.workspaces.len() {
        return;
    }
    // Switch to target workspace if needed
    if state.session.active_workspace_idx != target_ws {
        crate::switch_workspace_tracked(state, target_ws);
    }
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
    let backend_id = next_id;
    let col = *col_idx;
    if let Some(ws) = state.session.active_workspace_mut() {
        let capped_col = col.min(ws.scrolling.columns.len().saturating_sub(1));
        ws.scrolling
            .add_pane_to_column(capped_col, None, pane, true);
    }
    state
        .backends
        .insert(backend_id, Box::new(FakeBackend::new(80, 24)));
    sync_focus(state);
    state.needs_redraw = true;
}

/// Delete a column and all its panes (destructive).
pub fn handle_delete_column(state: &mut AppState, action: &WmAction) {
    let WmAction::DeleteColumn { ws_idx, col_idx } = action else {
        return;
    };
    let target_ws = *ws_idx;
    if target_ws >= state.session.workspaces.len() {
        return;
    }
    // Collect pane IDs from the column, remove backends, then remove the column.
    let pane_ids: Vec<u64> = state
        .session
        .workspaces
        .get(target_ws)
        .and_then(|ws| ws.scrolling.columns.get(*col_idx))
        .map(|col| col.panes.iter().map(|p| p.id.0).collect())
        .unwrap_or_default();

    // Remove backends
    for pid in &pane_ids {
        state.backends.remove(pid);
    }

    // Remove the column
    if let Some(ws) = state.session.workspaces.get_mut(target_ws) {
        let capped_col = (*col_idx).min(ws.scrolling.columns.len().saturating_sub(1));
        ws.scrolling.remove_column(capped_col);
    }

    sync_focus(state);
    state.needs_redraw = true;
}

/// Delete a workspace and all its columns/panes (destructive).
/// The last workspace cannot be deleted.
pub fn handle_delete_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::DeleteWorkspace { ws_idx } = action else {
        return;
    };
    let target_ws = *ws_idx;
    if target_ws >= state.session.workspaces.len() || state.session.workspaces.len() <= 1 {
        return;
    }

    // Collect all pane IDs from the workspace to clean up backends
    let pane_ids: Vec<u64> = state
        .session
        .workspaces
        .get(target_ws)
        .map(|ws| {
            ws.scrolling
                .columns
                .iter()
                .flat_map(|col| col.panes.iter().map(|p| p.id.0))
                .collect()
        })
        .unwrap_or_default();

    // Remove backends
    for pid in &pane_ids {
        state.backends.remove(pid);
    }

    // Remove the workspace
    state.session.remove_workspace(target_ws);

    // Fix up tracking indices (same logic as destroy_empty_workspace)
    if state.last_visited_ws_idx == Some(target_ws) {
        state.last_visited_ws_idx = None;
    } else if let Some(ref mut idx) = state.last_visited_ws_idx
        && *idx > target_ws
    {
        *idx -= 1;
    }
    if target_ws < state.last_visited_pane_per_ws.len() {
        state.last_visited_pane_per_ws.remove(target_ws);
    }

    sync_focus(state);
    state.needs_redraw = true;
}

// ── Take pane ──

pub fn handle_pane_take(state: &mut AppState, _action: &WmAction) {
    let candidates = crate::collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneTake {
            candidates,
            focus_after: false,
        };
        state.needs_redraw = true;
    }
}

pub fn handle_pane_take_and_focus(state: &mut AppState, _action: &WmAction) {
    let candidates = crate::collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneTake {
            candidates,
            focus_after: true,
        };
        state.needs_redraw = true;
    }
}

/// Move a pane from wherever it is to the bottom of the active column.
pub fn handle_take_pane(state: &mut AppState, action: &WmAction) {
    let WmAction::TakePane {
        pane_id,
        focus_after,
    } = action
    else {
        return;
    };
    let target = *pane_id;
    let should_focus = *focus_after;

    // 1. If already at the bottom of the active column → no-op.
    if let Some(ws) = state.session.active_workspace() {
        let active_col = ws.scrolling.active_column_idx;
        if active_col < ws.scrolling.columns.len()
            && ws.scrolling.columns[active_col]
                .panes
                .last()
                .map(|p| p.id.0)
                == Some(target)
        {
            return;
        }
    }

    let active_ws_idx = state.session.active_workspace_idx;

    // 2. Try to find and remove from scrolling columns.
    let removed =
        crate::find_pane_location(&state.session, target).and_then(|(src_ws, src_col, src_idx)| {
            state
                .session
                .workspaces
                .get_mut(src_ws)
                .and_then(|ws| {
                    if src_col < ws.scrolling.columns.len() {
                        ws.scrolling.remove_pane(src_col, src_idx)
                    } else {
                        None
                    }
                })
                .map(|pane| (src_ws, pane))
        });

    let (src_ws, pane) = match removed {
        Some(r) => r,
        None => {
            // 3. Not in scrolling → try floating panes.
            let mut found: Option<(usize, heca_core::layout::column::Pane)> = None;
            for (ws_idx, ws) in state.session.workspaces.iter_mut().enumerate() {
                if let Some(pos) = ws.floating_panes.iter().position(|f| f.pane.id.0 == target) {
                    let fp = ws.floating_panes.remove(pos);
                    ws.deactivate_floating_panes();
                    ws.floating_is_active = false;
                    found = Some((ws_idx, fp.pane));
                    break;
                }
            }
            match found {
                Some(r) => r,
                None => return,
            }
        }
    };

    // 4. Add to active workspace's active column at the bottom.
    let active_col = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    // Need next_id for potential new column; grab before mutable borrow.
    let new_col_id = ColumnId(state.session.next_id());
    if let Some(ws) = state.session.active_workspace_mut() {
        if active_col < ws.scrolling.columns.len() {
            ws.scrolling
                .add_pane_to_column(active_col, None, pane, should_focus);
        } else if ws.scrolling.columns.is_empty() {
            // No columns at all — create one.
            let col = Column::new(new_col_id, pane, ColumnWidth::Proportion(0.85));
            ws.scrolling.add_column(None, col, should_focus);
        } else {
            // Fallback: add to last column.
            let last = ws.scrolling.columns.len() - 1;
            ws.scrolling
                .add_pane_to_column(last, None, pane, should_focus);
        }
    }

    // 5. Clean up empty source workspace if cross-workspace.
    if src_ws != active_ws_idx {
        crate::destroy_empty_workspace(state, src_ws);
    }

    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_create_workspace(state: &mut AppState, _action: &WmAction) {
    let working_area = state
        .session
        .active_workspace()
        .map(|ws| {
            heca_core::layout::types::Rectangle::new(
                ws.scrolling.working_area.loc,
                ws.scrolling.working_area.size,
            )
        })
        .unwrap_or_else(|| {
            heca_core::layout::types::Rectangle::new(
                heca_core::layout::types::Point::default(),
                state.session.viewport_size,
            )
        });
    state.session.add_workspace(working_area);
    let new_idx = state.session.workspaces.len() - 1;
    switch_workspace_tracked(state, new_idx);
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
    state.session.add_pane(pane, None, true);
    state
        .backends
        .insert(next_id, Box::new(FakeBackend::new(80, 24)));
    while state.last_visited_pane_per_ws.len() <= new_idx {
        state.last_visited_pane_per_ws.push(None);
    }
    sync_focus(state);
    state.needs_redraw = true;
}

pub fn handle_rename_workspace(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.session.active_workspace_idx;
    let current_name = state
        .session
        .active_workspace()
        .and_then(|ws| ws.name.clone())
        .unwrap_or_default();
    state.input_mode = InputMode::Rename {
        target: RenameTarget::Workspace(ws_idx),
        buffer: current_name,
    };
    state.needs_redraw = true;
}

// ── Sidebar / Chrome ──

pub fn handle_sidebar_left(state: &mut AppState, _action: &WmAction) {
    state.sidebar.left_visible = !state.sidebar.left_visible;
    update_session_viewport(state);
    state.needs_redraw = true;
}

pub fn handle_sidebar_right(state: &mut AppState, _action: &WmAction) {
    state.sidebar.right_visible = !state.sidebar.right_visible;
    update_session_viewport(state);
    state.needs_redraw = true;
}

pub fn handle_sidebar_focus(state: &mut AppState, _action: &WmAction) {
    state.sidebar.left_visible = true;
    state.sidebar.left_width = 200.0;
    state.input_mode = InputMode::SidebarNav;
    update_session_viewport(state);
    state.sidebar_tree.rebuild(
        &state.session,
        state.last_visited_ws_idx,
        state.focused_pane,
        &state.last_visited_pane_per_ws,
    );
    state.needs_redraw = true;
}

pub fn handle_sidebar_up(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        let is_collapsed = !state.sidebar.left_visible || state.sidebar.left_width < 80.0;
        if is_collapsed {
            state.sidebar_tree.cursor_up_collapsed();
        } else {
            state.sidebar_tree.cursor_up();
        }
        state.needs_redraw = true;
    }
}

pub fn handle_sidebar_down(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        let is_collapsed = !state.sidebar.left_visible || state.sidebar.left_width < 80.0;
        if is_collapsed {
            state.sidebar_tree.cursor_down_collapsed();
        } else {
            state.sidebar_tree.cursor_down();
        }
        state.needs_redraw = true;
    }
}

pub fn handle_sidebar_left_nav(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        state.sidebar_tree.collapse();
        state.needs_redraw = true;
    }
}

pub fn handle_sidebar_right_nav(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        let item = state.sidebar_tree.current_item().cloned();
        match &item {
            Some(sidebar::SidebarItem::Pane { pane_id }) => {
                let target_pane_id = heca_core::layout::PaneId(*pane_id);
                let target_ws = state
                    .session
                    .workspaces
                    .iter()
                    .position(|ws| ws.find_pane(target_pane_id).is_some());
                if let Some(ws_idx) = target_ws {
                    if ws_idx != state.session.active_workspace_idx {
                        switch_workspace_tracked(state, ws_idx);
                    }
                    focus_pane_by_id(state, *pane_id);
                }
                // Stay in sidebar mode; only Enter/Esc exit
            }
            Some(sidebar::SidebarItem::FloatingPane { .. }) => {}
            Some(sidebar::SidebarItem::Workspace { .. }) => {
                let ws_idx = state
                    .sidebar_tree
                    .cursor_workspace_index()
                    .unwrap_or(state.session.active_workspace_idx);
                if ws_idx != state.session.active_workspace_idx {
                    switch_workspace_tracked(state, ws_idx);
                }
                let next_id = state.session.next_id();
                let pane = LayoutPane::new(PaneId(next_id), pane_name(next_id));
                state.session.add_pane(pane, None, true);
                state
                    .backends
                    .insert(next_id, Box::new(FakeBackend::new(80, 24)));
                sync_focus(state);
                // Stay in sidebar mode; only Enter/Esc exit
            }
            _ => {
                state.sidebar_tree.expand();
            }
        }
        state.needs_redraw = true;
    }
}

pub fn handle_sidebar_expand_toggle(state: &mut AppState, _action: &WmAction) {
    if matches!(state.input_mode, InputMode::SidebarNav) {
        let item = state.sidebar_tree.current_item().cloned();
        match &item {
            Some(sidebar::SidebarItem::Pane { pane_id }) => {
                let target_pane_id = heca_core::layout::PaneId(*pane_id);
                let target_ws = state
                    .session
                    .workspaces
                    .iter()
                    .position(|ws| ws.find_pane(target_pane_id).is_some());
                if let Some(ws_idx) = target_ws {
                    if ws_idx != state.session.active_workspace_idx {
                        switch_workspace_tracked(state, ws_idx);
                    }
                    focus_pane_by_id(state, *pane_id);
                }
                state.input_mode = InputMode::Normal;
            }
            Some(sidebar::SidebarItem::FloatingPane { .. }) => {}
            _ => {
                state.sidebar_tree.toggle_expand();
            }
        }
        state.needs_redraw = true;
    }
}

// ── System ──

pub fn handle_command_palette(state: &mut AppState, _action: &WmAction) {
    state.needs_redraw = true;
}

// ── External commands ──

pub fn handle_spawn_command(state: &mut AppState, action: &WmAction) {
    let WmAction::SpawnCommand { command } = action else {
        return;
    };
    // Create a new pane with the command as its title.
    // In the future this will spawn a real PTY via portable-pty.
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), command.clone());
    state.session.add_pane(pane, None, true);
    state
        .backends
        .insert(next_id, Box::new(FakeBackend::new(80, 24)));
    sync_focus(state);
    state.needs_redraw = true;
}

// ── Mode ──

pub fn handle_enter_mode(state: &mut AppState, action: &WmAction) {
    let WmAction::EnterMode { name } = action else {
        return;
    };
    state.input_mode = InputMode::Mode { name: name.clone() };
    state.needs_redraw = true;
}

// ── Config ──

pub fn handle_reload_config(state: &mut AppState, _action: &WmAction) {
    state.pending_reload = true;
}
