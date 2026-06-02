//! Named action handlers — one per `WmAction` variant.
//!
//! Each handler is a plain `fn(&mut AppState, &WmAction)` that performs the
//! action.  Parameterized variants destructure their fields from the enum;
//! unit variants ignore the `_action` parameter.

use crate::app_state::{AppState, InputMode, RenameTarget};
use crate::input::WmAction;
use crate::sidebar;
use crate::{
    collect_all_pane_candidates, destroy_empty_workspace,
    find_pane_location, focus_pane_by_id, move_pane_to_column, move_pane_to_workspace_column,
    pane_name, switch_workspace_tracked, sync_focus, update_session_viewport,
};
use heca_core::backend::FakeBackend;
use heca_core::layout::animation::AnimationConfig;
use heca_core::layout::{Column, ColumnId, ColumnWidth, Pane as LayoutPane, PaneId};
use heca_core::layout::types::Point;

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
    if let Some(prev_pane) = state.last_visited_pane_per_ws.get(ws_idx).copied().flatten() {
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
    let WmAction::FocusPane { pane_id } = action else { return };
    focus_pane_by_id(state, *pane_id);
}

pub fn handle_focus_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusWorkspace { ws_idx } = action else { return };
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
    state.backends.insert(backend_id, Box::new(FakeBackend::new(80, 24)));
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
        ws.scrolling
            .add_pane_to_column(col_idx, None, pane, true);
    }
    state.backends.insert(backend_id, Box::new(FakeBackend::new(80, 24)));
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

pub fn handle_swap_param(state: &mut AppState, action: &WmAction) {
    let WmAction::Swap { a_id, b_id } = action else { return };
    if a_id == b_id { return; }

    eprintln!("[swap] handler called: a={} b={}", a_id, b_id);

    // Find both panes' locations.
    let a_loc = find_pane_location(&state.session, *a_id);
    let b_loc = find_pane_location(&state.session, *b_id);
    eprintln!("[swap] a_loc={:?} b_loc={:?}", a_loc, b_loc);
    let ((aws, acol, api), (bws, bcol, bpi)) = match (a_loc, b_loc) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            eprintln!("[swap] one or both panes not found; aborting");
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
            eprintln!("[swap] same workspace & same column: col={} first_pi={} second_pi={}", acol, first_pi, second_pi);
            if let Some(ws) = state.session.workspaces.get_mut(aws) {
                if acol >= ws.scrolling.columns.len() { eprintln!("[swap] column index out of range"); return; }
                if second_pi >= ws.scrolling.columns[acol].panes.len() || first_pi >= ws.scrolling.columns[acol].panes.len() {
                    eprintln!("[swap] pane indices out of range for column");
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
                eprintln!("[swap] swapped panes in column {} at indices {} and {}", acol, first_pi, second_pi);
            }
        } else {
            // Different columns, same workspace: perform reinsert-first to avoid column deletion
            // (which causes a slide animation). We insert placeholders at both target spots,
            // then remove the original panes and replace placeholders with the real panes,
            // animating each pane from its old position to its new position.
            eprintln!("[swap] same workspace different columns (reinsert-first): acol={} bcol={} (api={},bpi={})", acol, bcol, api, bpi);

            let a_pid = *a_id;
            let b_pid = *b_id;

            // Capture old positions (immutable borrow) so we can animate from them later.
            let old_rects = state.session.workspaces.get(aws).map(|ws| ws.scrolling.panes_with_positions()).unwrap_or_default();
            let old_a_rect = old_rects.iter().find(|(pid, _)| *pid == heca_core::layout::PaneId(a_pid)).map(|(_, r)| *r);
            let old_b_rect = old_rects.iter().find(|(pid, _)| *pid == heca_core::layout::PaneId(b_pid)).map(|(_, r)| *r);

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
                let mut inserts = vec![(bcol, bpi, placeholder_a_pid, new_col_for_a), (acol, api, placeholder_b_pid, new_col_for_b)];
                // Insert in descending col index order so earlier inserts don't shift later targets.
                inserts.sort_by(|a, b| b.0.cmp(&a.0));

                for (col_pos, pane_idx, ph_pid, new_cid) in inserts {
                    if col_pos < ws.scrolling.columns.len() {
                        let insert_idx = pane_idx.min(ws.scrolling.columns[col_pos].panes.len());
                        let placeholder = LayoutPane::new(PaneId(ph_pid), pane_name(ph_pid));
                        ws.scrolling.add_pane_to_column(col_pos, Some(insert_idx), placeholder, true);
                        eprintln!("[swap] inserted placeholder id={} at col {} idx {} (target)", ph_pid, col_pos, insert_idx);
                    } else {
                        let pos = col_pos.min(ws.scrolling.columns.len());
                        let placeholder = LayoutPane::new(PaneId(ph_pid), pane_name(ph_pid));
                        ws.scrolling.add_column(Some(pos), Column::new(new_cid, placeholder, ColumnWidth::Proportion(0.5)), true);
                        eprintln!("[swap] created placeholder column at pos {} with id {:?} (placeholder id={}) (target)", pos, new_cid, ph_pid);
                    }
                }

                // After placeholders exist, find and remove the original panes by id.
                // Removing order: higher column index first to avoid index invalidation.
                let mut found_a = None;
                let mut found_b = None;
                for (ci, col) in ws.scrolling.columns.iter().enumerate() {
                    for (pi, pane) in col.panes.iter().enumerate() {
                        if pane.id == heca_core::layout::PaneId(a_pid) { found_a = Some((ci, pi)); }
                        if pane.id == heca_core::layout::PaneId(b_pid) { found_b = Some((ci, pi)); }
                    }
                }

                // Decide removal order by column index (descending)
                let mut removes = vec![];
                if let Some((ci, pi)) = found_a { removes.push((ci, pi, a_pid)); }
                if let Some((ci, pi)) = found_b { removes.push((ci, pi, b_pid)); }
                removes.sort_by(|x, y| y.0.cmp(&x.0));

                let mut removed_a: Option<LayoutPane> = None;
                let mut removed_b: Option<LayoutPane> = None;

                for (ci, pi, pid) in removes {
                    if ci < ws.scrolling.columns.len() {
                        if let Some(removed) = ws.scrolling.remove_pane(ci, pi) {
                            eprintln!("[swap] removed original pane id={} from col {} idx {}", pid, ci, pi);
                            if pid == a_pid { removed_a = Some(removed); }
                            else if pid == b_pid { removed_b = Some(removed); }
                        }
                    }
                }

                // Now replace placeholders with the removed panes and animate from old positions.
                // Helper to find placeholder by pane id. Clamp large dx/dy for cross-workspace cases.
                let vw = state.session.viewport_size.w;
                let vh = state.session.viewport_size.h;
                let max_dx = vw * 0.9;
                let max_dy = vh * 0.9;

                let replace_placeholder = |ws: &mut heca_core::layout::workspace::Workspace, ph_id: u64, new_pane: LayoutPane, old_rect_opt: Option<heca_core::layout::types::Rectangle>| {
                    let mut found = None;
                    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
                        for (pi, pane) in col.panes.iter().enumerate() {
                            if pane.id.0 == ph_id { found = Some((ci, pi)); break; }
                        }
                        if found.is_some() { break; }
                    }
                    if let Some((ci, pi)) = found {
                        ws.scrolling.columns[ci].panes[pi] = new_pane;
                        ws.scrolling.columns[ci].active_pane_idx = pi;
                        ws.scrolling.columns[ci].compute_pane_sizes(ws.scrolling.working_area.size.h, ws.scrolling.options.gaps);
                        ws.scrolling.update_all_column_widths();

                        if let Some(old_rect) = old_rect_opt {
                            if let Some((_, new_rect)) = ws.scrolling.panes_with_positions().into_iter().find(|(pid, _)| *pid == ws.scrolling.columns[ci].panes[pi].id) {
                                let mut dx = old_rect.loc.x - new_rect.loc.x;
                                let mut dy = old_rect.loc.y - new_rect.loc.y;
                                // Clamp extreme values so panes don't dash across the whole window when
                                // swapping between workspaces (coordinate frames may differ).
                                if dx > max_dx { dx = max_dx; } else if dx < -max_dx { dx = -max_dx; }
                                if dy > max_dy { dy = max_dy; } else if dy < -max_dy { dy = -max_dy; }
                                ws.scrolling.columns[ci].panes[pi].animate_move_from(Point::new(dx, dy), AnimationConfig::default());
                                eprintln!("[swap] animated pane id={} from ({:.1},{:.1}) to ({:.1},{:.1}) offset=({:.1},{:.1})", ws.scrolling.columns[ci].panes[pi].id.0, old_rect.loc.x, old_rect.loc.y, new_rect.loc.x, new_rect.loc.y, dx, dy);
                            }
                        }
                        eprintln!("[swap] replaced placeholder {} at ws col {} idx {}", ph_id, ci, pi);
                    } else {
                        eprintln!("[swap] placeholder {} not found for replacement", ph_id);
                    }
                };

                if let Some(a_pane) = removed_a { replace_placeholder(ws, placeholder_a_pid, a_pane, old_a_rect); }
                if let Some(b_pane) = removed_b { replace_placeholder(ws, placeholder_b_pid, b_pane, old_b_rect); }
            }
        }
    } else {
        // Different workspaces: exchange panes between workspaces preserving column identity when possible.
        // Implement reinsert-first semantics to avoid temporary column deletion and to allow
        // nicer pane-level animations (move pane visually from old location to new).
        eprintln!("[swap] cross-workspace (reinsert-first): A ws={} col={} idx={}  B ws={} col={} idx={}", aws, acol, api, bws, bcol, bpi);

        // Capture meta info before mutation.
        let a_col_id = state.session.workspaces.get(aws).and_then(|ws| ws.scrolling.columns.get(acol).map(|c| c.id));
        let b_col_id = state.session.workspaces.get(bws).and_then(|ws| ws.scrolling.columns.get(bcol).map(|c| c.id));
        let a_col_pos = acol;
        let b_col_pos = bcol;
        let a_col_len = state.session.workspaces.get(aws).and_then(|ws| ws.scrolling.columns.get(acol).map(|c| c.panes.len())).unwrap_or(0);
        let b_col_len = state.session.workspaces.get(bws).and_then(|ws| ws.scrolling.columns.get(bcol).map(|c| c.panes.len())).unwrap_or(0);

        // Capture old pane render positions (used to compute animation offsets).
        let old_a_rect = state.session.workspaces.get(aws)
            .map(|ws| ws.scrolling.panes_with_positions().into_iter().find(|(pid, _)| *pid == heca_core::layout::PaneId(*a_id)).map(|(_, r)| r))
            .flatten();
        let old_b_rect = state.session.workspaces.get(bws)
            .map(|ws| ws.scrolling.panes_with_positions().into_iter().find(|(pid, _)| *pid == heca_core::layout::PaneId(*b_id)).map(|(_, r)| r))
            .flatten();

        // Pre-generate ids for placeholder panes and for any recreated columns.
        let placeholder_a_pid = state.session.next_id();
        let placeholder_b_pid = state.session.next_id();
        let new_col_for_a = ColumnId(state.session.next_id());
        let new_col_for_b = ColumnId(state.session.next_id());

        // Step A -> B: insert a placeholder in B's workspace where A should land.
        if let Some(ws_b) = state.session.workspaces.get_mut(bws) {
            if b_col_len > 1 {
                let target_col = b_col_pos.min(ws_b.scrolling.columns.len().saturating_sub(1));
                let insert_idx = bpi.min(ws_b.scrolling.columns[target_col].panes.len());
                let placeholder = LayoutPane::new(PaneId(placeholder_a_pid), pane_name(placeholder_a_pid));
                ws_b.scrolling.add_pane_to_column(target_col, Some(insert_idx), placeholder, true);
                eprintln!("[swap] placed placeholder for A at ws={} col={} idx={} (placeholder id={})", bws, target_col, insert_idx, placeholder_a_pid);
            } else {
                let pos = b_col_pos.min(ws_b.scrolling.columns.len());
                let placeholder = LayoutPane::new(PaneId(placeholder_a_pid), pane_name(placeholder_a_pid));
                ws_b.scrolling.add_column(Some(pos), Column::new(new_col_for_a, placeholder, ColumnWidth::Proportion(0.5)), true);
                eprintln!("[swap] created column placeholder for A at ws={} pos={} (placeholder id={})", bws, pos, placeholder_a_pid);
            }
        } else {
            eprintln!("[swap] target workspace {} missing when placing placeholder for A", bws);
            return;
        }

        // Now remove the original A from its workspace.
        let removed_a = {
            let ws_a = match state.session.workspaces.get_mut(aws) { Some(w) => w, None => { eprintln!("[swap] source workspace {} missing when removing A", aws); return; } };
            if acol < ws_a.scrolling.columns.len() {
                eprintln!("[swap] removing A from ({},{})", aws, acol);
                ws_a.scrolling.remove_pane(acol, api)
            } else { None }
        };
        if removed_a.is_none() { eprintln!("[swap] failed to remove pane A"); return; }
        let removed_a = removed_a.unwrap();

        // Replace placeholder in B's workspace with removed A and animate from old position.
        if let Some(ws_b) = state.session.workspaces.get_mut(bws) {
            // Find the placeholder we inserted.
            let mut found = None;
            for (ci, col) in ws_b.scrolling.columns.iter().enumerate() {
                for (pi, p) in col.panes.iter().enumerate() {
                    if p.id.0 == placeholder_a_pid { found = Some((ci, pi)); break; }
                }
                if found.is_some() { break; }
            }
            if let Some((ci, pi)) = found {
                // Replace placeholder with the removed pane.
                ws_b.scrolling.columns[ci].panes[pi] = removed_a;
                ws_b.scrolling.columns[ci].active_pane_idx = pi;
                // Recompute sizes and widths.
                ws_b.scrolling.columns[ci].compute_pane_sizes(ws_b.scrolling.working_area.size.h, ws_b.scrolling.options.gaps);
                ws_b.scrolling.update_all_column_widths();

                // Compute animation offset if we have old position info.
                if let Some(old_rect) = old_a_rect {
                    let new_rect_opt = ws_b.scrolling.panes_with_positions().into_iter().find(|(pid, _)| *pid == ws_b.scrolling.columns[ci].panes[pi].id);
                    if let Some((_, new_rect)) = new_rect_opt {
                        let dx = old_rect.loc.x - new_rect.loc.x;
                        let dy = old_rect.loc.y - new_rect.loc.y;
                        ws_b.scrolling.columns[ci].panes[pi].animate_move_from(Point::new(dx, dy), AnimationConfig::default());
                        eprintln!("[swap] animated A from ({:.1},{:.1}) to ({:.1},{:.1}) offset=({:.1},{:.1})", old_rect.loc.x, old_rect.loc.y, new_rect.loc.x, new_rect.loc.y, dx, dy);
                    }
                }
                eprintln!("[swap] placed A into ws={} at col={} idx={}", bws, ci, pi);
            } else {
                eprintln!("[swap] placeholder for A not found in ws={}", bws);
            }
        }

        // Step B -> A: insert placeholder in A's workspace.
        if let Some(ws_a) = state.session.workspaces.get_mut(aws) {
            if a_col_len > 1 {
                let target_col = a_col_pos.min(ws_a.scrolling.columns.len().saturating_sub(1));
                let insert_idx = api.min(ws_a.scrolling.columns[target_col].panes.len());
                let placeholder = LayoutPane::new(PaneId(placeholder_b_pid), pane_name(placeholder_b_pid));
                ws_a.scrolling.add_pane_to_column(target_col, Some(insert_idx), placeholder, true);
                eprintln!("[swap] placed placeholder for B at ws={} col={} idx={} (placeholder id={})", aws, target_col, insert_idx, placeholder_b_pid);
            } else {
                let pos = a_col_pos.min(ws_a.scrolling.columns.len());
                let placeholder = LayoutPane::new(PaneId(placeholder_b_pid), pane_name(placeholder_b_pid));
                ws_a.scrolling.add_column(Some(pos), Column::new(new_col_for_b, placeholder, ColumnWidth::Proportion(0.5)), true);
                eprintln!("[swap] created column placeholder for B at ws={} pos={} (placeholder id={})", aws, pos, placeholder_b_pid);
            }
        } else {
            eprintln!("[swap] source workspace {} missing when placing placeholder for B", aws);
            return;
        }

        // Now remove the original B from its workspace.
        let removed_b = {
            let ws_b = match state.session.workspaces.get_mut(bws) { Some(w) => w, None => { eprintln!("[swap] target workspace {} missing when removing B", bws); return; } };
            if bcol < ws_b.scrolling.columns.len() {
                eprintln!("[swap] removing B from ({},{})", bws, bcol);
                ws_b.scrolling.remove_pane(bcol, bpi)
            } else { None }
        };
        if removed_b.is_none() { eprintln!("[swap] failed to remove pane B"); return; }
        let removed_b = removed_b.unwrap();

        // Replace placeholder in A's workspace with removed B and animate from old position.
        if let Some(ws_a) = state.session.workspaces.get_mut(aws) {
            let mut found = None;
            for (ci, col) in ws_a.scrolling.columns.iter().enumerate() {
                for (pi, p) in col.panes.iter().enumerate() {
                    if p.id.0 == placeholder_b_pid { found = Some((ci, pi)); break; }
                }
                if found.is_some() { break; }
            }
            if let Some((ci, pi)) = found {
                ws_a.scrolling.columns[ci].panes[pi] = removed_b;
                ws_a.scrolling.columns[ci].active_pane_idx = pi;
                ws_a.scrolling.columns[ci].compute_pane_sizes(ws_a.scrolling.working_area.size.h, ws_a.scrolling.options.gaps);
                ws_a.scrolling.update_all_column_widths();

                if let Some(old_rect) = old_b_rect {
                    let new_rect_opt = ws_a.scrolling.panes_with_positions().into_iter().find(|(pid, _)| *pid == ws_a.scrolling.columns[ci].panes[pi].id);
                    if let Some((_, new_rect)) = new_rect_opt {
                        let dx = old_rect.loc.x - new_rect.loc.x;
                        let dy = old_rect.loc.y - new_rect.loc.y;
                        ws_a.scrolling.columns[ci].panes[pi].animate_move_from(Point::new(dx, dy), AnimationConfig::default());
                        eprintln!("[swap] animated B from ({:.1},{:.1}) to ({:.1},{:.1}) offset=({:.1},{:.1})", old_rect.loc.x, old_rect.loc.y, new_rect.loc.x, new_rect.loc.y, dx, dy);
                    }
                }
                eprintln!("[swap] placed B into ws={} at col={} idx={}", aws, ci, pi);
            } else {
                eprintln!("[swap] placeholder for B not found in ws={}", aws);
            }
        }
    }


    eprintln!("[swap] final locations: a_loc={:?} b_loc={:?}", find_pane_location(&state.session, *a_id), find_pane_location(&state.session, *b_id));
    state.needs_redraw = true;
}

pub fn handle_move_param(state: &mut AppState, action: &WmAction) {
    let WmAction::Move { pane_id, target_col } = action else { return };
    eprintln!("[move] handler called: pane={} target_col={}", pane_id, target_col);
    if let Some((ws_idx, col_idx, _)) = find_pane_location(&state.session, *pane_id) {
        eprintln!("[move] found pane at ws={} col_idx={}", ws_idx, col_idx);
        // Ensure the source workspace is active before calling move_pane_to_column,
        // which operates on the active workspace.
        if state.session.active_workspace_idx != ws_idx {
            eprintln!("[move] switching active workspace from {} to {}", state.session.active_workspace_idx, ws_idx);
            switch_workspace_tracked(state, ws_idx);
        }
        eprintln!("[move] calling move_pane_to_column pane={} src_col={} dst_col={}", pane_id, col_idx, target_col);
        move_pane_to_column(state, *pane_id, col_idx, *target_col);
        eprintln!("[move] move_pane_to_column completed for pane={}", pane_id);
    } else {
        eprintln!("[move] pane not found: {}", pane_id);
    }
    state.needs_redraw = true;
}

pub fn handle_move_pane_to_workspace(state: &mut AppState, action: &WmAction) {
    let WmAction::MovePaneToWorkspace { pane_id, ws_idx } = action else { return };
    eprintln!("[move-ws] handler called: pane={} target_ws={}", pane_id, ws_idx);
    if let Some((current_ws, current_col, _)) = find_pane_location(&state.session, *pane_id)
        && current_ws != *ws_idx
    {
        eprintln!("[move-ws] found pane at ws={} col={}", current_ws, current_col);
        if state.session.active_workspace_idx != current_ws {
            eprintln!("[move-ws] switching active workspace from {} to {}", state.session.active_workspace_idx, current_ws);
            switch_workspace_tracked(state, current_ws);
        }
        eprintln!("[move-ws] calling move_pane_to_workspace_column pane={} target_ws={} target_col={}", pane_id, ws_idx, current_col);
        move_pane_to_workspace_column(state, *pane_id, *ws_idx, current_col);
        eprintln!("[move-ws] move_pane_to_workspace_column completed for pane={}", pane_id);
    } else {
        eprintln!("[move-ws] pane not found or already in target workspace: pane={} target_ws={}", pane_id, ws_idx);
    }
    state.needs_redraw = true;
}

pub fn handle_move_pane_to_column(state: &mut AppState, action: &WmAction) {
    let WmAction::MovePaneToColumn { pane_id, ws_idx, col_idx } = action else { return };
    eprintln!("[move-col] handler called: pane={} target_ws={} target_col={}", pane_id, ws_idx, col_idx);
    if let Some((current_ws, current_col, _)) = find_pane_location(&state.session, *pane_id) {
        eprintln!("[move-col] found pane at ws={} col={}", current_ws, current_col);
        if current_ws == *ws_idx {
            if state.session.active_workspace_idx != current_ws {
                eprintln!("[move-col] switching active workspace from {} to {}", state.session.active_workspace_idx, current_ws);
                switch_workspace_tracked(state, current_ws);
            }
            eprintln!("[move-col] calling move_pane_to_column pane={} src_col={} dst_col={}", pane_id, current_col, col_idx);
            move_pane_to_column(state, *pane_id, current_col, *col_idx);
            eprintln!("[move-col] move_pane_to_column completed for pane={}", pane_id);
        } else {
            if state.session.active_workspace_idx != current_ws {
                eprintln!("[move-col] switching active workspace from {} to {}", state.session.active_workspace_idx, current_ws);
                switch_workspace_tracked(state, current_ws);
            }
            eprintln!("[move-col] calling move_pane_to_workspace_column pane={} target_ws={} target_col={}", pane_id, ws_idx, col_idx);
            move_pane_to_workspace_column(state, *pane_id, *ws_idx, *col_idx);
            eprintln!("[move-col] move_pane_to_workspace_column completed for pane={}", pane_id);
        }
    } else {
        eprintln!("[move-col] pane not found: {}", pane_id);
    }
    state.needs_redraw = true;
}

pub fn handle_resize(state: &mut AppState, action: &WmAction) {
    let WmAction::Resize { target, axis, amount } = action else { return };
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
    let WmAction::ResizeTo { target, width, height } = action else { return };
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
            if let Some(idx) = ws.floating_panes.iter().position(|f| f.pane.id.0 == pane_id) {
                let float = ws.floating_panes.remove(idx);
                let orig_col = float.original_column_idx;
                let orig_pane = float.original_pane_idx;
                if let Some(col_idx) = orig_col {
                    if col_idx < ws.scrolling.columns.len() {
                        let target_idx = orig_pane
                            .unwrap_or(0)
                            .min(ws.scrolling.columns[col_idx].panes.len());
                        ws.scrolling.columns[col_idx].panes.insert(target_idx, float.pane);
                        ws.scrolling.columns[col_idx].active_pane_idx = target_idx;
                        ws.scrolling.active_column_idx = col_idx;
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
                        Column::new(
                            ColumnId(pane_id),
                            float.pane,
                            ColumnWidth::Proportion(0.5),
                        ),
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
                let fw = wa.size.w * 0.75;
                let fh = wa.size.h * 0.75;
                let fx = wa.loc.x + (wa.size.w - fw) / 2.0;
                let fy = wa.loc.y + (wa.size.h - fh) / 2.0;
                ws.floating_panes.push(heca_core::layout::workspace::FloatingPane {
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
        state.backends.insert(next_id, Box::new(FakeBackend::new(80, 24)));
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
        state.input_mode = InputMode::PaneSwap { candidates, focus_after: false };
        state.needs_redraw = true;
    }
}

pub fn handle_swap_and_focus_pane(state: &mut AppState, _action: &WmAction) {
    let candidates = collect_all_pane_candidates(&state.session);
    if !candidates.is_empty() {
        state.input_mode = InputMode::PaneSwap { candidates, focus_after: true };
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

pub fn handle_float_at(state: &mut AppState, action: &WmAction) {
    let WmAction::FloatAt { pane_id, x, y, width, height } = action else { return };
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
            ws.floating_panes.push(heca_core::layout::workspace::FloatingPane {
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
    let WmAction::ClosePaneById { pane_id } = action else { return };
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
        state.backends.insert(next_id, Box::new(FakeBackend::new(80, 24)));
        sync_focus(state);
    } else {
        sync_focus(state);
    }
    state.needs_redraw = true;
}

pub fn handle_rename_target(state: &mut AppState, action: &WmAction) {
    let WmAction::RenameTarget { pane_id, name } = action else { return };
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
    state.backends.insert(next_id, Box::new(FakeBackend::new(80, 24)));
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
                state.backends.insert(next_id, Box::new(FakeBackend::new(80, 24)));
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
            _ => {
                state.sidebar_tree.toggle_expand();
            }
        }
        state.needs_redraw = true;
    }
}

// ── System ──

pub fn handle_command_palette(state: &mut AppState, _action: &WmAction) {
    eprintln!("Command palette triggered (not yet implemented)");
    state.needs_redraw = true;
}

// ── External commands ──

pub fn handle_spawn_command(state: &mut AppState, action: &WmAction) {
    let WmAction::SpawnCommand { command } = action else { return };
    // Create a new pane with the command as its title.
    // In the future this will spawn a real PTY via portable-pty.
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), command.clone());
    state.session.add_pane(pane, None, true);
    state.backends.insert(next_id, Box::new(FakeBackend::new(80, 24)));
    sync_focus(state);
    state.needs_redraw = true;
}

// ── Mode ──

pub fn handle_enter_mode(state: &mut AppState, action: &WmAction) {
    let WmAction::EnterMode { name } = action else { return };
    state.input_mode = InputMode::Mode { name: name.clone() };
    state.needs_redraw = true;
}

// ── Config ──

pub fn handle_reload_config(state: &mut AppState, _action: &WmAction) {
    eprintln!("[config] reload requested");
    state.pending_reload = true;
}
