//! Render and viewport helpers.
//!
//! These helpers keep low-level pane rendering and viewport synchronization out
//! of `main.rs` while preserving the current render pipeline behavior.

use crate::app_state::AppState;
use crate::chrome::ChromeConfig;
use heca_core::backend::BackendRenderData;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;

/// Render a backend's content into a pane rectangle.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_backend_data(
    data: &BackendRenderData,
    px: f32,
    py: f32,
    pw: f32,
    ph: f32,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    _theme: &heca_config::theme::Theme,
) {
    if let BackendRenderData::Terminal {
        lines,
        cursor_col,
        cursor_row,
        cell_w,
        cell_h,
    } = data
    {
        let cell_h = *cell_h;
        let cell_w = *cell_w;

        // Background
        primitive_renderer.draw_rect(px, py, pw, ph, [0.0, 0.0, 0.0, 1.0]);

        // Text
        for (row, line) in lines.iter().enumerate() {
            let y = py + row as f32 * cell_h;
            let mut current_text = String::new();
            let mut current_fg = [1.0f32; 4];
            let mut start_col = 0usize;

            for (col, cell) in line.cells.iter().enumerate() {
                if cell.c == ' ' || cell.c == '\0' {
                    if !current_text.is_empty() {
                        let x = px + start_col as f32 * cell_w;
                        text_renderer.queue_text(&current_text, x, y, cell_h, current_fg);
                        current_text.clear();
                    }
                    start_col = col + 1;
                    continue;
                }

                if col > start_col && cell.fg != current_fg {
                    if !current_text.is_empty() {
                        let x = px + start_col as f32 * cell_w;
                        text_renderer.queue_text(&current_text, x, y, cell_h, current_fg);
                        current_text.clear();
                    }
                    current_fg = cell.fg;
                    start_col = col;
                }

                current_text.push(cell.c);
            }

            if !current_text.is_empty() {
                let x = px + start_col as f32 * cell_w;
                text_renderer.queue_text(&current_text, x, y, cell_h, current_fg);
            }
        }

        // Cursor
        let cursor_x = px + *cursor_col as f32 * cell_w;
        let cursor_y = py + *cursor_row as f32 * cell_h;
        primitive_renderer.draw_rect(cursor_x, cursor_y, cell_w, cell_h, [1.0, 1.0, 1.0, 0.7]);
    }
}

/// Update session viewport to match current chrome/content area size.
pub(crate) fn update_session_viewport(state: &mut AppState) {
    let phys = state.window.inner_size();
    let win_w = phys.width as f32 / state.scale_factor as f32;
    let win_h = phys.height as f32 / state.scale_factor as f32;
    let chrome = ChromeConfig {
        tab_bar_height: 32.0,
        status_bar_height: 24.0,
        left_sidebar_width: if state.sidebar.left_visible {
            state.sidebar.left_width
        } else {
            40.0
        },
        right_sidebar_width: if state.sidebar.right_visible {
            state.sidebar.right_width
        } else {
            40.0
        },
    };
    let pane_area = chrome.content_rect(win_w, win_h);
    let new_size = heca_core::layout::types::Size::new(pane_area.w as f64, pane_area.h as f64);
    state.session.update_viewport(new_size);
}
