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
use crate::mouse;
use heca_grid_ui::Component;
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
///
/// Chrome focus deliberately says **nothing** here (user, 2026-07-30): this bar is temporary and is
/// being replaced, and a dock's name sitting where an input mode's word goes reads as a mode when it
/// is not one. The affordance for "the keys are going there" is the **focus ring**, which is on the
/// thing itself rather than in a corner.
pub(crate) fn status_mode_parts(
    input_mode: &InputMode,
    catalog: &crate::actions::ActionCatalog,
) -> (&'static str, String) {
    // For pick modes the prompt suffix is sourced from the action's `ActionDescriptor`
    // (via `pending_pick`) so the text lives in one place — the action catalog.
    let pick_suffix = || {
        input_mode
            .pending_pick(catalog)
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
        InputMode::Chord { sequence } => ("CHORD", format!(" w→{}", sequence.join("→"))),
        InputMode::Mode { name } => ("MODE", format!(" {} → ?", name)),
        // The confirm now lives entirely in the Modal dialog; the status bar only
        // shows the mode word, no duplicated prompt.
        InputMode::ConfirmDelete => ("CONFIRM", String::new()),
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
        InputMode::DockPick { .. } => ("FOCUS DOCK", pick_suffix()),
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

#[expect(
    clippy::too_many_arguments,
    reason = "GPU flush pass threads renderers + queue + view + encoder + scene + overlay sink explicitly"
)]
fn render_chrome(
    grid: &mut GridRenderer,
    text: &mut TextRenderer,
    queue: &wgpu::Queue,
    scene: &heca_grid_ui::Scene,
    opts: ChromePassOpts,
    view: &wgpu::TextureView,
    encoder: &mut wgpu::CommandEncoder,
    // Overlay content (a button's hover Tooltip, a popover) is NOT flushed with this surface —
    // it is collected here and flushed once, above every surface, by `render_overlay_band` at
    // the end of the frame. This makes overlays a real **top band** (the surface-compositor
    // paint-z-order model, docs/surface-compositor.md), so e.g. a pane-header tooltip is no
    // longer occluded by a neighbouring pane or the sidebar that flush after it.
    overlay_sink: &mut Vec<heca_grid_ui::Scene>,
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
    // Defer overlay segments to the frame-final top band (see the param doc + `render_overlay_band`).
    overlay_sink.extend(scene.overlay_segments());
}

/// Flush the collected overlay segments from every surface, in accumulation order (panes →
/// floats → chrome, so higher surfaces' overlays sit on top), **above all surface bases**.
/// This is the overlay **top band** of the surface compositor's paint z-order: a tooltip /
/// popover always paints over every pane, float, and the chrome/sidebars, never occluded by a
/// surface that flushed after its own. Full repaint (no damage/clip); the segments already
/// carry their own geometry.
fn render_overlay_band(
    grid: &mut GridRenderer,
    text: &mut TextRenderer,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    encoder: &mut wgpu::CommandEncoder,
    overlays: &[heca_grid_ui::Scene],
    glow_alpha_scale: f32,
) {
    grid.set_damage(None);
    grid.set_clip(None);
    text.set_damage(None);
    for seg in overlays {
        heca_renderer::scene::enqueue_scene(grid, text, seg, glow_alpha_scale);
        grid.render(queue, view, encoder);
        text.render(queue, view, encoder, None);
    }
}

/// Render the full frame for the current app state.
pub(crate) fn render_frame(state: &mut AppState) {
    // Mount whatever a widget asked to open since the last frame: a declared context menu is a
    // layer, and this is the one moment the host has `&mut AppState` and has not yet drawn.
    crate::chrome::drain_pending_menus(state);
    // And the drags that finished: the same shape, drained in the same breath.
    crate::chrome::drain_pending_drops(state);
    if !state.needs_redraw {
        return;
    }
    state.needs_redraw = false;
    let _ = crate::chrome::sync_chrome_state(state);
    // Build/position the retained per-pane info-bar headers *before* the GPU borrow
    // below (`scene_view` borrows `state.compositor`), so render can paint them
    // read-only and `mouse.rs` can dispatch pointer events into them.
    crate::chrome::sync_pane_headers(state);
    // The retained per-pane shells — the frame, the pane's identity and its pick letter. Same
    // moment and same reason as the headers: built before the GPU borrow so render can paint them
    // read-only (F011/P094/T451).
    crate::chrome::sync_panes(state);

    let phys_size = state.window.inner_size();
    let scale = state.scale_factor as f32;
    let w = phys_size.width as f32 / scale;
    let h = phys_size.height as f32 / scale;

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

    let tb = &chrome;
    // Frosted chrome colors come from the loaded theme's surface tone, with
    // alpha derived from the current appearance settings.
    let (side_bg, _, _) = crate::chrome::chrome_colors(state);
    let side_bg = side_bg.to_f32x4();
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

    // Overlay content (hover tooltips, popovers) from every surface — panes, floats, chrome —
    // is collected here and flushed once at the very end, above all bases (the surface-compositor
    // top band). Declared before the first surface flush; consumed after the last.
    let mut overlay_sink: Vec<heca_grid_ui::Scene> = Vec::new();

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
            &mut overlay_sink,
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
                &mut overlay_sink,
            );
        }
    }

    let pane_area_rect = heca_core::layout::Rectangle::new(
        heca_core::layout::Point::new(pane_area.loc.x, pane_area.loc.y),
        heca_core::layout::Size::new(pane_area.size.w, pane_area.size.h),
    );

    // The left sidebar is drawn by the grid-ui chrome scene (`build_chrome_scene`)
    // when Expanded; when Hidden its width is 0 and nothing is drawn. There is no
    // collapsed icon rail (dropped — see `docs/sidebar-provider-modes.md`).

    mouse::render_detached_pane(state, pane_area_rect);
    mouse::render_insert_hint(state, pane_area_rect);

    // Reborrow compositor scene texture for the final flush (the previous
    // `scene_view` borrow ended at its last use before the mouse:: calls above).
    let scene_view = state.compositor.scene_view();

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
        let (chrome_root, signals, drag_items, intent_source) =
            crate::chrome::build_chrome_root(state, chrome);
        // **Seat the chrome subtree, keep the window root.** Every surface hangs beside the chrome
        // rather than inside it, so a rebuild — a resize, a sidebar toggle, a theme reload — leaves
        // them untouched.
        crate::chrome::seat_chrome(&mut state.window_root, chrome_root);
        state.chrome_tree = Some(crate::chrome::RetainedChrome {
            sig: chrome_sig,
            signals,
            drag_items,
            intent_source,
        });
    }
    // Push value-state (selection + status) into the retained tree's bound signals so
    // focus/mode changes update in place without a rebuild (the signature excludes them).
    let chrome_signals_changed = crate::chrome::sync_chrome_signals(state);
    if chrome_signals_changed {
        // Runtime/git signal writes happen during render, but wrappers like
        // `Visibility` apply their `style.hidden` flip in `tick()`. Advance the
        // retained chrome tree immediately so new branch/count rows participate in
        // this frame's layout + damage pass instead of waiting for a later focus/input
        // event to flush the signal-backed structure.
        state.window_root.tick(0.0);
    }
    let chrome_theme = crate::chrome::chrome_gui_theme(state);
    let mut chrome_scene =
        crate::chrome::paint_chrome_root(&mut state.window_root, w, h, &chrome_theme);
    // **A drag draws itself.** The insertion line, the swap outline and the picture of the thing
    // under the pointer are painted by the widgets the drag passes through, inside `paint_child` —
    // so nothing opts in, and a plugin's own row gets the same feedback (F003/P097/T496). The host
    // pass that drew this for the left sidebar alone is gone.
    // Follow-link keycaps (prefix+Shift+o) over the focused terminal's hyperlinks,
    // painted into the chrome scene so they sit above pane content. terminal-task-18.
    crate::chrome::paint_link_hints(state, &mut chrome_scene, w, h, &chrome_theme);
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
        &mut overlay_sink,
    );

    // ── Dynamic layers (overlay dialogs, the context menu, plugin panels, the exposé) ──
    //
    // **They were painted here, into a scene of their own. They are not any more.** Every surface is
    // a child of the window root, so `paint_chrome_root` above already walked it — one tree, one
    // paint (`docs/surface-compositor.md` § 0.8). What is left is the GPU work the walk recorded.
    //
    // The z-order that pass used to arrange by hand is now child order: a surface is placed after
    // the chrome, so it paints above the bell flash and the search highlights, which is what it did
    // before by being flushed later.
    let layer_scene = &chrome_scene;
    // **Perform the host work the scene recorded** (`docs/surface-compositor.md` § 0.5).
    //
    // The host asks nothing about layers here and knows no surface by name. A node that wants its
    // backdrop blurred records the request while it paints — `Overlay::frosted` is the one that
    // does today — and this performs it: blur the scene texture, stamp it back. Between the base
    // flush and the overlay flush is exactly "after everything beneath the surface, before the
    // surface", which is what a backdrop means and the reason the request goes in the base band.
    //
    // `HostDraw::Surface` is not emitted yet — the terminal still blits through its retained path
    // until `P094(F011)/T449` makes it a component.
    for req in heca_renderer::scene::host_requests(layer_scene) {
        let heca_grid_ui::scene::HostDraw::Backdrop { radius } = req.draw else {
            continue;
        };
        let sf = state.scale_factor as f32;
        let vp_w = phys_size.width as f32;
        let vp_h = phys_size.height as f32;
        // Logical → physical, then intersect with the clip in force where it was recorded.
        let (mut x, mut y, mut bw, mut bh) = (
            req.rect.loc.x as f32 * sf,
            req.rect.loc.y as f32 * sf,
            req.rect.size.w as f32 * sf,
            req.rect.size.h as f32 * sf,
        );
        if let Some(c) = req.clip {
            let x1 = (x + bw).min((c[0] + c[2]) * sf);
            let y1 = (y + bh).min((c[1] + c[3]) * sf);
            x = x.max(c[0] * sf);
            y = y.max(c[1] * sf);
            bw = (x1 - x).max(0.0);
            bh = (y1 - y).max(0.0);
        }
        if radius <= 0.0 || req.alpha <= 0.0 || bw <= 0.0 || bh <= 0.0 {
            continue;
        }
        let blurred = state.blur.process(
            &state.device,
            &state.queue,
            &mut encoder,
            scene_view,
            radius * sf,
        );
        state.backdrop.draw(
            &state.device,
            &state.queue,
            &mut encoder,
            scene_view,
            blurred,
            (vp_w, vp_h),
            // `None` src-uv samples the same screen location as the destination, so the blur is of
            // exactly what sits behind the rect.
            (x, y, bw, bh),
            None,
            // The frost fades with the layer that asked for it. Left at full strength it would hold
            // the whole session out of focus for the length of the fade and then snap back sharp in
            // one frame — the exact pop the fade exists to remove.
            req.alpha,
            None,
        );
    }
    // **The picker's keycaps are NOT painted here, and must never be.** Each is drawn by the widget
    // that declared the pick, in that widget's own paint (`heca_grid_ui::offer_hint`).
    //
    // Twice now a host pass tried to draw them and put them somewhere the user could not see: first
    // into the chrome scene, flushed before the layers, so the letters sat under the exposé
    // (F003/P082/T416); then into this scene's **base**, while an `Overlay`-rooted layer paints
    // into an overlay segment deferred to a later band — under the map again, at the right
    // coordinates (F003/P082/T427). A host cannot know which half of which scene a widget it has
    // never seen paints into, so it must not try.
    // **There is no second flush.** The surfaces were painted by the same walk as the chrome, into
    // the same scene, and that scene was flushed above — a second `render_chrome` here would draw
    // the whole frame twice. What is left of the old ordering is where the frost is performed:
    // after the base flush, before the overlay band, which is the moment a backdrop means.

    // Top band: every surface's overlay content (tooltips, popovers), above all bases.
    render_overlay_band(
        &mut state.grid_renderer,
        &mut state.text_renderer,
        &state.queue,
        scene_view,
        &mut encoder,
        &overlay_sink,
        glow_alpha_scale,
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
    use crate::actions::ActionCatalog;
    use crate::app_state::InputMode;
    use heca_core::layout::PaneId;

    #[test]
    fn status_mode_parts_formats_take_and_confirm() {
        let catalog = ActionCatalog::with_builtins();
        assert_eq!(
            status_mode_parts(
                &InputMode::PaneTake {
                    candidates: vec![("a".chars().next().expect("candidate label"), PaneId(1))],
                    focus_after: true,
                },
                &catalog
            ),
            (
                "TAKE+",
                " — Select a pane to pull into the active column, then focus it.".to_string()
            )
        );

        assert_eq!(
            status_mode_parts(&InputMode::ConfirmDelete, &catalog),
            ("CONFIRM", String::new()),
            "the prompt lives in the Modal now — the status bar shows only the mode word"
        );
    }
}
