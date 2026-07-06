//! Render and viewport helpers.
//!
//! These helpers keep low-level pane rendering and viewport synchronization out
//! of `main.rs` while preserving the current render pipeline behavior.

use crate::app::terminal_host::prepare_terminal_mount;
use crate::app::terminal_render::{
    PaneRenderState, TerminalPaneShell, TerminalRenderPassContext, blit_retained_terminal_layer,
    paint_terminal_pane_shell, pane_scissor_rect, queue_terminal_dynamic_overlays,
    render_terminal_mount, selection_overlay_for_pane, stable_floating_content_rect,
    stable_tiled_content_rect, sync_retained_terminal_layers,
};
use crate::app_state::{AppState, InputMode};
use crate::chrome::ChromeConfig;
use crate::{mouse, sidebar};
use heca_grid_ui::Component;
use heca_grid_ui::drag::DragSurfaceId;
use heca_grid_ui::{
    Point as GuiPoint, Rectangle as GuiRectangle, Scene as GuiScene, Size as GuiSize,
};
use heca_renderer::grid::GridRenderer;
use heca_renderer::terminal::{HyperlinkDecor, TerminalFontFamilies, TerminalStyle};
use heca_renderer::text::TextRenderer;

/// Project the terminal font-family group from config into the renderer's
/// per-style family slots. The renderer stays config-free; this is the app-side
/// bridge. Takes the whole `FontFamilies` so the `normal` slot can be resolved
/// with the **terminal** embedded fallback (`terminal_normal()`), not the UI
/// fallback — omitting `[font.family.terminal].normal` keeps Maple Mono, not
/// Geist Mono. Borrows from `families` so the returned slots live as long as it.
fn terminal_font_families_from(
    families: &heca_config::font::FontFamilies,
) -> TerminalFontFamilies<'_> {
    let tf = &families.terminal;
    TerminalFontFamilies {
        normal: families.terminal_normal(),
        bold: tf.bold.as_deref(),
        italic: tf.italic.as_deref(),
        bold_italic: tf.bold_italic.as_deref(),
    }
}

/// Map the config hyperlink decoration onto the renderer's enum.
fn hyperlink_decor_from(style: heca_config::appearance::HyperlinkStyle) -> HyperlinkDecor {
    use heca_config::appearance::HyperlinkStyle as S;
    match style {
        S::None => HyperlinkDecor::None,
        S::Color => HyperlinkDecor::Color,
        S::Underline => HyperlinkDecor::Underline,
        S::Undercurl => HyperlinkDecor::Undercurl,
    }
}

/// Human-readable status mode label and suffix for the status bar.
pub(crate) fn status_mode_parts(input_mode: &InputMode) -> (&'static str, String) {
    // For pick modes the prompt suffix is sourced from the action's `ActionDescriptor`
    // (via `pending_pick`) so the text lives in one place — the action registry.
    let pick_suffix = || {
        input_mode
            .pending_pick()
            .map(|p| format!(" — {}", p.prompt))
            .unwrap_or_default()
    };
    match input_mode {
        InputMode::Normal => ("NORMAL", String::new()),
        InputMode::Prefix => ("PREFIX", String::new()),
        InputMode::PaneSelect { .. } => ("SELECT", pick_suffix()),
        InputMode::FollowLink { .. } => {
            ("FOLLOW", " — press a letter to open the link".to_string())
        }
        InputMode::HintPick { .. } => {
            ("HINT", " — press a letter to activate a target".to_string())
        }
        InputMode::Search => (
            "SEARCH",
            " — type to search, Enter to keep, Esc to cancel".to_string(),
        ),
        InputMode::PaneSwap { focus_after, .. } => (
            if *focus_after { "SWAP+FOCUS" } else { "SWAP" },
            pick_suffix(),
        ),
        InputMode::SidebarNav => ("SIDEBAR", String::new()),
        InputMode::Rename { buffer, .. } => ("RENAME", format!(": {}_", buffer)),
        InputMode::Chord { sequence } => ("CHORD", format!(" w→{}", sequence.join("→"))),
        InputMode::Mode { name } => ("MODE", format!(" {} → ?", name)),
        // The confirm now lives entirely in the Modal dialog; the status bar only
        // shows the mode word, no duplicated prompt.
        InputMode::ConfirmDelete { .. } => ("CONFIRM", String::new()),
        InputMode::PaneTake { focus_after, .. } => {
            (if *focus_after { "TAKE+" } else { "TAKE" }, pick_suffix())
        }
        InputMode::WorkspacePick { target, .. } => {
            let label = match target {
                crate::app_state::WorkspacePickTarget::Column { .. } => "MOVE COL",
                crate::app_state::WorkspacePickTarget::Pane(_) => "MOVE PANE",
            };
            (label, pick_suffix())
        }
        InputMode::ColumnPick { .. } => ("MOVE PANE", pick_suffix()),
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
struct ChromePassOpts {
    damage: Option<heca_grid_ui::Rectangle>,
    glow_alpha_scale: f32,
}

fn render_chrome(
    grid: &mut GridRenderer,
    text: &mut TextRenderer,
    queue: &wgpu::Queue,
    scene: &heca_grid_ui::Scene,
    opts: ChromePassOpts,
    view: &wgpu::TextureView,
    encoder: &mut wgpu::CommandEncoder,
) {
    // NOTE: `begin_frame()` is called once at the top of `render_frame`, not here.
    // Calling it per `render_chrome` reset the persistent vertex-buffer write
    // offset to 0 every pass, so each pass overwrote the previous pass's vertices
    // at buffer offset 0 — and since all passes are submitted in one
    // `queue.submit` at end of frame, every encoded pass read the *last* pass's
    // vertex data. The result: only the final grid scene (chrome) rendered; the
    // pane-shell scenes (Pass 3 borders + floating pane shells) drew the chrome
    // geometry clipped to their own scissor and showed nothing. One
    // `begin_frame()` per frame makes each `render()` append at a distinct offset
    // so all grid scenes render their own geometry.
    let damage = opts.damage.map(|r| {
        [
            r.loc.x as f32,
            r.loc.y as f32,
            r.size.w as f32,
            r.size.h as f32,
        ]
    });
    grid.set_damage(damage);
    grid.set_clip(None);
    text.set_damage(damage);
    heca_renderer::scene::enqueue_scene(grid, text, &scene.base_layer(), opts.glow_alpha_scale);
    grid.render(queue, view, encoder);
    text.render(queue, view, encoder, None);
    for overlay in scene.overlay_segments() {
        heca_renderer::scene::enqueue_scene(grid, text, &overlay, opts.glow_alpha_scale);
        grid.render(queue, view, encoder);
        text.render(queue, view, encoder, None);
    }
}

/// Render the full frame for the current app state.
pub(crate) fn render_frame(state: &mut AppState) {
    if !state.needs_redraw {
        return;
    }
    state.needs_redraw = false;
    let _ = crate::chrome::sync_chrome_state(state);
    // Build/position the retained per-pane info-bar headers *before* the GPU borrow
    // below (`scene_view` borrows `state.compositor`), so render can paint them
    // read-only and `mouse.rs` can dispatch pointer events into them.
    crate::chrome::sync_pane_headers(state);

    let phys_size = state.window.inner_size();
    let scale = state.scale_factor as f32;
    let w = phys_size.width as f32 / scale;
    let h = phys_size.height as f32 / scale;

    // Lay out the right-click context menu now, before the scene-texture borrow, so
    // its paint pass (below) can take a shared `&AppState`. terminal-task-18.
    crate::chrome::layout_context_menu(state, w, h);
    crate::chrome::layout_confirm_dialog(state, w, h);

    let glow_alpha_scale =
        heca_renderer::scene::glow_alpha_scale_for_background(state.theme.background.to_f32x4());
    let surface_alpha = state.terminal_surface_opacity();
    // Floating panes use independent opacity/blur/border knobs so they can stay
    // readable (opaque by default) while tiled panes are frosted.
    let floating_surface_alpha = state.terminal_floating_surface_opacity();
    let terminal_ligatures = state.appearance.terminal.ligatures;
    let terminal_hyperlink_style = hyperlink_decor_from(state.appearance.terminal.hyperlink_style);
    // Link color: config override → theme accent.
    let terminal_hyperlink_color = state
        .appearance
        .terminal
        .hyperlink_color
        .unwrap_or(state.theme.accent)
        .to_f32x4();

    let chrome = ChromeConfig {
        tab_bar_height: state.tab_bar_height(),
        status_bar_height: state.status_bar_height(),
        left_sidebar_width: state.left_sidebar_width(),
        right_sidebar_width: state.right_sidebar_width(),
        sidebar_gap: state.appearance.effective_sidebar_gap(&state.theme),
    };
    let pane_area = chrome.content_rect(w, h);
    state.text_renderer.begin_frame();
    state.text_renderer.set_damage(None);
    state.text_renderer.set_clip(None);
    // Reset the grid renderer's persistent vertex/index buffer offsets once per
    // frame so each `render_chrome` pass appends at a distinct region (see the
    // note in `render_chrome`).
    state.grid_renderer.begin_frame();

    let active_pane_id = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|pane| pane.id)
        .or_else(|| state.chrome_state.workspaces.active_pane())
        .or(state.focused_pane);

    // ── Pane chrome from pane-specific config ──
    let pane_border_color = state
        .appearance
        .effective_pane_border_color(&state.theme)
        .to_f32x4();
    let pane_active_border_color = state
        .appearance
        .effective_pane_active_border_color(&state.theme)
        .to_f32x4();
    let pane_floating_border_color = state
        .appearance
        .effective_pane_floating_border_color(&state.theme)
        .to_f32x4();
    let pane_border_width = state.appearance.effective_pane_border_width(&state.theme);
    let pane_border_radius = state.appearance.effective_pane_border_radius(&state.theme);
    let pane_content_inset = state.appearance.effective_pane_padding(&state.theme);
    let pane_title_top_inset = crate::app::terminal_render::pane_title_top_inset(state);
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
    let content_scissor = pane_scissor_rect(
        pane_area.loc.x as f32,
        pane_area.loc.y as f32,
        pane_area.size.w as f32,
        pane_area.size.h as f32,
        state.scale_factor,
        surface_physical_size,
    );

    let mut tiled_panes = Vec::with_capacity(pane_positions.len());
    for (pane_id, rect) in &pane_positions {
        let px = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
        let py = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
        let pw = rect.size.w as f32;
        let ph = rect.size.h as f32;
        let content_rect =
            stable_tiled_content_rect(px, py, pw, ph, pane_content_inset, pane_title_top_inset);
        let base_cell = state.pane_base_cell_size(*pane_id);
        let mount = content_rect.and_then(|content_rect| {
            prepare_terminal_mount(
                &mut state.backends,
                *pane_id,
                content_rect,
                base_cell,
                state.scale_factor as f32,
            )
        });
        // Sync viewport state into the chrome store for GUI reactivity.
        if let Some(ref m) = mount {
            state.chrome_state.workspaces.set_pane_viewport(
                *pane_id,
                m.snapshot.viewport_offset,
                m.snapshot.at_bottom,
                m.snapshot.scrollback_rows,
            );
        }
        tiled_panes.push(PaneRenderState {
            pane_id: *pane_id,
            x: px,
            y: py,
            w: pw,
            h: ph,
            is_active: active_pane_id == Some(*pane_id),
            content_rect,
            mount,
        });
    }

    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = h - chrome.status_bar_height;
    let sidebar_h = sidebar_bottom - sidebar_top;
    let terminal_font_config = state.font_config.clone();
    // Effective global terminal size (config + global zoom). Per-pane offsets are
    // applied inside `sync_retained_terminal_layers`; this is the base/fallback.
    let app_font_size = state.app_font_size();
    let mut floating_panes = Vec::new();
    if let Some(ws) = state.session.active_workspace() {
        for float in &ws.floating_panes {
            let fx = float.position.x as f32 + pane_area.loc.x as f32 + ws_offset.0;
            let fy = float.position.y as f32 + pane_area.loc.y as f32 + ws_offset.1;
            let fw = float.size.w as f32;
            let fh = float.size.h as f32;
            let content_rect = stable_floating_content_rect(
                fx,
                fy,
                fw,
                fh,
                pane_content_inset,
                pane_title_top_inset,
            );
            let base_cell = state.pane_base_cell_size(float.pane.id);
            let mount = content_rect.and_then(|content_rect| {
                prepare_terminal_mount(
                    &mut state.backends,
                    float.pane.id,
                    content_rect,
                    base_cell,
                    state.scale_factor as f32,
                )
            });
            // Sync viewport state into the chrome store for GUI reactivity.
            if let Some(ref m) = mount {
                state.chrome_state.workspaces.set_pane_viewport(
                    float.pane.id,
                    m.snapshot.viewport_offset,
                    m.snapshot.at_bottom,
                    m.snapshot.scrollback_rows,
                );
            }
            floating_panes.push(PaneRenderState {
                pane_id: float.pane.id,
                x: fx,
                y: fy,
                w: fw,
                h: fh,
                is_active: active_pane_id == Some(float.pane.id),
                content_rect,
                mount,
            });
        }
    }

    let mut viewport_widget_panes: Vec<&PaneRenderState> = Vec::with_capacity(
        tiled_panes.len() + floating_panes.len(),
    );
    viewport_widget_panes.extend(tiled_panes.iter());
    viewport_widget_panes.extend(floating_panes.iter());
    crate::chrome::sync_pane_viewport_widgets(state, &viewport_widget_panes);

    // Recomputed each frame by the two `sync_retained_terminal_layers` calls
    // below (tiled + floating), which OR into it. Reset once here first.
    state.has_animated_images = false;
    sync_retained_terminal_layers(
        state,
        &tiled_panes,
        TerminalStyle {
            font_size: app_font_size,
            families: terminal_font_families_from(&terminal_font_config.family),
            surface_alpha,
            ligatures: terminal_ligatures,
            hyperlink_style: terminal_hyperlink_style,
            hyperlink_color: terminal_hyperlink_color,
        },
        (w, h),
        surface_physical_size,
    );
    sync_retained_terminal_layers(
        state,
        &floating_panes,
        TerminalStyle {
            font_size: app_font_size,
            families: terminal_font_families_from(&terminal_font_config.family),
            surface_alpha: floating_surface_alpha,
            ligatures: terminal_ligatures,
            hyperlink_style: terminal_hyperlink_style,
            hyperlink_color: terminal_hyperlink_color,
        },
        (w, h),
        surface_physical_size,
    );

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
    // Stencil buffer paired with the scene texture: holds the rounded content-clip
    // mask written each frame so terminal content follows the pane's rounded border.
    let stencil_view = state.compositor.stencil_view();

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render"),
        });

    let theme = &state.theme;

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

    // ── z=0 background layer (heca-owned frosted gradient) ──
    //
    // Bottom-most layer: a blurred vertical gradient composited at
    // `background_alpha()` (opaque by default per the locked decision). Tiled
    // panes then render translucent (`surface_alpha`) directly over this — their
    // frost IS z=0 showing through, not a per-pane tint. Drawn pre-stencil
    // (`stencil = None`) so the tiled content-clip never clips the background.
    // `BackgroundLayer` caches the blurred result; it recomputes only on resize
    // or param change (see `heca-renderer/src/background.rs`).
    {
        let top = state
            .appearance
            .effective_background_gradient_top(theme)
            .to_linear_f32x4();
        let bottom = state
            .appearance
            .effective_background_gradient_bottom(theme)
            .to_linear_f32x4();
        let blur_radius = state.appearance.background_blur_radius() * scale;
        state.background.set_params(top, bottom, blur_radius);
        // Order invariant: BackgroundLayer snapshots the blurred gradient into its
        // own cache inside `render()`, so the shared `state.blur` is free to be
        // reused afterwards by the floating-pane frost pass below. The z=0 layer
        // MUST be rendered BEFORE any other `state.blur` user this frame — if a
        // later blur user runs first, the z=0 cache would capture that user's
        // output instead of the gradient. (See `heca-renderer/src/background.rs`.)
        let bg_view =
            state
                .background
                .render(&state.device, &state.queue, &mut encoder, &state.blur);
        let vp_w = phys_size.width as f32;
        let vp_h = phys_size.height as f32;
        state.backdrop.draw(
            &state.device,
            &state.queue,
            &mut encoder,
            scene_view,
            bg_view,
            (vp_w, vp_h),
            (0.0, 0.0, vp_w, vp_h),
            Some([0.0, 0.0, 1.0, 1.0]),
            state.appearance.background_alpha(),
            None,
        );
    }

    let chrome_text = crate::chrome::CHROME_TEXT_SIZE;
    let tb = &chrome;
    // Frosted chrome colors come from the loaded theme's surface tone, with
    // alpha derived from the current appearance settings.
    let (side_bg, collapsed_sidebar_bg, _) = crate::chrome::chrome_colors(state);
    let side_bg = side_bg.to_f32x4();
    let collapsed_sidebar_bg = collapsed_sidebar_bg.to_f32x4();
    state
        .primitive_renderer
        .draw_rect(0.0, 0.0, w, tb.tab_bar_height, side_bg);

    state
        .primitive_renderer
        .render(&state.device, scene_view, &mut encoder);
    state
        .text_renderer
        .render(&state.queue, scene_view, &mut encoder, None);

    // ── Floating-pane real blur ──
    //
    // Floating panes frost the actual tiled content behind them (the scene
    // already includes tiled panes + z=0), so a real blur pass is captured once
    // per frame and stamped behind each floating pane at 100% opacity. Tiled
    // panes frost via the z=0 background layer showing through their translucent
    // surface (`surface_alpha`) — no per-tiled-pane tint.
    let needs_floating_frost =
        floating_surface_alpha < 1.0 && state.appearance.terminal_floating_blur_radius() > 0.0;
    let mut float_blurred_view: Option<&wgpu::TextureView> = None;

    // ── Stencil-write: rounded content-clip mask for tiled panes ──
    //
    // Mark each tiled pane's rounded rect (the full pane, radius =
    // `pane_border_radius`) in the stencil buffer so the content passes
    // (backdrop + text + primitives) test against it and fill to the pane edge,
    // following the rounded border. The border is drawn OUTSIDE this rect, so
    // content meets the border's inner edge with no gap (outer-border style).
    // One pass writes the union of masks; each pane's content is separately
    // scissored to its own content rect, so the union mask clips it to its own
    // rounded shape. The grid renderer appends this at its running frame offset
    // (composing with the later border/color passes). `begin_frame` was already
    // called at the frame top, so this composes correctly with Pass 3 borders.
    // Clear-once contract: `render_stencil` clears the stencil to 0 then writes
    // the mask; the content passes below `Load` it (never clear). Called exactly
    // once per frame, guarded by `!tiled_panes.is_empty()` — the app invariant is
    // always ≥1 tiled pane, so the mask is fresh every frame and the
    // `Some(stencil_view)` content passes never read a stale buffer.
    if !tiled_panes.is_empty() {
        for pane in &tiled_panes {
            let inner = heca_renderer::grid::GlowRect {
                x: pane.x,
                y: pane.y,
                w: pane.w,
                h: pane.h,
                fill: [0.0; 4],
                border: [0.0; 4],
                border_width: 0.0,
                radius: pane_border_radius,
                glow: [0.0; 4],
                glow_radius: 0.0,
                glow_intensity: 0.0,
                glow_alpha_scale: 0.0,
                shadow: [0.0; 4],
                shadow_radius: 0.0,
                shadow_offset: [0.0, 0.0],
            };
            state.grid_renderer.draw(inner);
        }
        state
            .grid_renderer
            .render_stencil(&state.queue, stencil_view, &mut encoder);
    }

    // ── Pass 2: Terminal content ──
    for pane in &tiled_panes {
        if pane.content_rect.is_some()
            && let Some(mount) = pane.mount.as_ref()
        {
            let pane_font_size = state.effective_terminal_font_size(pane.pane_id);
            let selection_overlay =
                selection_overlay_for_pane(state, pane.pane_id, &mount.snapshot);
            if blit_retained_terminal_layer(
                state,
                pane.pane_id,
                &mut encoder,
                scene_view,
                (
                    surface_physical_size.width as f32,
                    surface_physical_size.height as f32,
                ),
                mount,
                Some(stencil_view),
            )
            {
                queue_terminal_dynamic_overlays(
                    &mut state.text_renderer,
                    &mut state.primitive_renderer,
                    mount,
                    selection_overlay,
                    matches!(state.input_mode, InputMode::Selection),
                );
            } else {
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
                        stencil: Some(stencil_view),
                    },
                    TerminalStyle {
                        font_size: pane_font_size,
                        families: terminal_font_families_from(&state.font_config.family),
                        surface_alpha,
                        ligatures: state.appearance.terminal.ligatures,
                        hyperlink_style: terminal_hyperlink_style,
                        hyperlink_color: terminal_hyperlink_color,
                    },
                    mount.clone(),
                    selection_overlay,
                );
            }
        }
    }
    state.primitive_renderer.render_clipped(
        &state.device,
        scene_view,
        &mut encoder,
        content_scissor,
        Some(stencil_view),
    );

    // ── Pass 3: Pane chrome overlay (on top of terminal content) ──
    //
    // The terminal content is a rectangular raster path; drawing the shell after
    // it guarantees the border/radius/highlight stay visible instead of being
    // visually swallowed by the terminal surface.
    if !tiled_panes.is_empty() {
        let mut pane_scene = GuiScene::new();
        pane_scene.push(heca_grid_ui::scene::DrawCommand::PushClip(
            GuiRectangle::new(
                GuiPoint::new(pane_area.loc.x, pane_area.loc.y),
                GuiSize::new(pane_area.size.w, pane_area.size.h),
            ),
        ));

        for pane in &tiled_panes {
            let bcolor = if pane.is_active {
                pane_active_border_color
            } else {
                pane_border_color
            };
            paint_terminal_pane_shell(
                state,
                &mut pane_scene,
                TerminalPaneShell {
                    pane_id: pane.pane_id,
                    x: pane.x,
                    y: pane.y,
                    w: pane.w,
                    h: pane.h,
                    border_color: bcolor,
                    border_width: pane_border_width,
                    border_radius: pane_border_radius,
                    content_inset: pane_content_inset,
                    is_active: pane.is_active,
                },
            );
        }

        pane_scene.push(heca_grid_ui::scene::DrawCommand::PopClip);
        render_chrome(
            &mut state.grid_renderer,
            &mut state.text_renderer,
            &state.queue,
            &pane_scene,
            ChromePassOpts {
                damage: None,
                glow_alpha_scale,
            },
            scene_view,
            &mut encoder,
        );
    }

    // ── Blur pass 2: scene including tiled pane content ──
    //
    // Capture a second blur after tiled panes have been rendered into the scene.
    // This lets floating panes frost the actual tiled content behind them, not
    // just the chrome/background.
    if needs_floating_frost {
        let radius_physical =
            state.appearance.terminal_floating_blur_radius() * state.scale_factor as f32;
        float_blurred_view = Some(state.blur.process(
            &state.device,
            &state.queue,
            &mut encoder,
            scene_view,
            radius_physical,
        ));
    }

    // ── Stencil-write: rounded content-clip mask for floating panes ──
    //
    // A second stencil-write pass (the tiled one above has already been consumed
    // by the tiled content passes) that clears the stencil and marks the union
    // of floating panes' rounded rects (full pane, radius = `pane_border_radius`).
    // Each floating pane's backdrop + terminal content is scissored to its own
    // rect, so the union mask clips it to its own rounded shape — matching the
    // tiled clip, with no corner overflow and a filled (not transparent) margin.
    if !floating_panes.is_empty() {
        for pane in &floating_panes {
            let inner = heca_renderer::grid::GlowRect {
                x: pane.x,
                y: pane.y,
                w: pane.w,
                h: pane.h,
                fill: [0.0; 4],
                border: [0.0; 4],
                border_width: 0.0,
                radius: pane_border_radius,
                glow: [0.0; 4],
                glow_radius: 0.0,
                glow_intensity: 0.0,
                glow_alpha_scale: 0.0,
                shadow: [0.0; 4],
                shadow_radius: 0.0,
                shadow_offset: [0.0, 0.0],
            };
            state.grid_renderer.draw(inner);
        }
        state
            .grid_renderer
            .render_stencil(&state.queue, stencil_view, &mut encoder);
    }

    for pane in &floating_panes {
        // Floating panes use their own border color (distinct layer), independent
        // of the tiled active/inactive border colors.
        let fborder = pane_floating_border_color;
        if pane.content_rect.is_some() {
            // ── Floating pane backdrop (solid or frosted), rounded-clipped ──
            //
            // Floating panes get their own rounded content-clip stencil (written
            // once before this loop), so the backdrop + terminal content follow
            // the rounded border (no corner overflow) and the pane_padding margin
            // is filled, not a transparent ring. When frosted
            // (`terminal_floating_blur` > 0 + translucent), stamp the blurred tiled
            // content behind the pane. When solid (default — opaque, no frost),
            // fill the full pane with the theme `float_background` so the pane is a
            // readable solid window. Both are scissored to the pane's own rect and
            // clipped to the floating union stencil (per-pane rounded shape).
            let float_scissor = pane_scissor_rect(
                pane.x,
                pane.y,
                pane.w,
                pane.h,
                state.scale_factor,
                surface_physical_size,
            );
            if let Some(blurred) = float_blurred_view {
                let scale = state.scale_factor as f32;
                let vp_w = surface_physical_size.width as f32;
                let vp_h = surface_physical_size.height as f32;
                let (dst_x, dst_y, dst_w, dst_h) = (pane.x, pane.y, pane.w, pane.h);
                let dst = (dst_x * scale, dst_y * scale, dst_w * scale, dst_h * scale);
                state.backdrop.draw(
                    &state.device,
                    &state.queue,
                    &mut encoder,
                    scene_view,
                    blurred,
                    (vp_w, vp_h),
                    dst,
                    None,
                    1.0,
                    Some(stencil_view),
                );
            } else {
                // Solid floating backdrop: fill the full pane with the theme
                // `float_background` (theme-driven floating-window frame color) so
                // the pane is opaque/readable and the inner padding margin is
                // filled, not transparent. Rounded-clipped via the floating stencil.
                let bg = theme.float_background.to_f32x4();
                state
                    .primitive_renderer
                    .draw_rect(pane.x, pane.y, pane.w, pane.h, bg);
                state.primitive_renderer.render_clipped(
                    &state.device,
                    scene_view,
                    &mut encoder,
                    float_scissor,
                    Some(stencil_view),
                );
            }

            if let Some(mount) = pane.mount.as_ref() {
                let pane_font_size = state.effective_terminal_font_size(pane.pane_id);
                let selection_overlay =
                    selection_overlay_for_pane(state, pane.pane_id, &mount.snapshot);
                if blit_retained_terminal_layer(
                    state,
                    pane.pane_id,
                    &mut encoder,
                    scene_view,
                    (
                        surface_physical_size.width as f32,
                        surface_physical_size.height as f32,
                    ),
                    mount,
                    Some(stencil_view),
                )
                {
                    queue_terminal_dynamic_overlays(
                        &mut state.text_renderer,
                        &mut state.primitive_renderer,
                        mount,
                        selection_overlay,
                        matches!(state.input_mode, InputMode::Selection),
                    );
                    state.primitive_renderer.render_clipped(
                        &state.device,
                        scene_view,
                        &mut encoder,
                        float_scissor,
                        Some(stencil_view),
                    );
                } else {
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
                            stencil: Some(stencil_view),
                        },
                        TerminalStyle {
                            font_size: pane_font_size,
                            families: terminal_font_families_from(&state.font_config.family),
                            surface_alpha: floating_surface_alpha,
                            ligatures: state.appearance.terminal.ligatures,
                            hyperlink_style: terminal_hyperlink_style,
                            hyperlink_color: terminal_hyperlink_color,
                        },
                        mount.clone(),
                        selection_overlay,
                    );
                }
            }

            let mut float_scene = GuiScene::new();
            float_scene.push(heca_grid_ui::scene::DrawCommand::PushClip(
                GuiRectangle::new(
                    GuiPoint::new(pane_area.loc.x, pane_area.loc.y),
                    GuiSize::new(pane_area.size.w, pane_area.size.h),
                ),
            ));
            paint_terminal_pane_shell(
                state,
                &mut float_scene,
                TerminalPaneShell {
                    pane_id: pane.pane_id,
                    x: pane.x,
                    y: pane.y,
                    w: pane.w,
                    h: pane.h,
                    border_color: fborder,
                    border_width: pane_border_width,
                    border_radius: pane_border_radius,
                    content_inset: pane_content_inset,
                    is_active: pane.is_active,
                },
            );
            float_scene.push(heca_grid_ui::scene::DrawCommand::PopClip);
            render_chrome(
                &mut state.grid_renderer,
                &mut state.text_renderer,
                &state.queue,
                &float_scene,
                ChromePassOpts {
                    damage: None,
                    glow_alpha_scale,
                },
                scene_view,
                &mut encoder,
            );
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
    if chrome.left_sidebar_width > 0.0
        && chrome.left_sidebar_width < crate::chrome::SIDEBAR_EXPANDED_THRESHOLD
    {
        let sidebar_gap = chrome.sidebar_gap.max(0.0);
        let rail_x = sidebar_gap.min(chrome.left_sidebar_width * 0.5);
        let rail_y = sidebar_top + sidebar_gap;
        let rail_w = (chrome.left_sidebar_width - rail_x * 2.0).max(0.0);
        let rail_h = (sidebar_h - sidebar_gap * 2.0).max(0.0);
        state
            .primitive_renderer
            .draw_rect(rail_x, rail_y, rail_w, rail_h, collapsed_sidebar_bg);
        state.primitive_renderer.draw_outline(
            rail_x,
            rail_y,
            rail_w,
            rail_h,
            [
                theme.border.to_f32x4()[0],
                theme.border.to_f32x4()[1],
                theme.border.to_f32x4()[2],
                0.35,
            ],
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

    // Legacy hand-drawn ghost — only for the COLLAPSED rail. When the sidebar is
    // expanded the grid-ui chrome shell is painted on top (covering this), so the
    // ghost is drawn into the chrome scene instead via `paint_drag_overlay` below.
    if chrome.left_sidebar_width > 0.0
        && chrome.left_sidebar_width < crate::chrome::SIDEBAR_EXPANDED_THRESHOLD
        && let Some(label) = state
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

    if chrome.right_sidebar_width > 0.0
        && chrome.right_sidebar_width < crate::chrome::SIDEBAR_EXPANDED_THRESHOLD
    {
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
            [
                theme.border.to_f32x4()[0],
                theme.border.to_f32x4()[1],
                theme.border.to_f32x4()[2],
                0.35,
            ],
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
        .render(&state.queue, scene_view, &mut encoder, None);

    // Grid-ui chrome (full-height sidebar SHELL + status bar) painted LAST so the
    // shell sits ON TOP of the pane content/canvas instead of panes bleeding under
    // it. The collapsed left rail + right "Details" sidebar stay hand-drawn above.
    //
    // F4.1 — retained tree: rebuild the widget tree only when the chrome signature
    // changes; otherwise re-layout + paint the kept tree (no per-frame signal churn,
    // and a live tree to dispatch events into in F4.2).
    let chrome_sig = crate::chrome::chrome_signature(state, chrome);
    if state.chrome_tree.as_ref().map(|t| t.sig) != Some(chrome_sig) {
        // Drop the previous chrome tree's hint ids, then register the new tree's targets
        // into the shared allocator (spans the chrome + pane-header trees), recording the
        // contiguous id range this tree used so it can be removed on the next rebuild.
        if let Some(old) = state.chrome_tree.as_ref() {
            let old_range = old.hint_range.clone();
            state.hint_targets.remove_range(old_range);
        }
        let mut hint_targets = std::mem::take(&mut state.hint_targets);
        let start = hint_targets.checkpoint();
        let (root, signals, drag_items) =
            crate::chrome::build_chrome_root(state, chrome, &mut hint_targets);
        let hint_range = start..hint_targets.checkpoint();
        state.hint_targets = hint_targets;
        state.chrome_tree = Some(crate::chrome::RetainedChrome {
            root,
            sig: chrome_sig,
            signals,
            drag_items,
            hint_range,
        });
    }
    // Push value-state (selection + status) into the retained tree's bound signals so
    // focus/mode changes update in place without a rebuild (the signature excludes them).
    let chrome_signals_changed = crate::chrome::sync_chrome_signals(state);
    if chrome_signals_changed && let Some(tree) = state.chrome_tree.as_mut() {
        // Runtime/git signal writes happen during render, but wrappers like
        // `Visibility` apply their `style.hidden` flip in `tick()`. Advance the
        // retained chrome tree immediately so new branch/count rows participate in
        // this frame's layout + damage pass instead of waiting for a later focus/input
        // event to flush the signal-backed structure.
        tree.root.tick(0.0);
    }
    let chrome_theme = crate::chrome::chrome_gui_theme(state);
    let mut chrome_scene = crate::chrome::paint_chrome_root(
        &mut state
            .chrome_tree
            .as_mut()
            .expect("chrome tree set above")
            .root,
        w,
        h,
        &chrome_theme,
    );
    // F4.5 1b — in-drag visuals on the expanded sidebar: paint the drop indicator +
    // ghost into the chrome scene so they sit ON TOP of the grid-ui shell. (Collapsed
    // rail uses the hand-drawn ghost above.)
    if chrome.left_sidebar_width >= crate::chrome::SIDEBAR_EXPANDED_THRESHOLD {
        crate::chrome::paint_drag_overlay(state, &mut chrome_scene, w, h, &chrome_theme);
    }
    // Follow-link keycaps (prefix+Shift+o) over the focused terminal's hyperlinks,
    // painted into the chrome scene so they sit above pane content. terminal-task-18.
    crate::chrome::paint_link_hints(state, &mut chrome_scene, w, h, &chrome_theme);
    crate::chrome::paint_hint_targets(state, &mut chrome_scene, w, h, &chrome_theme);
    // Right-click context menu overlay, on top of everything. terminal-task-18.
    crate::chrome::paint_context_menu(state, &mut chrome_scene, w, h, &chrome_theme);
    crate::chrome::paint_confirm_dialog(state, &mut chrome_scene, w, h, &chrome_theme);
    // Visual-bell flash over the content area (fades out). terminal-task-17.
    crate::chrome::paint_bell_flash(state, &mut chrome_scene, pane_area, w, h, &chrome_theme);
    // Scrollback-search match highlights + query bar. terminal-task-19.
    crate::chrome::paint_search(state, &mut chrome_scene, w, h, &chrome_theme);
    render_chrome(
        &mut state.grid_renderer,
        &mut state.text_renderer,
        &state.queue,
        &chrome_scene,
        ChromePassOpts {
            // Always repaint the full chrome. The scene texture is cleared every
            // frame (the clear pass above) and panes redraw in full, so a partial
            // (damage-scissored) chrome repaint would leave the rest of the chrome
            // (sidebars + tab/status bars) as bare background for that frame — the
            // dark "re-render" flash seen mid-animation (e.g. the split button's
            // press flash). Partial chrome is only sound with a *preserved* scene,
            // which this render path does not keep. See PLAN.md "Damage-region
            // render" for the deferred optimization that would make it sound.
            damage: None,
            glow_alpha_scale,
        },
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
        tab_bar_height: state.tab_bar_height(),
        status_bar_height: state.status_bar_height(),
        left_sidebar_width: state.left_sidebar_width(),
        right_sidebar_width: state.right_sidebar_width(),
        sidebar_gap: state.appearance.effective_sidebar_gap(&state.theme),
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
            (
                "TAKE+",
                " — Select a pane to pull into the active column, then focus it.".to_string()
            )
        );

        assert_eq!(
            status_mode_parts(&InputMode::ConfirmDelete {
                message: "Delete pane?".to_string(),
                action: Box::new(WmAction::ClosePane),
                resume_sidebar: false,
            }),
            ("CONFIRM", String::new()),
            "the prompt lives in the Modal now — the status bar shows only the mode word"
        );
    }
}
