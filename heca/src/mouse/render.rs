//! Mouse rendering helpers.
//!
//! This module owns the temporary drag visuals rendered on top of pane content.

use crate::app_state::AppState;
use heca_core::layout::types::InsertPosition;

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
pub(crate) fn render_insert_hint(state: &mut AppState, pane_area: (f32, f32, f32, f32)) {
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
        InsertPosition::NewColumn(col_idx) => {
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
        InsertPosition::InColumn { col_idx, pane_idx } => {
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
