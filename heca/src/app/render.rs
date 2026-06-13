//! Render and viewport helpers.
//!
//! These helpers keep low-level pane rendering and viewport synchronization out
//! of `main.rs` while preserving the current render pipeline behavior.

use crate::app::terminal_host::prepare_terminal_mount;
use crate::app_state::{AppState, InputMode};
use crate::chrome::{ChromeConfig, DEFAULT_TAB_BAR_HEIGHT, DEFAULT_STATUS_BAR_HEIGHT};
use crate::{mouse, sidebar};
use heca_core::layout::{Point, Rectangle, Size};
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::terminal::{TerminalRenderer, TerminalStyle};
use heca_grid_ui::drag::DragSurfaceId;
use heca_renderer::text::{TextBox, TextRenderer};

fn pane_content_rect(px: f32, py: f32, pw: f32, ph: f32, border_width: f32) -> Option<(f32, f32, f32, f32)> {
    let inset = border_width.max(1.0);
    let content_w = (pw - inset * 2.0).max(0.0);
    let content_h = (ph - inset * 2.0).max(0.0);
    if content_w <= 0.0 || content_h <= 0.0 {
        return None;
    }

    Some((px + inset, py + inset, content_w, content_h))
}

fn stable_tiled_content_rect(
    px: f32,
    py: f32,
    pw: f32,
    ph: f32,
    theme_border_width: f32,
) -> Option<Rectangle> {
    pane_content_rect(px, py, pw, ph, theme_border_width).map(|(x, y, w, h)| {
        Rectangle::new(Point::new(x as f64, y as f64), Size::new(w as f64, h as f64))
    })
}

fn stable_floating_content_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    theme_border_width: f32,
) -> Option<Rectangle> {
    pane_content_rect(x, y, w, h, theme_border_width * 2.0).map(|(cx, cy, cw, ch)| {
        Rectangle::new(
            Point::new(cx as f64, cy as f64),
            Size::new(cw as f64, ch as f64),
        )
    })
}

fn pane_scissor_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale_factor: f64,
    physical_size: winit::dpi::PhysicalSize<u32>,
) -> Option<(u32, u32, u32, u32)> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }

    let scale = scale_factor as f32;
    let left = (x.max(0.0) * scale).floor() as u32;
    let top = (y.max(0.0) * scale).floor() as u32;
    let right = ((x + w).max(0.0) * scale).ceil() as u32;
    let bottom = ((y + h).max(0.0) * scale).ceil() as u32;

    let clipped_left = left.min(physical_size.width);
    let clipped_top = top.min(physical_size.height);
    let clipped_right = right.min(physical_size.width);
    let clipped_bottom = bottom.min(physical_size.height);
    let clipped_width = clipped_right.saturating_sub(clipped_left);
    let clipped_height = clipped_bottom.saturating_sub(clipped_top);

    if clipped_width == 0 || clipped_height == 0 {
        return None;
    }

    Some((clipped_left, clipped_top, clipped_width, clipped_height))
}

fn rect_to_text_box(rect: Rectangle) -> TextBox {
    TextBox {
        x: rect.loc.x as f32,
        y: rect.loc.y as f32,
        w: rect.size.w as f32,
        h: rect.size.h as f32,
    }
}

struct TerminalRenderPassContext<'a> {
    text_renderer: &'a mut TextRenderer,
    primitive_renderer: &'a mut PrimitiveRenderer,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    view: &'a wgpu::TextureView,
    encoder: &'a mut wgpu::CommandEncoder,
    scale_factor: f64,
    surface_physical_size: winit::dpi::PhysicalSize<u32>,
}

fn render_terminal_mount(
    render_ctx: TerminalRenderPassContext<'_>,
    terminal_style: TerminalStyle<'_>,
    mount: crate::app::terminal_host::TerminalMount,
) {
    let TerminalRenderPassContext {
        text_renderer,
        primitive_renderer,
        device,
        queue,
        view,
        encoder,
        scale_factor,
        surface_physical_size,
    } = render_ctx;
    let content_box = rect_to_text_box(mount.content_rect);
    let (clip_x, clip_y, clip_w, clip_h) =
        (content_box.x, content_box.y, content_box.w, content_box.h);
    text_renderer.set_clip(Some([clip_x, clip_y, clip_w, clip_h]));
    let mut terminal_renderer = TerminalRenderer::new(text_renderer, primitive_renderer);
    terminal_renderer.render_snapshot(
        &mount.snapshot,
        content_box,
        terminal_style,
    );
    text_renderer.set_clip(None);

    let clip_rect = pane_scissor_rect(
        clip_x,
        clip_y,
        clip_w,
        clip_h,
        scale_factor,
        surface_physical_size,
    );
    primitive_renderer.render_clipped(device, view, encoder, clip_rect);
    text_renderer.render(queue, view, encoder);
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
    state.text_renderer.begin_frame();
    state.text_renderer.set_damage(None);
    state.text_renderer.set_clip(None);

    let bg = theme.background.to_linear_f32x4();
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

    let chrome_text = crate::chrome::CHROME_TEXT_SIZE;
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
    let active_pane_id = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|pane| pane.id)
        .or(state.focused_pane);
    let focus_title = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
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
        .render(&state.queue, &view, &mut encoder);

    let theme_border = theme.border.to_f32x4();
    let border_width = theme.border_width;
    let accent_color = theme.accent.to_f32x4();

    let pane_positions = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.panes_with_positions())
        .unwrap_or_default();
    let mut active_tiled_borders = Vec::new();
    let mut active_float_borders = Vec::new();

    let ws_geometries = state.session.workspace_geometries();
    let ws_offset = ws_geometries
        .first()
        .map(|(_, rect)| (rect.loc.x as f32, rect.loc.y as f32))
        .unwrap_or((0.0, 0.0));
    let surface_physical_size = state.window.inner_size();

    for (pane_id, rect) in &pane_positions {
        let px = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
        let py = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
        let pw = rect.size.w as f32;
        let ph = rect.size.h as f32;
        let is_active = active_pane_id == Some(*pane_id);
        let bcolor = if is_active {
            accent_color
        } else {
            [theme_border[0], theme_border[1], theme_border[2], 0.5]
        };
        let pane_border_width = border_width;
        let focus_outline_width = border_width.max(2.0);
        let content_rect = stable_tiled_content_rect(px, py, pw, ph, border_width);

        let pane_mount = if let Some(content_rect) = content_rect {
            prepare_terminal_mount(&mut state.backends, *pane_id, content_rect)
        } else {
            None
        };

        if let Some(content_rect) = content_rect {
            if let Some(mount) = pane_mount {
                render_terminal_mount(
                    TerminalRenderPassContext {
                        text_renderer: &mut state.text_renderer,
                        primitive_renderer: &mut state.primitive_renderer,
                        device: &state.device,
                        queue: &state.queue,
                        view: &view,
                        encoder: &mut encoder,
                        scale_factor: state.scale_factor,
                        surface_physical_size,
                    },
                    TerminalStyle {
                        font_size: theme.terminal_font_size,
                        font_family: &theme.terminal_font_family,
                        italic_font_family: &theme.terminal_italic_font_family,
                    },
                    mount,
                );
            } else {
                let content_box = rect_to_text_box(content_rect);
                state
                    .primitive_renderer
                    .draw_rect(content_box.x, content_box.y, content_box.w, content_box.h, [0.118, 0.118, 0.180, 1.0]);
            }
        } else {
            state
                .primitive_renderer
                .draw_rect(px, py, pw, ph, [0.118, 0.118, 0.180, 1.0]);
        }

        if is_active {
            active_tiled_borders.push((px, py, pw, ph, bcolor, pane_border_width, focus_outline_width));
        } else {
            state
                .primitive_renderer
                .draw_border(px, py, pw, ph, bcolor, pane_border_width);
        }
    }
    for (px, py, pw, ph, color, width, outline_width) in active_tiled_borders {
        state
            .primitive_renderer
            .draw_border(px, py, pw, ph, color, width);
        state
            .primitive_renderer
            .draw_outline(px, py, pw, ph, color, outline_width);
    }
    state
        .primitive_renderer
        .render(&state.device, &view, &mut encoder);
    state
        .text_renderer
        .render(&state.queue, &view, &mut encoder);

    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = h - chrome.status_bar_height;
    let sidebar_h = sidebar_bottom - sidebar_top;

    if let Some(ws) = state.session.active_workspace() {
        for float in &ws.floating_panes {
            let fx = float.position.x as f32 + pane_area.loc.x as f32 + ws_offset.0;
            let fy = float.position.y as f32 + pane_area.loc.y as f32 + ws_offset.1;
            let fw = float.size.w as f32;
            let fh = float.size.h as f32;
            let is_focused = active_pane_id == Some(float.pane.id);
            let fborder = if is_focused {
                theme.float_focus.to_f32x4()
            } else {
                theme.float_accent.to_f32x4()
            };
            let float_border_width = border_width * 2.0;
            let float_outline_width = (border_width * 2.0).max(3.0);
            let content_rect = stable_floating_content_rect(fx, fy, fw, fh, border_width);
            let pane_mount = if let Some(content_rect) = content_rect {
                prepare_terminal_mount(&mut state.backends, float.pane.id, content_rect)
            } else {
                None
            };
            if let Some(content_rect) = content_rect {
                if let Some(mount) = pane_mount {
                    render_terminal_mount(
                        TerminalRenderPassContext {
                            text_renderer: &mut state.text_renderer,
                            primitive_renderer: &mut state.primitive_renderer,
                            device: &state.device,
                            queue: &state.queue,
                            view: &view,
                            encoder: &mut encoder,
                            scale_factor: state.scale_factor,
                            surface_physical_size,
                        },
                        TerminalStyle {
                            font_size: theme.terminal_font_size,
                            font_family: &theme.terminal_font_family,
                            italic_font_family: &theme.terminal_italic_font_family,
                        },
                        mount,
                    );
                } else {
                    let content_box = rect_to_text_box(content_rect);
                    state.primitive_renderer.draw_rect(
                        content_box.x,
                        content_box.y,
                        content_box.w,
                        content_box.h,
                        theme.float_background.to_f32x4(),
                    );
                }
            } else {
                state.primitive_renderer.draw_rect(
                    fx,
                    fy,
                    fw,
                    fh,
                    theme.float_background.to_f32x4(),
                );
            }
            if is_focused {
                active_float_borders.push((fx, fy, fw, fh, fborder, float_border_width, float_outline_width));
            } else {
                state
                    .primitive_renderer
                    .draw_border(fx, fy, fw, fh, fborder, float_border_width);
            }
        }
    }
    for (fx, fy, fw, fh, color, width, outline_width) in active_float_borders {
        state
            .primitive_renderer
            .draw_border(fx, fy, fw, fh, color, width);
        state
            .primitive_renderer
            .draw_outline(fx, fy, fw, fh, color, outline_width);
    }

    let pane_area_rect = heca_core::layout::Rectangle::new(
        heca_core::layout::Point::new(pane_area.loc.x, pane_area.loc.y),
        heca_core::layout::Size::new(pane_area.size.w, pane_area.size.h),
    );

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
    let drag_hover = state
        .mouse
        .drag_ctx
        .surface(DragSurfaceId::LeftSidebar)
        .and_then(|s| s.hover_item);
    let drag_source = state
        .mouse
        .drag_ctx
        .surface(DragSurfaceId::LeftSidebar)
        .and_then(|s| s.source_item);
    let drag_source_bg = theme.sidebar_drag_source_bg.to_f32x4();
    let drag_source_border = theme.sidebar_drag_source_border.to_f32x4();
    if chrome.left_sidebar_width >= crate::chrome::SIDEBAR_EXPANDED_THRESHOLD {
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
            active_pane_id,
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
            active_pane_id,
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

    if let Some(label) = state
        .mouse
        .drag_ctx
        .surface(DragSurfaceId::LeftSidebar)
        .and_then(|s| s.ghost_label.as_ref())
    {
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
    if chrome.right_sidebar_width >= crate::chrome::SIDEBAR_EXPANDED_THRESHOLD {
        state.text_renderer.queue_text(
            "Details",
            rsx + 8.0,
            sidebar_top + 8.0,
            chrome_text,
            theme.foreground.to_f32x4(),
        );
    }

    mouse::render_detached_pane(state, pane_area_rect);
    mouse::render_insert_hint(state, pane_area_rect);

    if let Some(candidates) = state.input_mode.candidates() {
        let letter_size = 48.0f32;
        let label_color = [1.0, 0.9, 0.3, 0.9];
        for (ch, target_id) in candidates {
            if Some(*target_id) == active_pane_id {
                continue;
            }
            let mut found = false;
            for (pane_id, rect) in &pane_positions {
                if *pane_id == *target_id {
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
                    if float.pane.id == *target_id {
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
        .render(&state.queue, &view, &mut encoder);

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
    use heca_core::layout::PaneId;

    #[test]
    fn status_mode_parts_formats_rename_and_take() {
        assert_eq!(
            status_mode_parts(&InputMode::Rename {
                target: RenameTarget::Pane(PaneId(7)),
                buffer: "term".to_string(),
            }),
            ("RENAME", ": term_".to_string())
        );

        assert_eq!(
            status_mode_parts(&InputMode::PaneTake {
                candidates: vec![("a".chars().next().expect("candidate label"), PaneId(1))],
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
