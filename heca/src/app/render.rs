//! Render and viewport helpers.
//!
//! These helpers keep low-level pane rendering and viewport synchronization out
//! of `main.rs` while preserving the current render pipeline behavior.

use crate::app_state::{AppState, InputMode};
use crate::chrome::{ChromeConfig, DEFAULT_TAB_BAR_HEIGHT, DEFAULT_STATUS_BAR_HEIGHT};
use crate::{mouse, sidebar};
use heca_core::backend::BackendRenderData;
use heca_grid_ui::drag::DragSurfaceId;
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

        primitive_renderer.draw_rect(px, py, pw, ph, [0.0, 0.0, 0.0, 1.0]);

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

        let cursor_x = px + *cursor_col as f32 * cell_w;
        let cursor_y = py + *cursor_row as f32 * cell_h;
        primitive_renderer.draw_rect(cursor_x, cursor_y, cell_w, cell_h, [1.0, 1.0, 1.0, 0.7]);
    }
}

/// Human-readable status mode label and suffix for the status bar.
pub(crate) fn status_mode_parts(input_mode: &InputMode) -> (&'static str, String) {
    match input_mode {
        InputMode::Normal => ("NORMAL", String::new()),
        InputMode::Prefix => ("PREFIX", String::new()),
        InputMode::PaneSelect { .. } => ("SELECT", String::new()),
        InputMode::PaneSwap { focus_after, .. } => {
            if *focus_after {
                ("SWAP+FOCUS", String::new())
            } else {
                ("SWAP", String::new())
            }
        }
        InputMode::SidebarNav => ("SIDEBAR", String::new()),
        InputMode::Rename { buffer, .. } => ("RENAME", format!(": {}_", buffer)),
        InputMode::Chord { sequence } => ("CHORD", format!(" w→{}", sequence.join("→"))),
        InputMode::Mode { name } => ("MODE", format!(" {} → ?", name)),
        InputMode::ConfirmDelete { message, .. } => ("CONFIRM", format!(" {} ", message)),
        InputMode::PaneTake { focus_after, .. } => {
            if *focus_after {
                ("TAKE+", " pick a pane → ".to_string())
            } else {
                ("TAKE", " pick a pane → ".to_string())
            }
        }
    }
}

/// Render the full frame for the current app state.
pub(crate) fn render_frame(state: &mut AppState) {
    if !state.needs_redraw {
        return;
    }
    state.needs_redraw = false;

    let surface_texture = match state.surface.get_current_texture() {
        Ok(t) => t,
        Err(wgpu::SurfaceError::Lost) => {
            state
                .surface
                .configure(&state.device, &state.surface_config);
            return;
        }
        Err(wgpu::SurfaceError::OutOfMemory) => std::process::exit(1),
        Err(_) => {
            return;
        }
    };

    let view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let phys_size = state.window.inner_size();
    let scale = state.scale_factor as f32;
    let w = phys_size.width as f32 / scale;
    let h = phys_size.height as f32 / scale;

    let theme = &state.theme;

    let chrome = ChromeConfig {
        tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
        status_bar_height: DEFAULT_STATUS_BAR_HEIGHT,
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
    let pane_area = chrome.content_rect(w, h);

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render"),
        });

    let bg = theme.background.to_f32x4();
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: bg[0] as f64,
                    g: bg[1] as f64,
                    b: bg[2] as f64,
                    a: bg[3] as f64,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    let chrome_text = 14.0f32;
    let tb = &chrome;
    let side_bg = if theme.name == "Catppuccin Mocha" {
        [0.067, 0.067, 0.106, 1.0]
    } else {
        [0.953, 0.957, 0.973, 1.0]
    };
    state
        .primitive_renderer
        .draw_rect(0.0, 0.0, w, tb.tab_bar_height, side_bg);

    let sb_y = h - tb.status_bar_height;
    state
        .primitive_renderer
        .draw_rect(0.0, sb_y, w, tb.status_bar_height, side_bg);
    let pane_count = state
        .session
        .active_workspace()
        .map(|ws| {
            ws.scrolling
                .columns
                .iter()
                .map(|c| c.panes.len())
                .sum::<usize>()
        })
        .unwrap_or(0);
    let focus_title = state
        .session
        .active_workspace()
        .and_then(|ws| ws.scrolling.active_pane())
        .map(|p| p.title.as_str())
        .unwrap_or("—");
    let (mode_str, rename_hint) = status_mode_parts(&state.input_mode);
    let status = format!(
        "{} panes | {} | {}{}",
        pane_count, focus_title, mode_str, rename_hint
    );
    let status_text_y = sb_y + (tb.status_bar_height - chrome_text) / 2.0;
    state.text_renderer.queue_text(
        &status,
        8.0,
        status_text_y,
        chrome_text,
        theme.foreground.to_f32x4(),
    );

    state
        .primitive_renderer
        .render(&state.device, &view, &mut encoder);
    state
        .text_renderer
        .render(&state.device, &state.queue, &view, &mut encoder);

    let theme_border = theme.border.to_f32x4();
    let border_width = theme.border_width;
    let accent_color = theme.accent.to_f32x4();

    let pane_positions = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.panes_with_positions())
        .unwrap_or_default();

    let ws_geometries = state.session.workspace_geometries();
    let ws_offset = ws_geometries
        .first()
        .map(|(_, rect)| (rect.loc.x as f32, rect.loc.y as f32))
        .unwrap_or((0.0, 0.0));

    for (pane_id, rect) in &pane_positions {
        let px = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
        let py = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
        let pw = rect.size.w as f32;
        let ph = rect.size.h as f32;
        let is_active = state.focused_pane == Some(pane_id.0);
        let bcolor = if is_active {
            accent_color
        } else {
            [theme_border[0], theme_border[1], theme_border[2], 0.5]
        };

        if let Some(backend) = state.backends.get(pane_id.0) {
            let data = backend.render_data();
            render_backend_data(
                &data,
                px,
                py,
                pw,
                ph,
                &mut state.text_renderer,
                &mut state.primitive_renderer,
                theme,
            );
        } else {
            state
                .primitive_renderer
                .draw_rect(px, py, pw, ph, [0.118, 0.118, 0.180, 1.0]);
        }

        let pane_name = state
            .session
            .active_workspace()
            .and_then(|ws| ws.find_pane(*pane_id))
            .map(|p| p.title.as_str())
            .unwrap_or("?");
        let name_size = (pw.min(ph) * 0.25).clamp(24.0, 72.0);
        let name_color = if is_active {
            [1.0, 1.0, 1.0, 0.9]
        } else {
            [1.0, 1.0, 1.0, 0.4]
        };
        let name_w = name_size * pane_name.len() as f32 * 0.6;
        let name_x = px + (pw - name_w) / 2.0;
        let name_y = py + (ph - name_size) / 2.0;
        state
            .text_renderer
            .queue_text(pane_name, name_x, name_y, name_size, name_color);

        state
            .primitive_renderer
            .draw_border(px, py, pw, ph, bcolor, border_width);
    }
    state
        .primitive_renderer
        .render(&state.device, &view, &mut encoder);
    state
        .text_renderer
        .render(&state.device, &state.queue, &view, &mut encoder);

    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = h - chrome.status_bar_height;
    let sidebar_h = sidebar_bottom - sidebar_top;

    state.primitive_renderer.draw_rect(
        0.0,
        sidebar_top,
        chrome.left_sidebar_width,
        sidebar_h,
        side_bg,
    );
    state.primitive_renderer.draw_border(
        chrome.left_sidebar_width - 1.0,
        sidebar_top,
        1.0,
        sidebar_h,
        theme.border.to_f32x4(),
        1.0,
    );
    let candidates = state.input_mode.candidates();
    let drag_hover = state.mouse.drag_ctx.surface(DragSurfaceId::LeftSidebar).and_then(|s| s.hover_item);
    let drag_source = state.mouse.drag_ctx.surface(DragSurfaceId::LeftSidebar).and_then(|s| s.source_item);
    let drag_source_bg = theme.sidebar_drag_source_bg.to_f32x4();
    let drag_source_border = theme.sidebar_drag_source_border.to_f32x4();
    if chrome.left_sidebar_width >= 80.0 {
        sidebar::render_sidebar_expanded(
            &mut state.sidebar_tree,
            0.0,
            sidebar_top,
            chrome.left_sidebar_width,
            sidebar_h,
            matches!(state.input_mode, InputMode::SidebarNav),
            theme.accent.to_f32x4(),
            theme.foreground.to_f32x4(),
            [side_bg[0] * 2.0, side_bg[1] * 2.0, side_bg[2] * 2.0, 0.6],
            [
                theme.accent.to_f32x4()[0],
                theme.accent.to_f32x4()[1],
                theme.accent.to_f32x4()[2],
                0.5,
            ],
            candidates,
            state.focused_pane,
            &mut state.text_renderer,
            &mut state.primitive_renderer,
            drag_hover,
            drag_source,
            drag_source_bg,
            drag_source_border,
            state.mouse.sidebar_hovered_btn_idx,
            theme.sidebar_label_font_size,
            theme.sidebar_button_font_size,
        );
    } else {
        sidebar::render_sidebar_collapsed(
            &mut state.sidebar_tree,
            0.0,
            sidebar_top,
            chrome.left_sidebar_width,
            sidebar_h,
            matches!(state.input_mode, InputMode::SidebarNav),
            theme.accent.to_f32x4(),
            theme.foreground.to_f32x4(),
            [
                theme.accent.to_f32x4()[0],
                theme.accent.to_f32x4()[1],
                theme.accent.to_f32x4()[2],
                0.5,
            ],
            [side_bg[0] * 2.0, side_bg[1] * 2.0, side_bg[2] * 2.0, 0.6],
            candidates,
            state.focused_pane,
            &mut state.text_renderer,
            &mut state.primitive_renderer,
            drag_hover,
            drag_source,
            drag_source_bg,
            drag_source_border,
            state.mouse.sidebar_hovered_btn_idx,
            theme.sidebar_label_font_size,
            theme.sidebar_button_font_size,
        );
    }

    if let Some(label) = state.mouse.drag_ctx.surface(DragSurfaceId::LeftSidebar).and_then(|s| s.ghost_label.as_ref()) {
        let ghost_w = label.width;
        let ghost_h = 22.0;
        let ghost_x = label.x + 10.0;
        let ghost_y = label.y - ghost_h / 2.0;

        state.primitive_renderer.draw_rect(
            ghost_x,
            ghost_y,
            ghost_w,
            ghost_h,
            theme.sidebar_drag_ghost_bg.to_f32x4(),
        );
        state.primitive_renderer.draw_border(
            ghost_x,
            ghost_y,
            ghost_w,
            ghost_h,
            theme.sidebar_drag_source_border.to_f32x4(),
            1.5,
        );
        state.text_renderer.queue_text(
            &label.text,
            ghost_x + 6.0,
            ghost_y + 4.0,
            13.0,
            theme.sidebar_drag_ghost_fg.to_f32x4(),
        );
    }

    let rsx = w - chrome.right_sidebar_width;
    state.primitive_renderer.draw_rect(
        rsx,
        sidebar_top,
        chrome.right_sidebar_width,
        sidebar_h,
        side_bg,
    );
    state.primitive_renderer.draw_border(
        rsx,
        sidebar_top,
        1.0,
        sidebar_h,
        theme.border.to_f32x4(),
        1.0,
    );
    if chrome.right_sidebar_width >= 80.0 {
        state.text_renderer.queue_text(
            "Details",
            rsx + 8.0,
            sidebar_top + 8.0,
            chrome_text,
            theme.foreground.to_f32x4(),
        );
    }

    if let Some(ws) = state.session.active_workspace() {
        for float in &ws.floating_panes {
            let fx = float.position.x as f32 + pane_area.loc.x as f32 + ws_offset.0;
            let fy = float.position.y as f32 + pane_area.loc.y as f32 + ws_offset.1;
            let fw = float.size.w as f32;
            let fh = float.size.h as f32;
            let is_focused = state.focused_pane == Some(float.pane.id.0);
            let fborder = if is_focused {
                theme.float_focus.to_f32x4()
            } else {
                theme.float_accent.to_f32x4()
            };
            if let Some(backend) = state.backends.get(float.pane.id.0) {
                let data = backend.render_data();
                render_backend_data(
                    &data,
                    fx,
                    fy,
                    fw,
                    fh,
                    &mut state.text_renderer,
                    &mut state.primitive_renderer,
                    theme,
                );
            } else {
                state.primitive_renderer.draw_rect(
                    fx,
                    fy,
                    fw,
                    fh,
                    theme.float_background.to_f32x4(),
                );
            }
            state
                .primitive_renderer
                .draw_border(fx, fy, fw, fh, fborder, border_width * 2.0);
            let float_name = &float.pane.title;
            let f_name_size = (fw.min(fh) * 0.25).clamp(24.0, 72.0);
            let f_name_color = if is_focused {
                [1.0, 1.0, 1.0, 0.9]
            } else {
                [1.0, 1.0, 1.0, 0.4]
            };
            let f_name_w = f_name_size * float_name.len() as f32 * 0.6;
            let f_name_x = fx + (fw - f_name_w) / 2.0;
            let f_name_y = fy + (fh - f_name_size) / 2.0;
            state.text_renderer.queue_text(
                float_name,
                f_name_x,
                f_name_y,
                f_name_size,
                f_name_color,
            );
        }
    }

    let pane_area_rect = heca_core::layout::Rectangle::new(
        heca_core::layout::Point::new(pane_area.loc.x, pane_area.loc.y),
        heca_core::layout::Size::new(pane_area.size.w, pane_area.size.h),
    );
    mouse::render_detached_pane(state, pane_area_rect);
    mouse::render_insert_hint(state, pane_area_rect);

    if let Some(candidates) = state.input_mode.candidates() {
        let letter_size = 48.0f32;
        let label_color = [1.0, 0.9, 0.3, 0.9];
        for (ch, target_id) in candidates {
            if Some(*target_id) == state.focused_pane {
                continue;
            }
            let mut found = false;
            for (pane_id, rect) in &pane_positions {
                if pane_id.0 == *target_id {
                    let px = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
                    let py = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
                    let pw = rect.size.w as f32;
                    let ph = rect.size.h as f32;
                    let lx = px + (pw - letter_size * 0.6) / 2.0;
                    let ly = py + (ph - letter_size) / 2.0;
                    let label = ch.to_string();
                    state
                        .text_renderer
                        .queue_text(&label, lx, ly, letter_size, label_color);
                    found = true;
                    break;
                }
            }
            if !found && let Some(ws) = state.session.active_workspace() {
                for float in &ws.floating_panes {
                    if float.pane.id.0 == *target_id {
                        let fx = float.position.x as f32 + pane_area.loc.x as f32;
                        let fy = float.position.y as f32 + pane_area.loc.y as f32;
                        let fw = float.size.w as f32;
                        let fh = float.size.h as f32;
                        let lx = fx + (fw - letter_size * 0.6) / 2.0;
                        let ly = fy + (fh - letter_size) / 2.0;
                        let label = ch.to_string();
                        state
                            .text_renderer
                            .queue_text(&label, lx, ly, letter_size, label_color);
                        found = true;
                        break;
                    }
                }
            }
            let _ = found;
        }
    }

    state
        .primitive_renderer
        .render(&state.device, &view, &mut encoder);
    state
        .text_renderer
        .render(&state.device, &state.queue, &view, &mut encoder);

    state.queue.submit(std::iter::once(encoder.finish()));
    surface_texture.present();
}

/// Update session viewport to match current chrome/content area size.
pub(crate) fn update_session_viewport(state: &mut AppState) {
    let phys = state.window.inner_size();
    let win_w = phys.width as f32 / state.scale_factor as f32;
    let win_h = phys.height as f32 / state.scale_factor as f32;
    let chrome = ChromeConfig {
        tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
        status_bar_height: DEFAULT_STATUS_BAR_HEIGHT,
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
    let new_size = heca_core::layout::types::Size::new(pane_area.size.w, pane_area.size.h);
    state.session.update_viewport(new_size);
}

#[cfg(test)]
mod tests {
    use super::status_mode_parts;
    use crate::app_state::{InputMode, RenameTarget};
    use crate::input::WmAction;

    #[test]
    fn status_mode_parts_formats_rename_and_take() {
        assert_eq!(
            status_mode_parts(&InputMode::Rename {
                target: RenameTarget::Pane(7),
                buffer: "term".to_string(),
            }),
            ("RENAME", ": term_".to_string())
        );

        assert_eq!(
            status_mode_parts(&InputMode::PaneTake {
                candidates: vec![("a".chars().next().expect("candidate label"), 1)],
                focus_after: true,
            }),
            ("TAKE+", " pick a pane → ".to_string())
        );

        assert_eq!(
            status_mode_parts(&InputMode::ConfirmDelete {
                message: "Delete pane?".to_string(),
                action: Box::new(WmAction::ClosePane),
                resume_sidebar: false,
            }),
            ("CONFIRM", " Delete pane? ".to_string())
        );
    }
}
