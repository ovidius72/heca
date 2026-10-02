//! Render and viewport helpers.
//!
//! These helpers keep low-level pane rendering and viewport synchronization out
//! of `main.rs` while preserving the current render pipeline behavior.

use crate::app::scene_flush::{ChromePassOpts, flush_overlay_band, flush_scene};
use crate::app::terminal_render::{
    PaneRenderState, TerminalTarget, draw_surface, hyperlink_decor_from, paint_pane_frame,
    pane_scissor_rect, sync_retained_terminal_layers, terminal_font_families_from,
};
use crate::app_state::{AppState, InputMode};
use crate::chrome::ChromeConfig;
use crate::mouse;
use heca_grid_ui::Component;
use heca_grid_ui::{
    Point as GuiPoint, Rectangle as GuiRectangle, Scene as GuiScene, Size as GuiSize,
};
use heca_renderer::terminal::TerminalStyle;

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

    let chrome = ChromeConfig::of(state);
    let pane_area = chrome.content_rect();
    state.text_renderer.begin_frame();
    state.text_renderer.set_damage(None);
    state.text_renderer.set_clip(None);
    // Reset the grid renderer's persistent vertex/index buffer offsets once per
    // frame so each `render_chrome` pass appends at a distinct region (see the
    // note in `render_chrome`).
    state.grid_renderer.begin_frame();

    // ── Pane chrome from pane-specific config ──
    // The three frame colours are **not** read here. A pane's frame colour is its own — written
    // onto its retained shell by `chrome::sync_panes`, which is also where it becomes the hue the
    // pane publishes to its contents. Reading them here meant the host decided how a widget looked.
    let pane_border_radius = state.appearance.effective_pane_border_radius(&state.theme);
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

    // **The columns are painted first**, before anything is measured against them. A terminal paints
    // one surface request at its own box, so this scene is where every terminal's real position is
    // — below its header, scrolled, clipped — and the host reads it from there instead of working
    // it out from the pane's rect. The same scene is flushed in Pass 3; it is painted once.
    let mut column_scene = GuiScene::new();
    column_scene.push(heca_grid_ui::scene::DrawCommand::PushClip(
        GuiRectangle::new(
            GuiPoint::new(pane_area.loc.x, pane_area.loc.y),
            GuiSize::new(pane_area.size.w, pane_area.size.h),
        ),
    ));
    {
        // **The columns' own letters ride in this scene too.** Through `paint_child`, never
        // `paint`: `paint_child` is what draws a widget's hint letter after painting it.
        let column_theme = crate::chrome::chrome_gui_theme(state);
        let mut cx = heca_grid_ui::PaintCx::new(&mut column_scene, &column_theme);
        for column in state.columns.values() {
            heca_grid_ui::paint_child(&column.root, &mut cx);
        }
    }
    column_scene.push(heca_grid_ui::scene::DrawCommand::PopClip);
    // **A floating pane's frame is painted early for the same reason**: its terminal says where it
    // is in the scene its own frame paints into. Each is flushed in its own pass below, between its
    // backdrop and what goes over it.
    let mut float_frames: std::collections::HashMap<heca_core::layout::PaneId, GuiScene> =
        std::collections::HashMap::new();
    if let Some(ws) = state.session.active_workspace() {
        for float in &ws.floating_panes {
            let mut frame = GuiScene::new();
            frame.push(heca_grid_ui::scene::DrawCommand::PushClip(
                GuiRectangle::new(
                    GuiPoint::new(pane_area.loc.x, pane_area.loc.y),
                    GuiSize::new(pane_area.size.w, pane_area.size.h),
                ),
            ));
            paint_pane_frame(state, &mut frame, float.pane.id);
            frame.push(heca_grid_ui::scene::DrawCommand::PopClip);
            float_frames.insert(float.pane.id, frame);
        }
    }
    let frames: Vec<&GuiScene> = std::iter::once(&column_scene)
        .chain(float_frames.values())
        .collect();
    crate::chrome::terminal::place_from(state, &frames);

    let tiled_panes: Vec<PaneRenderState> = pane_positions
        .iter()
        .map(|(pane_id, rect)| {
            let px = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
            let py = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
            PaneRenderState::collect(
                state,
                *pane_id,
                (px, py, rect.size.w as f32, rect.size.h as f32),
            )
        })
        .collect();

    let terminal_font_config = state.font_config.clone();
    // Effective global terminal size (config + global zoom). Per-pane offsets are
    // applied inside `sync_retained_terminal_layers`; this is the base/fallback.
    let app_font_size = state.app_font_size();
    let float_boxes: Vec<(heca_core::layout::PaneId, (f32, f32, f32, f32))> = state
        .session
        .active_workspace()
        .map(|ws| {
            ws.floating_panes
                .iter()
                .map(|float| {
                    let x = float.position.x as f32 + pane_area.loc.x as f32 + ws_offset.0;
                    let y = float.position.y as f32 + pane_area.loc.y as f32 + ws_offset.1;
                    (
                        float.pane.id,
                        (x, y, float.size.w as f32, float.size.h as f32),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let floating_panes: Vec<PaneRenderState> = float_boxes
        .into_iter()
        .map(|(pane_id, at)| PaneRenderState::collect(state, pane_id, at))
        .collect();

    // The chip and the scrollbar are children of each terminal and were painted above, before this
    // frame's snapshots were read: a change shows on the next frame, so ask for one.
    if crate::chrome::terminal::show_viewports(
        state,
        tiled_panes
            .iter()
            .chain(floating_panes.iter())
            .filter_map(|p| p.mount.as_ref().map(|m| (p.pane_id, &m.snapshot))),
    ) {
        state.mark_full_redraw();
    }

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
    // Cloned (a handle, not the texture) so the frame can hand `&mut state` to the flush while it
    // holds the views.
    let scene_tex = state.compositor.scene_view().clone();
    let scene_view = &scene_tex;
    // Stencil buffer paired with the scene texture: holds the rounded content-clip
    // mask written each frame so terminal content follows the pane's rounded border.
    let stencil_tex = state.compositor.stencil_view().clone();
    let stencil_view = &stencil_tex;

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render"),
        });

    let float_background = state.theme.float_background.to_f32x4();

    let bg = state.theme.background.to_linear_f32x4();
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
            .effective_background_gradient_top(&state.theme)
            .to_linear_f32x4();
        let bottom = state
            .appearance
            .effective_background_gradient_bottom(&state.theme)
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
    // A handle to the blurred texture, not a borrow of `state.blur`: the float loop below hands
    // `&mut state` to the flush while it still needs the blur.
    let mut float_blurred_view: Option<wgpu::TextureView> = None;

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

    // Overlay content (hover tooltips, popovers) from every surface — panes, floats, chrome —
    // is collected here and flushed once at the very end, above all bases (the surface-compositor
    // top band). Declared before the first surface flush; consumed after the last.
    let mut overlay_sink: Vec<heca_grid_ui::Scene> = Vec::new();
    let chrome_pass = ChromePassOpts {
        damage: None,
        glow_alpha_scale,
    };

    // ── Pass 2: the columns, with each terminal drawn where the scene puts it ──
    //
    // A terminal is a surface in the scene its pane paints, so it is drawn when the flush reaches
    // it: what the scene drew before it is under it, what it draws after — the chip, the scrollbar,
    // the pane's own border — is over it. Nothing here orders terminals against chrome.
    if !tiled_panes.is_empty() {
        let tiled_target = TerminalTarget {
            view: scene_view,
            stencil: stencil_view,
            scissor: content_scissor,
            surface_alpha,
            content_clip: pane_area,
        };
        flush_scene(
            state,
            &column_scene,
            &chrome_pass,
            scene_view,
            &mut encoder,
            &mut overlay_sink,
            &mut |state, id, encoder| draw_surface(state, &tiled_panes, id, &tiled_target, encoder),
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
        float_blurred_view = Some(
            state
                .blur
                .process(
                    &state.device,
                    &state.queue,
                    &mut encoder,
                    scene_view,
                    radius_physical,
                )
                .clone(),
        );
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
            if let Some(blurred) = &float_blurred_view {
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
                let bg = float_background;
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

            // The float's own scene, in order: its terminal where the scene puts it, then what is
            // drawn over it.
            if let Some(frame) = float_frames.remove(&pane.pane_id) {
                let float_target = TerminalTarget {
                    view: scene_view,
                    stencil: stencil_view,
                    scissor: float_scissor,
                    surface_alpha: floating_surface_alpha,
                    content_clip: pane_area,
                };
                flush_scene(
                    state,
                    &frame,
                    &chrome_pass,
                    scene_view,
                    &mut encoder,
                    &mut overlay_sink,
                    &mut |state, id, encoder| {
                        draw_surface(state, &floating_panes, id, &float_target, encoder)
                    },
                );
            }
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
    crate::chrome::sync_chrome_signals(state);
    // **Flush signal-driven structure before this frame is laid out.**
    //
    // Wrappers like `Visibility` apply their `hidden` flip in `tick`, so a row revealed by a
    // signal has no box until one runs. The frame pass already ticked, but that was before the
    // store was brought up to date — and a pane's runtime now reaches its row through the row's
    // own subscription, which fires during that update. Without this the reveal would land a
    // frame late, and it used to be skipped entirely whenever the sync pass reported no change.
    //
    // Unconditional, because "did anything change" is no longer a question one return value can
    // answer once rows subscribe for themselves. A tick with no time and nothing pending is a
    // walk that finds nothing.
    state.window_root.tick(0.0);
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
    // Always repaint the full chrome (`chrome_pass` has no damage). The scene texture is cleared every
    // frame (the clear pass above) and panes redraw in full, so a partial (damage-scissored) chrome
    // repaint would leave the rest of the chrome (sidebars + tab/status bars) as bare background for
    // that frame — the dark "re-render" flash seen mid-animation (e.g. the split button's press
    // flash). Partial chrome is only sound with a *preserved* scene, which this render path does not
    // keep. See PLAN.md "Damage-region render" for the deferred optimization that would make it
    // sound.
    //
    // A terminal surface in this scene is a dock's or an overlay's, not a pane's: none is
    // placeable yet (P094(F011)/T449 slice 5 adds the name-keyed process it needs), so the surface
    // finder is given no panes and a surface here is left undrawn.
    let dock_target = TerminalTarget {
        view: scene_view,
        stencil: stencil_view,
        scissor: content_scissor,
        surface_alpha,
        content_clip: pane_area,
    };
    flush_scene(
        state,
        &chrome_scene,
        &chrome_pass,
        scene_view,
        &mut encoder,
        &mut overlay_sink,
        &mut |state, id, encoder| draw_surface(state, &[], id, &dock_target, encoder),
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
    flush_overlay_band(
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
    let pane_area = ChromeConfig::of(state).content_rect();
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
