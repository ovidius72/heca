//! Render and viewport helpers.
//!
//! These helpers keep low-level pane rendering and viewport synchronization out
//! of `main.rs` while preserving the current render pipeline behavior.

use crate::app::terminal_host::prepare_terminal_mount;
use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionState};
use crate::app_state::{AppState, InputMode};
use crate::chrome::{ChromeConfig, DEFAULT_TAB_BAR_HEIGHT, DEFAULT_STATUS_BAR_HEIGHT};
use crate::{mouse, sidebar};
use heca_core::layout::{PaneId, Point, Rectangle, Size};
use heca_config::theme::Color;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::terminal::{CaretIndicator, SelectionOverlay, SelectionOverlaySpan, TerminalRenderer, TerminalStyle};
use heca_grid_ui::drag::DragSurfaceId;
use heca_renderer::grid::GridRenderer;
use heca_renderer::text::{TextBox, TextRenderer};

fn pane_content_rect(px: f32, py: f32, pw: f32, ph: f32, border_width: f32) -> Option<(f32, f32, f32, f32)> {
    // Border is drawn inside the pane rect via draw_border. Content must be
    // inset by border_width so terminal fills don't overlap the border stroke.
    let content_w = (pw - border_width * 2.0).max(0.0);
    let content_h = (ph - border_width * 2.0).max(0.0);
    if content_w <= 0.0 || content_h <= 0.0 {
        return None;
    }

    Some((px + border_width, py + border_width, content_w, content_h))
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
    pane_content_rect(x, y, w, h, theme_border_width).map(|(cx, cy, cw, ch)| {
        Rectangle::new(
            Point::new(cx as f64, cy as f64),
            Size::new(cw as f64, ch as f64),
        )
    })
}

fn push_pane_border_rect(
    scene: &mut heca_grid_ui::Scene,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    border_color: [f32; 4],
    border_width: f32,
    border_radius: f32,
) {
    use heca_grid_ui::color::Color as GuiColor;
    use heca_grid_ui::scene::{Border, DrawCommand, RectCmd};
    use heca_grid_ui::{Point as GuiPoint, Rectangle as GuiRect, Size as GuiSize};

    let max_r = w.min(h) * 0.5;
    let radius = border_radius.min(max_r).max(0.0);

    scene.push(DrawCommand::Rect(RectCmd {
        rect: GuiRect::new(
            GuiPoint::new(x as f64, y as f64),
            GuiSize::new(w as f64, h as f64),
        ),
        fill: GuiColor::TRANSPARENT,
        border: Some(Border {
            color: GuiColor::new(
                (border_color[0] * 255.0) as u8,
                (border_color[1] * 255.0) as u8,
                (border_color[2] * 255.0) as u8,
                (border_color[3] * 255.0) as u8,
            ),
            width: border_width,
        }),
        radius,
        glow: None,
        shadow: None,
    }));
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
    /// The scrolling content area; pane content is clipped to it so panes scrolled
    /// partially behind the chrome (sidebars/status) don't bleed under it.
    content_clip: Rectangle,
}

fn render_terminal_mount(
    render_ctx: TerminalRenderPassContext<'_>,
    terminal_style: TerminalStyle<'_>,
    mount: crate::app::terminal_host::TerminalMount,
    selection_overlay: Option<SelectionOverlay>,
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
        content_clip,
    } = render_ctx;
    let content_box = rect_to_text_box(mount.content_rect);
    // Intersect the pane's own rect with the scrolling content area, so a pane
    // scrolled partially under the sidebar/status chrome is cropped at the content
    // edge instead of bleeding into the chrome region.
    let clip_x = content_box.x.max(content_clip.loc.x as f32);
    let clip_y = content_box.y.max(content_clip.loc.y as f32);
    let clip_right =
        (content_box.x + content_box.w).min((content_clip.loc.x + content_clip.size.w) as f32);
    let clip_bottom =
        (content_box.y + content_box.h).min((content_clip.loc.y + content_clip.size.h) as f32);
    let clip_w = (clip_right - clip_x).max(0.0);
    let clip_h = (clip_bottom - clip_y).max(0.0);
    {
        text_renderer.set_clip(Some([clip_x, clip_y, clip_w, clip_h]));
        let mut terminal_renderer = TerminalRenderer::new(text_renderer, primitive_renderer);
        terminal_renderer.render_snapshot(
            &mount.snapshot,
            content_box,
            terminal_style,
        );
        // Draw the host-level selection overlay after cell backgrounds and
        // glyphs so the selected text remains readable, and before the
        // cursor overlay so the cursor is always visible on top.
        if let Some(ref overlay) = selection_overlay {
            terminal_renderer.render_selection_overlay(
                overlay,
                content_box,
                mount.snapshot.cell_w,
                mount.snapshot.cell_h,
            );
        }
        text_renderer.set_clip(None);
    }

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
    {
        let mut terminal_renderer = TerminalRenderer::new(text_renderer, primitive_renderer);
        terminal_renderer.render_cursor_overlay(&mount.snapshot, content_box);
    }
    primitive_renderer.render_clipped(device, view, encoder, clip_rect);
}

/// Build a `SelectionOverlay` for a pane if the shared host selection is
/// active, owned by that pane, and uses the `HostGrid` render mode.
///
/// Returns `None` when the selection is inactive, owned by a different pane,
/// or uses `BackendNative` rendering (the host does not draw backend-native
/// selections).
///
/// The anchor/focus cell coordinates from the selection model are converted
/// into a min/max bounding box here so the renderer crate stays agnostic of
/// selection-model semantics.
fn selection_overlay_for_pane(
    state: &AppState,
    pane_id: PaneId,
    cols: usize,
) -> Option<SelectionOverlay> {
    build_selection_overlay(&state.selection, pane_id, cols, &state.theme.accent)
}

/// Build a `SelectionOverlay` for a pane if the shared host selection is
/// active, owned by that pane, and uses the `HostGrid` render mode.
///
/// Returns `None` when the selection is inactive, owned by a different pane,
/// or uses `BackendNative` rendering (the host does not draw backend-native
/// selections). The anchor/focus cell coordinates are converted into a
/// min/max bounding box so the renderer stays agnostic of selection-model
/// semantics.
///
/// Pure-logic counterpart of `selection_overlay_for_pane` — resolves all
/// `AppState`-dependent lookups (`selection`, `accent`) at its call site so
/// this function can be unit-tested without GPU state.
fn build_selection_overlay(
    selection: &SelectionState,
    pane_id: PaneId,
    cols: usize,
    accent: &Color,
) -> Option<SelectionOverlay> {
    if cols == 0 {
        return None;
    }

    // Caret-only state: return a thin caret indicator (no selection spans).
    if let SelectionState::Caret { owner, row, col } = selection {
        if *owner != SelectionOwner::Pane(pane_id) {
            return None;
        }
        let color = [
            accent.r as f32 / 255.0,
            accent.g as f32 / 255.0,
            accent.b as f32 / 255.0,
            0.25,
        ];
        return Some(
            SelectionOverlay::new(vec![], color)
                .with_caret(CaretIndicator {
                    row: *row,
                    col: *col,
                    is_selection_endpoint: false,
                }),
        );
    }

    let active = selection.active()?;
    if active.owner != SelectionOwner::Pane(pane_id) {
        return None;
    }
    match &active.region {
        SelectionRegion::HostGrid {
            anchor_row,
            anchor_col,
            focus_row,
            focus_col,
        } => {
            let color = [
                accent.r as f32 / 255.0,
                accent.g as f32 / 255.0,
                accent.b as f32 / 255.0,
                0.25,
            ];
            let last_col = cols.saturating_sub(1);
            let mut spans = Vec::new();
            if anchor_row == focus_row {
                spans.push(SelectionOverlaySpan {
                    row: *anchor_row,
                    start_col: *anchor_col.min(focus_col),
                    end_col: (*anchor_col.max(focus_col)).min(last_col),
                });
            } else if anchor_row < focus_row {
                spans.push(SelectionOverlaySpan {
                    row: *anchor_row,
                    start_col: (*anchor_col).min(last_col),
                    end_col: last_col,
                });
                for row in (*anchor_row + 1)..*focus_row {
                    spans.push(SelectionOverlaySpan {
                        row,
                        start_col: 0,
                        end_col: last_col,
                    });
                }
                spans.push(SelectionOverlaySpan {
                    row: *focus_row,
                    start_col: 0,
                    end_col: (*focus_col).min(last_col),
                });
            } else {
                spans.push(SelectionOverlaySpan {
                    row: *focus_row,
                    start_col: (*focus_col).min(last_col),
                    end_col: last_col,
                });
                for row in (*focus_row + 1)..*anchor_row {
                    spans.push(SelectionOverlaySpan {
                        row,
                        start_col: 0,
                        end_col: last_col,
                    });
                }
                spans.push(SelectionOverlaySpan {
                    row: *anchor_row,
                    start_col: 0,
                    end_col: (*anchor_col).min(last_col),
                });
            }
            // Add a prominent caret at the focus (active) end of the selection
            // so the user can see which endpoint will move when they press
            // h/j/k/l or after toggling with `o`.
            Some(
                SelectionOverlay::new(spans, color)
                    .with_caret(CaretIndicator {
                        row: *focus_row,
                        col: *focus_col,
                        is_selection_endpoint: true,
                    }),
            )
        }
        SelectionRegion::BackendNative => None,
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
        InputMode::Selection => ("SELECTION", String::new()),
    }
}

/// Factored chrome render pass: feeds a grid-ui `Scene` through `GridRenderer`
/// and `TextRenderer`. Base layer first, then each overlay segment as its own
/// rects-then-text pass (matching the showcase ordering that avoids overlay
/// text-bleed).
///
/// Takes the renderer fields individually (not `&mut AppState`) because
/// `render_frame` holds `let theme = &state.theme;` across its body.
fn render_chrome(
    grid: &mut GridRenderer,
    text: &mut TextRenderer,
    queue: &wgpu::Queue,
    scene: &heca_grid_ui::Scene,
    view: &wgpu::TextureView,
    encoder: &mut wgpu::CommandEncoder,
) {
    grid.begin_frame();
    grid.set_damage(None);
    grid.set_clip(None);
    heca_renderer::scene::enqueue_scene(grid, text, &scene.base_layer());
    grid.render(queue, view, encoder);
    text.render(queue, view, encoder);
    for overlay in scene.overlay_segments() {
        heca_renderer::scene::enqueue_scene(grid, text, &overlay);
        grid.render(queue, view, encoder);
        text.render(queue, view, encoder);
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
    let scene_view = state.compositor.scene_view();

    let phys_size = state.window.inner_size();
    let scale = state.scale_factor as f32;
    let w = phys_size.width as f32 / scale;
    let h = phys_size.height as f32 / scale;

    let theme = &state.theme;
    let surface_alpha = state.terminal_surface_opacity();

    let chrome = ChromeConfig {
        tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
        status_bar_height: DEFAULT_STATUS_BAR_HEIGHT,
        left_sidebar_width: if state.chrome_state.left_visible() {
            state.chrome_state.left_size()
        } else {
            40.0
        },
        right_sidebar_width: if state.chrome_state.right_visible() {
            state.chrome_state.right_size()
        } else {
            40.0
        },
        sidebar_gap: state.appearance.effective_sidebar_gap(&state.theme),
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
    // Transparent window: clear fully transparent so empty/background areas show
    // the frosted vibrancy at full strength. Chrome panels draw translucent
    // (chrome_alpha) on top; opaque panes/text/borders stay crisp. transparent
    // == false reproduces today's opaque clear exactly.
    let (clear_r, clear_g, clear_b, clear_a) = if state.appearance.is_transparent() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        (bg[0] as f64, bg[1] as f64, bg[2] as f64, bg[3] as f64)
    };
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: scene_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: clear_r,
                    g: clear_g,
                    b: clear_b,
                    a: clear_a,
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
    // Frosted chrome: when transparent, draw chrome backgrounds (tab bar,
    // sidebars, status bar) translucent so the vibrancy shows through. Panes and
    // text stay opaque. (Per-pane translucency comes later, driven by a protocol.)
    let chrome_alpha = state.appearance.chrome_opacity();
    let side_bg = [side_bg[0], side_bg[1], side_bg[2], chrome_alpha];
    let collapsed_sidebar_bg = [
        side_bg[0],
        side_bg[1],
        side_bg[2],
        (0.5 + 0.5 * state.appearance.opacity()).clamp(0.0, 1.0),
    ];
    state
        .primitive_renderer
        .draw_rect(0.0, 0.0, w, tb.tab_bar_height, side_bg);

    // The scrolling content area is left TRANSPARENT (no canvas fill) so empty
    // (pane-less) space shows the frosted vibrancy, per design. Panes are still
    // clipped to `pane_area` below so they don't bleed under the chrome.

    let active_pane_id = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|pane| pane.id)
        .or(state.focused_pane);

    state
        .primitive_renderer
        .render(&state.device, scene_view, &mut encoder);
    state
        .text_renderer
        .render(&state.queue, scene_view, &mut encoder);

    // ── In-app frosted backdrop blur (two-pass) ──
    //
    // Both tiled and floating panes receive frosted backdrop stamps, per the task
    // spec ("apply the same surface policy to tiled and floating panes").
    //
    // Two blur passes are needed so each pane type frosts the content actually
    // behind it:
    //
    //   Pass 1: blur the chrome/background (no pane content yet).
    //     → Stamped behind tiled panes (frosts chrome/adjacent gaps).
    //
    //   Pass 2: blur the scene including tiled pane content.
    //     → Stamped behind floating panes (frosts the tiled content behind them).
    //
    // This is O(2) blurs per frame (not per-pane), which satisfies the performance
    // requirement ("avoid per-pane full-scene blur recomputation").
    //
    // Correct unit conversion: `appearance.blur_radius()` returns logical px,
    // but `Blur::process` consumes source-texture pixels (physical px for the
    // compositor scene texture). Scale by `scale_factor`.
    //
    // Policy: no visible pane frosting unless the pane surface actually has
    // alpha to reveal it (`terminal_surface_opacity() < 1.0`). When blur is 0
    // or transparency is off, this is a no-op (radius 0 = passthrough).
    let needs_frosted_backdrop = state.appearance.is_transparent()
        && state.appearance.blur_radius() > 0.0;
    let mut tiled_blurred_view: Option<&wgpu::TextureView> = None;
    let mut float_blurred_view: Option<&wgpu::TextureView> = None;

    // Blur pass 1: chrome/background only (before any pane content).
    if needs_frosted_backdrop {
        let radius_physical = state.appearance.blur_radius() * state.scale_factor as f32;
        tiled_blurred_view = Some(state.blur.process(
            &state.device,
            &state.queue,
            &mut encoder,
            scene_view,
            radius_physical,
        ));
    }

    // ── Pane chrome from pane-specific config ──
    // These are separate from the global theme border/accent so panes can
    // have their own border width, radius, and color treatment.
    let pane_border_color = state.appearance.effective_pane_border_color(theme).to_f32x4();
    let pane_active_border_color = state.appearance.effective_pane_active_border_color(theme).to_f32x4();
    let pane_border_width = state.appearance.effective_pane_border_width(theme);
    // Pane border radius: read from config but the primitive renderer does not yet
    // implement rounded corner clipping. When the heca-grid-ui Scene integration
    // replaces the primitive draw_border path, radius will take full effect.
    // Until then, `draw_border` draws straight rectangles regardless of this value.
    let pane_border_radius = state.appearance.effective_pane_border_radius(theme);

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
    let surface_physical_size = state.window.inner_size();
    // Scissor for the scrolling content area — pane backgrounds/borders are clipped
    // to it so panes scrolled partially behind the chrome don't bleed under it.
    let content_scissor = pane_scissor_rect(
        pane_area.loc.x as f32,
        pane_area.loc.y as f32,
        pane_area.size.w as f32,
        pane_area.size.h as f32,
        state.scale_factor,
        surface_physical_size,
    );

    // ── Pass A: Frosted backdrop stamps (under borders) ──
    //
    // Stamp the blurred chrome/background behind each tiled pane rect FIRST
    // so the border scene below sits on top of the frosted surface.
    // This matches the float pane ordering: backdrop → border → content.
    for (_pane_id, rect) in &pane_positions {
        let px = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
        let py = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
        let pw = rect.size.w as f32;
        let ph = rect.size.h as f32;
        if let Some(blurred) = tiled_blurred_view {
            let scale = state.scale_factor as f32;
            let vp_w = surface_physical_size.width as f32;
            let vp_h = surface_physical_size.height as f32;
            let dst = (px * scale, py * scale, pw * scale, ph * scale);
            state.backdrop.draw(
                &state.device,
                &state.queue,
                &mut encoder,
                scene_view,
                blurred,
                (vp_w, vp_h),
                dst,
                None,
                surface_alpha,
            );
        }
    }

    // ── Pass B: Pane chrome borders (rounded rect outlines via grid renderer) ──
    //
    // Draw a rounded-rect outline for each tiled pane as a `DrawCommand::Rect`
    // so pane_border_radius takes effect. Rendered via the grid renderer on top
    // of the frosted backdrop and below terminal content. Clipped to the pane
    // content area so borders don't bleed under chrome.
    if !pane_positions.is_empty() {
        use heca_grid_ui::scene::DrawCommand;
        use heca_grid_ui::Rectangle as GuiRect;
        use heca_grid_ui::Point as GuiPoint;
        use heca_grid_ui::Size as GuiSize;

        // Clip the whole pass to the pane content area.
        let mut pane_scene = heca_grid_ui::Scene::new();
        pane_scene.push(DrawCommand::PushClip(
            GuiRect::new(
                GuiPoint::new(pane_area.loc.x, pane_area.loc.y),
                GuiSize::new(pane_area.size.w, pane_area.size.h),
            ),
        ));

        for (pane_id, rect) in &pane_positions {
            let px = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
            let py = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
            let pw = rect.size.w as f32;
            let ph = rect.size.h as f32;
            let is_active = active_pane_id == Some(*pane_id);
            let bcolor = if is_active {
                pane_active_border_color
            } else {
                pane_border_color
            };
            push_pane_border_rect(
                &mut pane_scene,
                px,
                py,
                pw,
                ph,
                bcolor,
                pane_border_width,
                pane_border_radius,
            );
        }

        pane_scene.push(DrawCommand::PopClip);

        // Flush the border scene through the grid renderer.
        state.grid_renderer.begin_frame();
        state.grid_renderer.set_damage(None);
        state.grid_renderer.set_clip(None);
        heca_renderer::scene::enqueue_scene(
            &mut state.grid_renderer,
            &mut state.text_renderer,
            &pane_scene,
        );
        state.grid_renderer.render(
            &state.queue, scene_view, &mut encoder,
        );
    }

    // ── Pass 2: Terminal content (on top of backdrop + borders) ──
    // Pre-compute the pane background color from theme, modulated by surface
    // opacity, for the fallback fill when no backend is mounted.
    let theme_base = theme.background.to_f32x4();
    let pane_bg = [theme_base[0], theme_base[1], theme_base[2], theme_base[3] * surface_alpha];
    for (pane_id, rect) in &pane_positions {
        let px = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
        let py = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
        let pw = rect.size.w as f32;
        let ph = rect.size.h as f32;
        let content_rect = stable_tiled_content_rect(px, py, pw, ph, pane_border_width);
        let pane_mount = if let Some(content_rect) = content_rect {
            prepare_terminal_mount(
                &mut state.backends,
                *pane_id,
                content_rect,
                state.terminal_cell_size,
            )
        } else {
            None
        };

        if let Some(content_rect) = content_rect {
            if let Some(mount) = pane_mount {
                let selection_overlay =
                    selection_overlay_for_pane(state, *pane_id, mount.snapshot.cols);
                render_terminal_mount(
                    TerminalRenderPassContext {
                        text_renderer: &mut state.text_renderer,
                        primitive_renderer: &mut state.primitive_renderer,
                        device: &state.device,
                        queue: &state.queue,
                        view: scene_view,
                        encoder: &mut encoder,
                        scale_factor: state.scale_factor,
                        surface_physical_size,
                        content_clip: pane_area,
                    },
                    TerminalStyle {
                        font_size: theme.terminal_font_size,
                        font_family: &theme.terminal_font_family,
                        italic_font_family: &theme.terminal_italic_font_family,
                        surface_alpha,
                    },
                    mount,
                    selection_overlay,
                );
            } else {
                let content_box = rect_to_text_box(content_rect);
                state
                    .primitive_renderer
                    .draw_rect(content_box.x, content_box.y, content_box.w, content_box.h, pane_bg);
            }
        } else {
            state
                .primitive_renderer
                .draw_rect(px, py, pw, ph, pane_bg);
        }
    }
    // ── End pass 2 (terminal content flushed inside render_terminal_mount) ──
    state
        .primitive_renderer
        .render_clipped(&state.device, scene_view, &mut encoder, content_scissor);
    state
        .text_renderer
        .render(&state.queue, scene_view, &mut encoder);

    // ── Blur pass 2: scene including tiled pane content ──
    //
    // Capture a second blur after tiled panes have been rendered into the scene.
    // This lets floating panes frost the actual tiled content behind them, not
    // just the chrome/background.
    if needs_frosted_backdrop {
        let radius_physical = state.appearance.blur_radius() * state.scale_factor as f32;
        float_blurred_view = Some(state.blur.process(
            &state.device,
            &state.queue,
            &mut encoder,
            scene_view,
            radius_physical,
        ));
    }

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
                pane_active_border_color
            } else {
                pane_border_color
            };
            let content_rect = stable_floating_content_rect(fx, fy, fw, fh, pane_border_width);
            let pane_mount = if let Some(content_rect) = content_rect {
                prepare_terminal_mount(
                    &mut state.backends,
                    float.pane.id,
                    content_rect,
                    state.terminal_cell_size,
                )
            } else {
                None
            };
            if let Some(content_rect) = content_rect {
                // ── Frosted backdrop stamp for floating pane surfaces ──
                //
                // Uses blur pass 2 (includes tiled content) so floating panes frost
                // the actual tiled pane content behind them.
                if let Some(blurred) = float_blurred_view {
                    let scale = state.scale_factor as f32;
                    let vp_w = surface_physical_size.width as f32;
                    let vp_h = surface_physical_size.height as f32;
                    let dst = (fx * scale, fy * scale, fw * scale, fh * scale);
                    state.backdrop.draw(
                        &state.device,
                        &state.queue,
                        &mut encoder,
                        scene_view,
                        blurred,
                        (vp_w, vp_h),
                        dst,
                        None,
                        surface_alpha,
                    );
                }

                // Draw floating pane chrome through the same rounded scene path
                // used by tiled panes so border color/width/radius stay consistent.
                {
                    use heca_grid_ui::scene::DrawCommand;
                    use heca_grid_ui::Rectangle as GuiRect;
                    use heca_grid_ui::Point as GuiPoint;
                    use heca_grid_ui::Size as GuiSize;

                    let mut float_scene = heca_grid_ui::Scene::new();
                    float_scene.push(DrawCommand::PushClip(
                        GuiRect::new(
                            GuiPoint::new(pane_area.loc.x, pane_area.loc.y),
                            GuiSize::new(pane_area.size.w, pane_area.size.h),
                        ),
                    ));
                    push_pane_border_rect(
                        &mut float_scene,
                        fx,
                        fy,
                        fw,
                        fh,
                        fborder,
                        pane_border_width,
                        pane_border_radius,
                    );
                    float_scene.push(DrawCommand::PopClip);
                    state.grid_renderer.begin_frame();
                    state.grid_renderer.set_damage(None);
                    state.grid_renderer.set_clip(None);
                    heca_renderer::scene::enqueue_scene(
                        &mut state.grid_renderer,
                        &mut state.text_renderer,
                        &float_scene,
                    );
                    state.grid_renderer.render(&state.queue, scene_view, &mut encoder);
                }

                if let Some(mount) = pane_mount {
                    let selection_overlay =
                        selection_overlay_for_pane(state, float.pane.id, mount.snapshot.cols);
                    render_terminal_mount(
                        TerminalRenderPassContext {
                            text_renderer: &mut state.text_renderer,
                            primitive_renderer: &mut state.primitive_renderer,
                            device: &state.device,
                            queue: &state.queue,
                            view: scene_view,
                            encoder: &mut encoder,
                            scale_factor: state.scale_factor,
                            surface_physical_size,
                            content_clip: pane_area,
                        },
                        TerminalStyle {
                            font_size: theme.terminal_font_size,
                            font_family: &theme.terminal_font_family,
                            italic_font_family: &theme.terminal_italic_font_family,
                            surface_alpha,
                        },
                        mount,
                        selection_overlay,
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
        }
    }

    let pane_area_rect = heca_core::layout::Rectangle::new(
        heca_core::layout::Point::new(pane_area.loc.x, pane_area.loc.y),
        heca_core::layout::Size::new(pane_area.size.w, pane_area.size.h),
    );

    // The EXPANDED left sidebar is now drawn by the grid-ui chrome scene
    // (`build_chrome_scene`). Only the COLLAPSED icon rail is still hand-drawn
    // here; when expanded we skip the hand-drawn bg/divider/content entirely so it
    // doesn't paint over the grid sidebar.
    if chrome.left_sidebar_width < crate::chrome::SIDEBAR_EXPANDED_THRESHOLD {
        let sidebar_gap = chrome.sidebar_gap.max(0.0);
        let rail_x = sidebar_gap.min(chrome.left_sidebar_width * 0.5);
        let rail_y = sidebar_top + sidebar_gap;
        let rail_w = (chrome.left_sidebar_width - rail_x * 2.0).max(0.0);
        let rail_h = (sidebar_h - sidebar_gap * 2.0).max(0.0);
        state.primitive_renderer.draw_rect(
            rail_x,
            rail_y,
            rail_w,
            rail_h,
            collapsed_sidebar_bg,
        );
        state.primitive_renderer.draw_outline(
            rail_x,
            rail_y,
            rail_w,
            rail_h,
            [theme.border.to_f32x4()[0], theme.border.to_f32x4()[1], theme.border.to_f32x4()[2], 0.35],
            1.0,
        );
        state.primitive_renderer.draw_border(
            rail_x,
            rail_y,
            rail_w,
            rail_h,
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
        let drag_source_bg = theme.drag_source_bg.to_f32x4();
        let drag_source_border = theme.drag_source_border.to_f32x4();
        sidebar::render_sidebar_collapsed(
            &mut state.sidebar_tree,
            rail_x,
            rail_y,
            rail_w,
            rail_h,
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
            theme.drag_ghost_bg.to_f32x4(),
        );
        state.primitive_renderer.draw_border(
            ghost_x,
            ghost_y,
            ghost_w,
            ghost_h,
            theme.drag_source_border.to_f32x4(),
            1.5,
        );
        state.text_renderer.queue_text(
            &label.text,
            ghost_x + 6.0,
            ghost_y + 4.0,
            13.0,
            theme.drag_ghost_fg.to_f32x4(),
        );
    }

    if chrome.right_sidebar_width < crate::chrome::SIDEBAR_EXPANDED_THRESHOLD {
        let sidebar_gap = chrome.sidebar_gap.max(0.0);
        let rail_w = (chrome.right_sidebar_width - sidebar_gap * 2.0).max(0.0);
        let rail_h = (sidebar_h - sidebar_gap * 2.0).max(0.0);
        let rail_x = w - chrome.right_sidebar_width + sidebar_gap;
        let rail_y = sidebar_top + sidebar_gap;
        state
            .primitive_renderer
            .draw_rect(rail_x, rail_y, rail_w, rail_h, collapsed_sidebar_bg);
        state.primitive_renderer.draw_outline(
            rail_x,
            rail_y,
            rail_w,
            rail_h,
            [theme.border.to_f32x4()[0], theme.border.to_f32x4()[1], theme.border.to_f32x4()[2], 0.35],
            1.0,
        );
        state.primitive_renderer.draw_border(
            rail_x,
            rail_y,
            rail_w,
            rail_h,
            theme.border.to_f32x4(),
            1.0,
        );
        state.text_renderer.queue_text(
            "D",
            rail_x + 8.0,
            rail_y + 8.0,
            chrome_text,
            theme.foreground.to_f32x4(),
        );
    }

    mouse::render_detached_pane(state, pane_area_rect);
    mouse::render_insert_hint(state, pane_area_rect);

    // Reborrow compositor scene texture for the final flush (the previous
    // `scene_view` borrow ended at its last use before the mouse:: calls above).
    let scene_view = state.compositor.scene_view();

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
                        break;
                    }
                }
            }
        }
    }

    state
        .primitive_renderer
        .render(&state.device, scene_view, &mut encoder);
    state
        .text_renderer
        .render(&state.queue, scene_view, &mut encoder);

    // Grid-ui chrome (full-height sidebar SHELL + status bar) painted LAST so the
    // shell sits ON TOP of the pane content/canvas instead of panes bleeding under
    // it. The collapsed left rail + right "Details" sidebar stay hand-drawn above.
    //
    // F4.1 — retained tree: rebuild the widget tree only when the chrome signature
    // changes; otherwise re-layout + paint the kept tree (no per-frame signal churn,
    // and a live tree to dispatch events into in F4.2).
    let chrome_sig = crate::chrome::chrome_signature(state, chrome);
    if state.chrome_tree.as_ref().map(|t| t.sig) != Some(chrome_sig) {
        let root = crate::chrome::build_chrome_root(state, chrome);
        state.chrome_tree = Some(crate::chrome::RetainedChrome { root, sig: chrome_sig });
    }
    let chrome_theme = crate::chrome::chrome_gui_theme(state);
    let chrome_scene = crate::chrome::paint_chrome_root(
        &mut state.chrome_tree.as_mut().expect("chrome tree set above").root,
        w,
        h,
        &chrome_theme,
    );
    render_chrome(
        &mut state.grid_renderer,
        &mut state.text_renderer,
        &state.queue,
        &chrome_scene,
        scene_view,
        &mut encoder,
    );

    state.compositor.blit(&view, &mut encoder);
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
        left_sidebar_width: if state.chrome_state.left_visible() {
            state.chrome_state.left_size()
        } else {
            40.0
        },
        right_sidebar_width: if state.chrome_state.right_visible() {
            state.chrome_state.right_size()
        } else {
            40.0
        },
        sidebar_gap: state.appearance.effective_sidebar_gap(&state.theme),
    };
    let pane_area = chrome.content_rect(win_w, win_h);
    let new_size = heca_core::layout::types::Size::new(pane_area.size.w, pane_area.size.h);
    state.session.update_viewport(new_size);
}

#[cfg(test)]
mod tests {
    use super::{build_selection_overlay, status_mode_parts};
    use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionSource, SelectionState};
    use crate::app_state::{InputMode, RenameTarget};
    use crate::input::WmAction;
    use heca_config::theme::Color;
    use heca_core::layout::PaneId;

    #[test]
    fn build_selection_overlay_returns_none_when_inactive() {
        let selection = SelectionState::new();
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        assert!(build_selection_overlay(&selection, PaneId(1), 10, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_returns_none_when_owner_mismatch() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(7)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 2,
                anchor_col: 3,
                focus_row: 2,
                focus_col: 3,
            },
        );
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        // Query with a different pane id -> None
        assert!(build_selection_overlay(&selection, PaneId(1), 10, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_returns_none_for_backend_native() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::Rpc,
            SelectionRegion::BackendNative,
        );
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        assert!(build_selection_overlay(&selection, PaneId(1), 10, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_returns_overlay_for_host_grid_on_owning_pane() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 2,
                anchor_col: 3,
                focus_row: 2,
                focus_col: 3,
            },
        );
        selection.update_focus(5, 9);
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let overlay = build_selection_overlay(&selection, PaneId(1), 20, &accent);
        assert!(overlay.is_some());
        let overlay = overlay.unwrap();
        assert_eq!(overlay.spans.len(), 4);
        assert_eq!(overlay.spans[0].row, 2);
        assert_eq!(overlay.spans[0].start_col, 3);
        assert_eq!(overlay.spans[0].end_col, 19);
        assert_eq!(overlay.spans[3].row, 5);
        assert_eq!(overlay.spans[3].start_col, 0);
        assert_eq!(overlay.spans[3].end_col, 9);
        // Color should be accent / 255 with 0.25 alpha
        assert_eq!(overlay.color, [100.0 / 255.0, 150.0 / 255.0, 200.0 / 255.0, 0.25]);
    }

    #[test]
    fn build_selection_overlay_handles_reverse_multiline_selection() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 8,
                anchor_col: 12,
                focus_row: 8,
                focus_col: 12,
            },
        );
        selection.update_focus(4, 3);
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let overlay = build_selection_overlay(&selection, PaneId(1), 20, &accent).unwrap();
        assert_eq!(overlay.spans.len(), 5);
        assert_eq!(overlay.spans[0].row, 4);
        assert_eq!(overlay.spans[0].start_col, 3);
        assert_eq!(overlay.spans[0].end_col, 19);
        assert_eq!(overlay.spans[4].row, 8);
        assert_eq!(overlay.spans[4].start_col, 0);
        assert_eq!(overlay.spans[4].end_col, 12);
    }

    #[test]
    fn build_selection_overlay_caret_returns_caret_indicator() {
        let mut selection = SelectionState::new();
        selection.set_caret(SelectionOwner::Pane(PaneId(1)), 3, 7);
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let overlay = build_selection_overlay(&selection, PaneId(1), 20, &accent).unwrap();
        // Caret-only state: no selection spans.
        assert!(overlay.spans.is_empty());
        // But we get a caret indicator at the caret position.
        let caret = overlay.caret.expect("caret should be present in caret-only state");
        assert_eq!((caret.row, caret.col), (3, 7));
        assert!(!caret.is_selection_endpoint, "caret-only should not be a selection endpoint");
    }

    #[test]
    fn build_selection_overlay_caret_owner_mismatch_returns_none() {
        let mut selection = SelectionState::new();
        selection.set_caret(SelectionOwner::Pane(PaneId(2)), 0, 0);
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        assert!(build_selection_overlay(&selection, PaneId(1), 20, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_active_selection_has_focus_caret() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::KeyboardMode,
            SelectionRegion::HostGrid {
                anchor_row: 2,
                anchor_col: 0,
                focus_row: 4,
                focus_col: 5,
            },
        );
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let overlay = build_selection_overlay(&selection, PaneId(1), 20, &accent).unwrap();
        // Active selection: should have both selection spans and a focus-end caret.
        assert!(!overlay.spans.is_empty());
        let caret = overlay.caret.expect("focus caret should be present for active selection");
        assert_eq!((caret.row, caret.col), (4, 5));
        assert!(caret.is_selection_endpoint, "active selection caret should be a selection endpoint");
    }

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
