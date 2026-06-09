//! Mouse rendering helpers.
//!
//! This module owns the temporary drag visuals rendered on top of pane content.

use crate::app_state::AppState;
use heca_core::layout::types::PaneInsertTarget;

use super::hit_test_pane_excluding;

/// Render the detached pane during interactive move.
pub(crate) fn render_detached_pane(state: &mut AppState, pane_area: (f32, f32, f32, f32)) {
    let det = match &state.mouse.detached_pane {
        Some(d) => d,
        None => return,
    };

    let px = pane_area.0 + det.render_pos.x as f32;
    let py = pane_area.1 + det.render_pos.y as f32;
    let pw = det.size.w as f32;
    let ph = det.size.h as f32;
    let accent = state.theme.accent.to_f32x4();

    state
        .primitive_renderer
        .draw_rect(px, py, pw, ph, [0.118, 0.118, 0.180, 0.7]);

    let pane_name = &det.pane.title;
    let name_size = (pw.min(ph) * 0.25).clamp(24.0, 72.0);
    let name_w = name_size * pane_name.len() as f32 * 0.6;
    let name_x = px + (pw - name_w) / 2.0;
    let name_y = py + (ph - name_size) / 2.0;
    state
        .text_renderer
        .queue_text(pane_name, name_x, name_y, name_size, [1.0, 1.0, 1.0, 0.9]);

    let border_w = state.theme.border_width * 3.0;
    state
        .primitive_renderer
        .draw_border(px, py, pw, ph, accent, border_w);
}

/// Render the insert hint (placeholder rectangle) during interactive move.
///
/// In move mode, shows a thin strip indicating where the pane will be inserted.
/// In swap mode, highlights the full target pane rectangle.
pub(crate) fn render_insert_hint(state: &mut AppState, pane_area: (f32, f32, f32, f32)) {
    // Swap mode: highlight the full target pane under cursor.
    if matches!(&state.mouse.drag_state, crate::app_state::DragState::InteractiveMove { swap: true, .. })
        && state.mouse.detached_pane.is_none()
    {
        render_swap_target_hint(state, pane_area);
        return;
    }

    let hint = match state.mouse.insert_hint {
        Some(h) => h,
        None => return,
    };
    let ws = match state.session.active_workspace() {
        Some(w) => w,
        None => return,
    };

    let wa = ws.scrolling.working_area;
    let gaps = ws.scrolling.options.gaps;
    let view_pos = ws.scrolling.view_pos();

    let (rx, ry, rw, rh) = match hint {
        PaneInsertTarget::NewColumn(col_idx) => {
            let space_x = if col_idx == 0 {
                0.0
            } else if col_idx < ws.scrolling.columns.len() {
                let prev_x = ws.scrolling.column_x(col_idx - 1);
                let prev_w = ws
                    .scrolling
                    .column_widths
                    .get(col_idx - 1)
                    .copied()
                    .unwrap_or(0.0);
                prev_x + prev_w + gaps * 0.5
            } else if ws.scrolling.columns.is_empty() {
                0.0
            } else {
                let last_x = ws.scrolling.column_x(ws.scrolling.columns.len() - 1);
                let last_w = ws.scrolling.column_widths.last().copied().unwrap_or(0.0);
                last_x + last_w - gaps * 0.5
            };

            let x = space_x - view_pos;
            let y = wa.loc.y + wa.size.h * 0.2;
            let w = 24.0f64;
            let h = wa.size.h * 0.6;
            (x, y, w, h)
        }
        PaneInsertTarget::InColumn { col_idx, pane_idx } => {
            let col_len = ws.scrolling.columns.len();
            let space_x = if col_idx < col_len {
                ws.scrolling.column_x(col_idx)
            } else if col_len == 0 {
                0.0
            } else {
                ws.scrolling.column_x(col_len - 1)
            };

            let col_w = ws
                .scrolling
                .column_widths
                .get(col_idx)
                .copied()
                .unwrap_or(0.0);

            let space_y = if col_idx < ws.scrolling.columns.len() {
                let col = &ws.scrolling.columns[col_idx];
                if pane_idx == col.panes.len() && pane_idx > 0 {
                    let last_pane_idx = pane_idx - 1;
                    let last_pane_y = ws.scrolling.pane_y_in_column(col_idx, last_pane_idx);
                    let last_pane_h = col
                        .pane_sizes
                        .get(last_pane_idx)
                        .map(|s| s.h)
                        .unwrap_or(0.0);
                    last_pane_y + last_pane_h
                } else {
                    ws.scrolling.pane_y_in_column(col_idx, pane_idx)
                }
            } else {
                0.0
            };

            let x = space_x - view_pos;
            let y = space_y;
            let w = col_w;
            let h = 24.0f64;
            (x, y, w, h)
        }
    };

    let mut render_x = pane_area.0 + rx as f32;
    let mut render_y = pane_area.1 + ry as f32;
    let mut render_w = rw as f32;
    let mut render_h = rh as f32;

    let inset = (state.theme.border_width * 2.0).max(2.0);
    render_x = (render_x + inset).min(pane_area.0 + pane_area.2 - 4.0);
    render_y = (render_y + inset).min(pane_area.1 + pane_area.3 - 4.0);
    render_w = (render_w - 2.0 * inset).max(4.0);
    render_h = (render_h - 2.0 * inset).max(4.0);

    let ca_r = pane_area.0 + pane_area.2;
    let ca_b = pane_area.1 + pane_area.3;
    if render_x < pane_area.0 {
        let overflow = pane_area.0 - render_x;
        render_w = (render_w - overflow).max(4.0);
        render_x = pane_area.0;
    }
    if render_x >= ca_r {
        return;
    }
    if render_x + render_w > ca_r {
        render_w = (ca_r - render_x).max(4.0);
    }
    if render_y < pane_area.1 {
        let overflow = pane_area.1 - render_y;
        render_h = (render_h - overflow).max(4.0);
        render_y = pane_area.1;
    }
    if render_y >= ca_b {
        return;
    }
    if render_y + render_h > ca_b {
        render_h = (ca_b - render_y).max(4.0);
    }

    let accent = state.theme.accent.to_f32x4();
    state.primitive_renderer.draw_rect(
        render_x,
        render_y,
        render_w,
        render_h,
        [accent[0], accent[1], accent[2], 0.15],
    );
    state.primitive_renderer.draw_border(
        render_x,
        render_y,
        render_w,
        render_h,
        [accent[0], accent[1], accent[2], 0.6],
        2.0,
    );
}

/// Render a full-pane highlight for swap mode, showing the exact target pane bounds.
fn render_swap_target_hint(state: &mut AppState, pane_area: (f32, f32, f32, f32)) {
    // Exclude the dragged source pane so it doesn't highlight itself.
    let exclude_id = match state.mouse.drag_state {
        crate::app_state::DragState::InteractiveMove { swap: true, _pane_id, .. } => Some(_pane_id),
        _ => None,
    };
    let target_id = match hit_test_pane_excluding(state, state.mouse.pos, exclude_id) {
        Some(id) => id,
        None => return,
    };

    let ws = match state.session.active_workspace() {
        Some(w) => w,
        None => return,
    };

    // Find the target pane's bounds.
    let mut found = None;
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        for (pi, pane) in col.panes.iter().enumerate() {
            if pane.id.0 == target_id {
                found = Some((ci, pi));
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let (col_idx, pane_idx) = match found {
        Some(v) => v,
        None => return,
    };

    let col_x = ws.scrolling.column_x(col_idx) - ws.scrolling.view_pos();
    let pane_y = ws.scrolling.pane_y_in_column(col_idx, pane_idx);
    let col_w = ws.scrolling.column_widths.get(col_idx).copied().unwrap_or(0.0);
    let pane_h = ws.scrolling.columns.get(col_idx)
        .and_then(|c| c.pane_sizes.get(pane_idx))
        .map(|s| s.h)
        .unwrap_or(0.0);

    let rx = pane_area.0 + col_x as f32;
    let ry = pane_area.1 + pane_y as f32;
    let rw = col_w as f32;
    let rh = pane_h as f32;

    // Clip to content area.
    let ca_r = pane_area.0 + pane_area.2;
    let ca_b = pane_area.1 + pane_area.3;
    if rx >= ca_r || ry >= ca_b {
        return;
    }
    let rw = rw.min(ca_r - rx).max(4.0);
    let rh = rh.min(ca_b - ry).max(4.0);

    let accent = state.theme.accent.to_f32x4();
    state.primitive_renderer.draw_rect(
        rx, ry, rw, rh,
        [accent[0], accent[1], accent[2], 0.15],
    );
    state.primitive_renderer.draw_border(
        rx, ry, rw, rh,
        [accent[0], accent[1], accent[2], 0.6],
        4.0,
    );
}
