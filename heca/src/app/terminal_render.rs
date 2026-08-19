//! Terminal pane shell + content rendering helpers.
//!
//! This module keeps terminal-specific pane composition out of the top-level
//! frame orchestrator. `render.rs` owns frame ordering and geometry collection;
//! this module owns the `Pane` shell contract and terminal content mounting.

use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionState};
use crate::app::terminal_host::TerminalMount;
use crate::app_state::AppState;
use heca_config::theme::Color;
use heca_core::backend::{TerminalDamage, TerminalRowRange, TerminalSnapshot};
use heca_core::layout::{PaneId, Point, Rectangle, Size};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::{
    Color as GuiColor, Component, PaintCx, Point as GuiPoint,
    Rectangle as GuiRectangle, Scene as GuiScene, Size as GuiSize,
};
use heca_renderer::image::ImageLayer;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::terminal::{
    CaretIndicator, SelectionOverlay, SelectionOverlaySpan, TerminalRenderer, TerminalStyle,
};
use heca_renderer::text::{TextBox, TextRenderer};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

pub(crate) struct PaneRenderState {
    pub(crate) pane_id: PaneId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    pub(crate) is_active: bool,
    pub(crate) content_rect: Option<Rectangle>,
    pub(crate) mount: Option<TerminalMount>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalCopyBand {
    pub(crate) y: u32,
    pub(crate) height: u32,
}

pub(crate) fn sync_retained_terminal_layers(
    state: &mut AppState,
    panes: &[PaneRenderState],
    terminal_style: TerminalStyle<'_>,
    window_logical_size: (f32, f32),
    window_physical_size: winit::dpi::PhysicalSize<u32>,
) {
    retain_live_terminal_layers(state);

    // One image-cache generation per frame; upload marks images used, then stale
    // textures (scrolled-off / closed panes) are evicted after the pane loop.
    state.image_renderer.begin_frame();
    // Whether any visible pane shows an animated image, so the redraw loop keeps
    // ticking frames (`terminal-task-24`).
    let mut any_animated = false;

    for pane in panes {
        let Some(mount) = pane.mount.as_ref() else {
            state.terminal_layers.remove(&pane.pane_id);
            continue;
        };

        // Per-pane font zoom: start from the shared style and override the font
        // size for this pane. The render key already folds in `font_size`, so a
        // zoom change on one pane repaints only that pane's retained layer.
        let mut pane_style = terminal_style;
        pane_style.font_size = state.effective_terminal_font_size(pane.pane_id);
        let render_key = terminal_layer_render_key(&pane_style);

        let physical_size = retained_terminal_texture_size(mount.content_rect, state.scale_factor);
        // The offscreen scratch must match the pane's exact physical size. A
        // larger reused target would render the terminal at the wrong pixel
        // density and then crop the top-left subset during the copy into the
        // retained layer, which showed up as oversized glyphs while resizing and
        // hidden freshly typed content when damaged frames were presented live.
        state.terminal_layer_scratch.ensure_size(
            &state.device,
            state.surface_config.format,
            physical_size.width,
            physical_size.height,
        );

        let layer = state
            .terminal_layers
            .entry(pane.pane_id)
            .or_insert_with(|| {
                crate::app_state::RetainedTerminalLayer::new(
                    &state.device,
                    state.surface_config.format,
                    physical_size.width,
                    physical_size.height,
                    render_key,
                )
            });

        let resized = layer.ensure_size(
            &state.device,
            state.surface_config.format,
            physical_size.width,
            physical_size.height,
        );
        let style_changed = layer.render_key != render_key;
        if style_changed {
            layer.render_key = render_key;
        }

        let graphics_sig = graphics_signature(&mount.snapshot.graphics);
        let graphics_changed = layer.graphics_sig != graphics_sig;
        let image_rows_now = image_row_ranges(&mount.snapshot.graphics, mount.snapshot.rows);

        // Upload new images and advance any animated ones (`terminal-task-24`).
        // Done before the damage decision so a frame advance — which changes pixels
        // without changing the placement signature — can damage the image's rows.
        let anim_advanced =
            state
                .image_renderer
                .upload_images(&state.device, &state.queue, &mount.snapshot.images);
        any_animated |= mount.snapshot.images.iter().any(|img| img.is_animated());

        // The retained layer holds the last frame's content. The decision below
        // is the whole point of the retained-content foundation (`terminal-00b`):
        // when there is nothing to do we skip the update entirely so unchanged
        // rows stay visible; otherwise only the dirty rows — text damage plus the
        // rows the inline images cover (`terminal-task-23`) — are re-rendered. A
        // resize or style change still forces a full repaint. An image placement
        // change or animation frame advance damages the image rows (old ∪ new).
        let Some(damage) = retained_damage_to_apply(
            resized,
            style_changed,
            graphics_changed || anim_advanced,
            &mount.damage,
            &image_rows_now,
            &layer.image_rows,
        ) else {
            continue;
        };
        layer.graphics_sig = graphics_sig;
        layer.image_rows = image_rows_now;

        render_terminal_layer_update(
            state,
            pane.pane_id,
            mount,
            &damage,
            pane_style,
            window_logical_size,
            window_physical_size,
        );
    }

    // OR (never assign): this runs once for tiled panes and once for floating
    // panes per frame, so assigning would let the second call clobber the first.
    // `render_frame` resets the flag to false before the tiled call each frame.
    state.has_animated_images |= any_animated;

    // Drop image textures not referenced for a while (panes scrolled past the
    // image or closed). Generous grace so re-scrolling to a recent image does not
    // re-upload it; the retained layer keeps showing on-screen images regardless.
    state.image_renderer.evict_unused(IMAGE_TEXTURE_MAX_AGE_FRAMES);
}

/// Frames an unreferenced inline-image texture survives before eviction (~10s at
/// 60fps). The retained layer still shows any on-screen image; this only bounds
/// GPU memory for images no longer being re-rendered.
const IMAGE_TEXTURE_MAX_AGE_FRAMES: u64 = 600;

/// Decide what damage to re-render into a retained terminal layer this frame.
///
/// Returns `None` to skip the update entirely — the retained layer already
/// holds the last frame's content, so unchanged rows stay visible and only the
/// dirty rows need repainting. Returns `Some(Full)` when the layer was resized or
/// its style render-key changed (a structural change forces every row to be
/// repainted, otherwise the resized/retinted grid would show stale content).
/// Otherwise the backend's per-frame damage passes through unchanged, so only
/// the rows it reports are redrawn.
///
/// This is the pure policy behind `sync_retained_terminal_layers`; extracting it
/// keeps the retained-presentation contract unit-testable without a GPU.
///
/// `image_rows_now` / `image_rows_prev` are the visible row ranges the pane's
/// inline images cover this frame and last frame. Per-image damage
/// (`terminal-task-23`): an image change (appear / move / clear / animation frame
/// advance) damages only those rows (new ∪ old) instead of the whole pane, and a
/// text change on a pane holding images only re-blits the image where the changed
/// text actually overlaps it.
fn retained_damage_to_apply(
    resized: bool,
    style_changed: bool,
    graphics_changed: bool,
    mount_damage: &TerminalDamage,
    image_rows_now: &[TerminalRowRange],
    image_rows_prev: &[TerminalRowRange],
) -> Option<TerminalDamage> {
    // A structural change (resize / style) still repaints every row; the per-row
    // path can't reason about the whole grid moving or re-shaping.
    if resized || style_changed {
        return Some(TerminalDamage::Full);
    }
    // A `Full` text damage subsumes any image rows.
    if matches!(mount_damage, TerminalDamage::Full) {
        return Some(TerminalDamage::Full);
    }

    let text_rows: &[TerminalRowRange] = match mount_damage {
        TerminalDamage::Rows(rows) => rows,
        _ => &[],
    };

    let mut damage: Vec<TerminalRowRange> = text_rows.to_vec();
    if graphics_changed {
        // Appear / move / clear / frame advance: repaint the union of the old and
        // new image rows (old so a removed/moved image leaves no stale pixels).
        damage.extend_from_slice(image_rows_now);
        damage.extend_from_slice(image_rows_prev);
    } else if !text_rows.is_empty() && ranges_overlap(text_rows, image_rows_now) {
        // Text changed under/over an image: re-blit the image on those rows so the
        // glyph pass doesn't overwrite it (or leave the image's old pixels).
        damage.extend_from_slice(image_rows_now);
    }

    let merged = merge_row_ranges(damage);
    if merged.is_empty() {
        None
    } else {
        Some(TerminalDamage::Rows(merged))
    }
}

/// Visible row ranges an image placement list covers, clamped to `[0, rows)`.
fn image_row_ranges(
    graphics: &[heca_core::backend::GraphicsPlacement],
    rows: usize,
) -> Vec<TerminalRowRange> {
    let mut ranges: Vec<TerminalRowRange> = graphics
        .iter()
        .filter_map(|g| {
            let start = g.row.min(rows);
            let end = g.row.saturating_add(g.rows).min(rows);
            (end > start).then_some(TerminalRowRange::new(start, end))
        })
        .collect();
    ranges = merge_row_ranges(ranges);
    ranges
}

/// Whether any range in `a` overlaps any range in `b` (half-open `[start, end)`).
fn ranges_overlap(a: &[TerminalRowRange], b: &[TerminalRowRange]) -> bool {
    a.iter()
        .any(|ra| b.iter().any(|rb| ra.start < rb.end && rb.start < ra.end))
}

/// Sort and coalesce adjacent/overlapping row ranges into a minimal set.
fn merge_row_ranges(mut ranges: Vec<TerminalRowRange>) -> Vec<TerminalRowRange> {
    ranges.retain(|r| r.end > r.start);
    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<TerminalRowRange> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match merged.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => merged.push(r),
        }
    }
    merged
}

/// Stable signature of a pane's inline-image placements, so a layer can detect
/// when images appear, move, resize, or clear and force a full repaint.
fn graphics_signature(graphics: &[heca_core::backend::GraphicsPlacement]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    graphics.len().hash(&mut hasher);
    for g in graphics {
        g.image_id.hash(&mut hasher);
        g.row.hash(&mut hasher);
        g.col.hash(&mut hasher);
        g.cols.hash(&mut hasher);
        g.rows.hash(&mut hasher);
        g.z_index.hash(&mut hasher);
        for v in g
            .src_top_left
            .iter()
            .chain(g.src_bottom_right.iter())
        {
            v.to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}

pub(crate) fn blit_retained_terminal_layer(
    state: &AppState,
    pane_id: PaneId,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    viewport_px: (f32, f32),
    mount: &TerminalMount,
    stencil: Option<&wgpu::TextureView>,
) -> bool {
    let Some(layer) = state.terminal_layers.get(&pane_id) else {
        return false;
    };
    let rect = mount.content_rect;
    let scale = state.scale_factor as f32;
    state.backdrop.draw(
        &state.device,
        &state.queue,
        encoder,
        target,
        layer.view(),
        viewport_px,
        (
            rect.loc.x as f32 * scale,
            rect.loc.y as f32 * scale,
            rect.size.w as f32 * scale,
            rect.size.h as f32 * scale,
        ),
        Some([0.0, 0.0, 1.0, 1.0]),
        1.0,
        stencil,
    );
    true
}

pub(crate) fn queue_terminal_dynamic_overlays(
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    mount: &TerminalMount,
    selection_overlay: Option<SelectionOverlay>,
    hide_cursor: bool,
) {
    let content_box = rect_to_text_box(mount.content_rect);
    let mut terminal_renderer = TerminalRenderer::new(text_renderer, primitive_renderer);
    if let Some(ref overlay) = selection_overlay {
        terminal_renderer.render_selection_overlay(
            overlay,
            content_box,
            mount.snapshot.cell_w,
            mount.snapshot.cell_h,
        );
    }
    // Hide the host terminal cursor when Selection mode is active so the
    // selection caret (rendered by the SelectionOverlay) is the only cursor visible.
    if !hide_cursor {
        terminal_renderer.render_cursor_overlay(&mount.snapshot, content_box);
    }
}

fn retain_live_terminal_layers(state: &mut AppState) {
    let mut live_panes = HashSet::new();
    for ws in &state.session.workspaces {
        for col in &ws.scrolling.columns {
            for pane in &col.panes {
                live_panes.insert(pane.id);
            }
        }
        for float in &ws.floating_panes {
            live_panes.insert(float.pane.id);
        }
    }
    state
        .terminal_layers
        .retain(|pane_id, _| live_panes.contains(pane_id));
    // Per-pane font-zoom state is keyed by pane; drop it for closed panes so the
    // maps don't leak entries across the app's lifetime.
    state
        .pane_font_zoom
        .retain(|pane_id, _| live_panes.contains(pane_id));
    state
        .pane_cell_override
        .retain(|pane_id, _| live_panes.contains(pane_id));
}

fn terminal_layer_render_key(style: &TerminalStyle<'_>) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    style.font_size.to_bits().hash(&mut hasher);
    style.surface_alpha.to_bits().hash(&mut hasher);
    style.families.normal.hash(&mut hasher);
    style.families.bold.hash(&mut hasher);
    style.families.italic.hash(&mut hasher);
    style.families.bold_italic.hash(&mut hasher);
    style.ligatures.hash(&mut hasher);
    hasher.finish()
}

fn retained_terminal_texture_size(
    content_rect: Rectangle,
    scale_factor: f64,
) -> winit::dpi::PhysicalSize<u32> {
    let scale = scale_factor as f32;
    let width = (content_rect.size.w as f32 * scale).ceil().max(1.0) as u32;
    let height = (content_rect.size.h as f32 * scale).ceil().max(1.0) as u32;
    winit::dpi::PhysicalSize::new(width, height)
}

fn render_terminal_layer_update(
    state: &mut AppState,
    pane_id: PaneId,
    mount: &TerminalMount,
    damage: &TerminalDamage,
    terminal_style: TerminalStyle<'_>,
    window_logical_size: (f32, f32),
    window_physical_size: winit::dpi::PhysicalSize<u32>,
) {
    let logical_size = (
        mount.content_rect.size.w as f32,
        mount.content_rect.size.h as f32,
    );
    let physical_size = retained_terminal_texture_size(mount.content_rect, state.scale_factor);

    state
        .primitive_renderer
        .set_screen_size(&state.queue, logical_size.0, logical_size.1);
    state
        .text_renderer
        .set_screen_size(&state.queue, logical_size.0, logical_size.1);
    state
        .text_renderer
        .set_target_size(physical_size.width, physical_size.height);
    state.text_renderer.set_clip(None);
    state.text_renderer.set_damage(None);

    // Inline images draw in the same logical space as the primitive/text passes.
    // Textures were already uploaded/advanced in `sync_retained_terminal_layers`
    // before the damage decision; here we just queue one quad per placement to
    // flush into the scratch around the glyph pass below.
    state
        .image_renderer
        .set_screen_size(&state.queue, logical_size.0, logical_size.1);
    for placement in &mount.snapshot.graphics {
        state.image_renderer.queue_placement(
            placement,
            0.0,
            0.0,
            mount.snapshot.cell_w,
            mount.snapshot.cell_h,
        );
    }

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("terminal_layer_update"),
        });
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("terminal_layer_clear_scratch"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: state.terminal_layer_scratch.view(),
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    let local_rect = Rectangle::new(
        Point::new(0.0, 0.0),
        Size::new(logical_size.0 as f64, logical_size.1 as f64),
    );
    {
        let mut terminal_renderer =
            TerminalRenderer::new(&mut state.text_renderer, &mut state.primitive_renderer);
        terminal_renderer.render_snapshot_damage(
            &mount.snapshot,
            rect_to_text_box(local_rect),
            terminal_style,
            damage,
        );
    }
    state.primitive_renderer.render(
        &state.device,
        state.terminal_layer_scratch.view(),
        &mut encoder,
    );
    // Under-text images (z < 0) sit above cell backgrounds but below the glyphs.
    state.image_renderer.render(
        &state.device,
        state.terminal_layer_scratch.view(),
        &mut encoder,
        ImageLayer::UnderText,
    );
    state.text_renderer.render(
        &state.queue,
        state.terminal_layer_scratch.view(),
        &mut encoder,
        None,
    );
    // Over-text images (z >= 0, the default) sit above the glyphs.
    state.image_renderer.render(
        &state.device,
        state.terminal_layer_scratch.view(),
        &mut encoder,
        ImageLayer::OverText,
    );

    let copy_bands = terminal_damage_copy_bands(
        damage,
        &mount.snapshot,
        logical_size.1,
        state.scale_factor,
        physical_size.height,
    );
    let layer_texture = state
        .terminal_layers
        .get(&pane_id)
        .expect("terminal layer must exist before updating")
        .texture();
    for band in copy_bands {
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: state.terminal_layer_scratch.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: band.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: layer_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: band.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: physical_size.width,
                height: band.height,
                depth_or_array_layers: 1,
            },
        );
    }

    state.queue.submit(std::iter::once(encoder.finish()));
    state.primitive_renderer.set_screen_size(
        &state.queue,
        window_logical_size.0,
        window_logical_size.1,
    );
    state
        .text_renderer
        .set_screen_size(&state.queue, window_logical_size.0, window_logical_size.1);
    state
        .text_renderer
        .set_target_size(window_physical_size.width, window_physical_size.height);
}

fn terminal_damage_copy_bands(
    damage: &TerminalDamage,
    snapshot: &TerminalSnapshot,
    logical_height: f32,
    scale_factor: f64,
    texture_height: u32,
) -> Vec<TerminalCopyBand> {
    match damage {
        TerminalDamage::None => Vec::new(),
        TerminalDamage::Full => vec![TerminalCopyBand {
            y: 0,
            height: texture_height.max(1),
        }],
        TerminalDamage::Rows(ranges) => {
            let scale = scale_factor as f32;
            let max_height = logical_height.max(0.0);
            let mut bands = Vec::with_capacity(ranges.len());
            for range in ranges {
                let start = range.start.min(snapshot.rows);
                let end = range.end.min(snapshot.rows);
                if start >= end {
                    continue;
                }
                let top = ((start as f32 * snapshot.cell_h) * scale).floor() as u32;
                let bottom =
                    (((end as f32 * snapshot.cell_h).min(max_height)) * scale).ceil() as u32;
                let y = top.min(texture_height);
                let clipped_bottom = bottom.min(texture_height);
                if clipped_bottom > y {
                    bands.push(TerminalCopyBand {
                        y,
                        height: clipped_bottom - y,
                    });
                }
            }
            bands
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TerminalPaneShell {
    pub(crate) pane_id: PaneId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    pub(crate) border_color: [f32; 4],
    pub(crate) border_width: f32,
    pub(crate) border_radius: f32,
}

/// Approx `Tag` internal vertical padding (each side) — for bar height/centering.
const BAR_TAG_VPAD: f32 = 5.0;
/// Vertical margin above + below the bar within its reserved strip.
const BAR_VMARGIN: f32 = 5.0;
/// Monospace line-height ratio (matches `heca-grid-ui`'s `MONO_LINE_RATIO`).
const BAR_LINE_RATIO: f32 = 1.4;

/// The info bar's content height for the given `font`.
fn title_bar_height(font: f32) -> f32 {
    font * BAR_LINE_RATIO + 2.0 * BAR_TAG_VPAD
}

/// Total vertical strip the info bar reserves at the pane top.
pub(crate) fn title_bar_reserve(font: f32) -> f32 {
    title_bar_height(font) + 2.0 * BAR_VMARGIN
}

/// The info bar font — the chrome theme's base font, the same size the sidebar
/// tree lays out with (`paint_chrome_root`), so the two read identically.
fn bar_font(state: &AppState) -> f32 {
    crate::chrome::chrome_gui_theme(state).font_size
}

/// Whether the pane info bar renders anything right now — segments **or** action
/// buttons. Drives both the reserved top strip and the header band/paint.
pub(crate) fn pane_info_bar_shown(state: &AppState) -> bool {
    state.appearance.pane_info_bar_visible()
}

/// Extra **top** content padding (logical px) reserved for the pane info bar, so
/// terminal content starts below it. Zero when the bar is hidden. Shared by the
/// render path and the mouse→cell mapping so the rendered grid and pointer
/// hit-testing use identical geometry.
pub(crate) fn pane_title_top_inset(state: &AppState) -> f32 {
    if pane_info_bar_shown(state) {
        title_bar_reserve(bar_font(state))
    } else {
        0.0
    }
}

pub(crate) fn stable_tiled_content_rect(
    px: f32,
    py: f32,
    pw: f32,
    ph: f32,
    content_inset: f32,
    extra_top: f32,
) -> Option<Rectangle> {
    pane_content_rect(px, py, pw, ph, content_inset, extra_top).map(|(x, y, w, h)| {
        Rectangle::new(
            Point::new(x as f64, y as f64),
            Size::new(w as f64, h as f64),
        )
    })
}

pub(crate) fn stable_floating_content_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    content_inset: f32,
    extra_top: f32,
) -> Option<Rectangle> {
    pane_content_rect(x, y, w, h, content_inset, extra_top).map(|(cx, cy, cw, ch)| {
        Rectangle::new(
            Point::new(cx as f64, cy as f64),
            Size::new(cw as f64, ch as f64),
        )
    })
}

pub(crate) fn pane_scissor_rect(
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

pub(crate) fn paint_terminal_pane_shell(
    state: &AppState,
    scene: &mut GuiScene,
    shell: TerminalPaneShell,
) {
    let TerminalPaneShell {
        pane_id,
        x,
        y,
        w,
        h,
        border_color,
        border_width,
        border_radius,
    } = shell;
    let theme = terminal_pane_gui_theme(state, border_color, border_width, border_radius);
    // The frame is the pane's **retained** shell (`chrome::sync_panes`), not a tree built here and
    // thrown away: the picker writes a letter into it when it opens and reads it back a keystroke
    // later, so a tree that does not outlive the frame cannot carry one. That is why the pane
    // letters used to be stamped by a host paint pass in `render.rs` (F011/P094/T451).
    let retained_pane = state.panes.get(&pane_id);

    let show_bar = pane_info_bar_shown(state);
    let bar_theme = crate::chrome::chrome_gui_theme(state);
    let font = bar_theme.font_size;

    {
        let mut cx = PaintCx::new(scene, &theme);
        // Distinguishable header band behind the title, drawn *before* the frame so
        // the rounded border traces over it. Theme-driven from the dedicated
        // `top_bottom_pane_background` token rather than the generic sidebar/card
        // surface.
        if show_bar {
            cx.rect(
                GuiRectangle::new(
                    GuiPoint::new(x as f64, y as f64),
                    GuiSize::new(w as f64, title_bar_reserve(font) as f64),
                ),
                to_gui_color(
                    state
                        .theme
                        .effective_top_bottom_pane_background()
                        .to_f32x4(),
                ),
                None,
                border_radius,
                None,
            );
        }
        // **Painted through `paint_child`, never `paint`** — `paint_child` is what draws a
        // widget's hint letter after painting it. A direct `.paint(cx)` here is exactly why a pane
        // could never show its own letter.
        if let Some(retained) = retained_pane {
            heca_grid_ui::paint_child(&retained.root, &mut cx);
        }
    }

    // Pane info-bar header (segments + action buttons): painted from the retained
    // per-pane tree that `chrome::sync_pane_headers` built + positioned earlier this
    // frame (before the GPU borrow). Clipped to the pane so the bar/buttons can't
    // spill into a neighbor when the pane is narrow (e.g. after a resize). The clip
    // is a *base-layer* scissor; a button's hover Tooltip draws on the **overlay**
    // layer (rendered after the base PopClip in `render_chrome`), so it still escapes
    // the pane and sits above neighbors. Interactivity is routed in `mouse.rs`.
    if show_bar && let Some(header) = state.pane_headers.get(&pane_id) {
        let clip = GuiRectangle::new(
            GuiPoint::new(x as f64, y as f64),
            GuiSize::new(w as f64, h as f64),
        );
        let mut cx = PaintCx::new(scene, &bar_theme);
        // Through `paint_child` for the same reason the pane is: a header button carrying a hint
        // letter cannot draw one when it is painted directly.
        cx.with_clip(clip, |cx| heca_grid_ui::paint_child(&header.root, cx));
    }

    if let Some(viewport) = state.pane_viewport_widgets.get(&pane_id) {
        let clip = GuiRectangle::new(
            GuiPoint::new(x as f64, y as f64),
            GuiSize::new(w as f64, h as f64),
        );
        let mut cx = PaintCx::new(scene, &bar_theme);
        cx.with_clip(clip, |cx| {
            viewport.badge.paint(cx);
            viewport.scrollbar.paint(cx);
        });
    }
}

pub(crate) struct TerminalRenderPassContext<'a> {
    pub(crate) text_renderer: &'a mut TextRenderer,
    pub(crate) primitive_renderer: &'a mut PrimitiveRenderer,
    pub(crate) device: &'a wgpu::Device,
    pub(crate) queue: &'a wgpu::Queue,
    pub(crate) view: &'a wgpu::TextureView,
    pub(crate) encoder: &'a mut wgpu::CommandEncoder,
    pub(crate) scale_factor: f64,
    pub(crate) surface_physical_size: winit::dpi::PhysicalSize<u32>,
    pub(crate) content_clip: Rectangle,
    /// Rounded content-clip mask. When `Some`, terminal surface/cells/selection/cursor
    /// + glyphs test against it so content follows the pane's rounded border.
    pub(crate) stencil: Option<&'a wgpu::TextureView>,
}

pub(crate) fn render_terminal_mount(
    render_ctx: TerminalRenderPassContext<'_>,
    terminal_style: TerminalStyle<'_>,
    mount: TerminalMount,
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
        stencil,
    } = render_ctx;
    let TerminalMount {
        content_rect,
        snapshot,
        damage: _damage,
    } = mount;
    let content_box = rect_to_text_box(content_rect);
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
        terminal_renderer.render_snapshot(&snapshot, content_box, terminal_style);
        if let Some(ref overlay) = selection_overlay {
            terminal_renderer.render_selection_overlay(
                overlay,
                content_box,
                snapshot.cell_w,
                snapshot.cell_h,
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
    primitive_renderer.render_clipped(device, view, encoder, clip_rect, stencil);
    text_renderer.render(queue, view, encoder, stencil);
    {
        let mut terminal_renderer = TerminalRenderer::new(text_renderer, primitive_renderer);
        terminal_renderer.render_cursor_overlay(&snapshot, content_box);
    }
    primitive_renderer.render_clipped(device, view, encoder, clip_rect, stencil);
}

pub(crate) fn selection_overlay_for_pane(
    state: &AppState,
    pane_id: PaneId,
    snapshot: &TerminalSnapshot,
) -> Option<SelectionOverlay> {
    build_selection_overlay(&state.selection, pane_id, snapshot, &state.theme.accent)
}

fn pane_content_rect(
    px: f32,
    py: f32,
    pw: f32,
    ph: f32,
    inset: f32,
    extra_top: f32,
) -> Option<(f32, f32, f32, f32)> {
    let content_w = (pw - inset * 2.0).max(0.0);
    let content_h = (ph - inset * 2.0 - extra_top).max(0.0);
    if content_w <= 0.0 || content_h <= 0.0 {
        return None;
    }
    Some((px + inset, py + inset + extra_top, content_w, content_h))
}

fn to_gui_color(color: [f32; 4]) -> GuiColor {
    GuiColor::new(
        (color[0].clamp(0.0, 1.0) * 255.0) as u8,
        (color[1].clamp(0.0, 1.0) * 255.0) as u8,
        (color[2].clamp(0.0, 1.0) * 255.0) as u8,
        (color[3].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

fn terminal_pane_gui_theme(
    state: &AppState,
    border_color: [f32; 4],
    border_width: f32,
    border_radius: f32,
) -> GuiTheme {
    let mut theme = crate::chrome::chrome_gui_theme(state);
    theme.colors.accent = to_gui_color(border_color);
    theme.colors.border = to_gui_color(border_color);
    theme.colors.border_radius = border_radius;
    theme.colors.border_width = border_width;
    // The title's `Cut` style matches its surroundings against `theme.background`;
    // for a pane that means the real app/window background sitting behind it (the
    // reserved title strip shows the window backdrop, not the chrome grey).
    theme.colors.background = to_gui_color(state.theme.background.to_f32x4());
    theme
}

fn rect_to_text_box(rect: Rectangle) -> TextBox {
    TextBox {
        x: rect.loc.x as f32,
        y: rect.loc.y as f32,
        w: rect.size.w as f32,
        h: rect.size.h as f32,
    }
}

// `pub(super)` so the selection-overlay unit tests (in `app::render`) can reach it
// after #118 moved this fn out of `render.rs`.
pub(super) fn build_selection_overlay(
    selection: &SelectionState,
    pane_id: PaneId,
    snapshot: &TerminalSnapshot,
    accent: &Color,
) -> Option<SelectionOverlay> {
    if snapshot.cols == 0 || snapshot.rows == 0 {
        return None;
    }

    let stable_to_visible = |stable_row: isize| -> Option<usize> {
        let visible = stable_row - snapshot.viewport_top_stable_row;
        usize::try_from(visible)
            .ok()
            .filter(|row| *row < snapshot.rows)
    };

    if let SelectionState::Caret {
        owner,
        stable_row,
        col,
        ..
    } = selection
    {
        if *owner != SelectionOwner::Pane(pane_id) {
            return None;
        }
        let row = stable_to_visible(*stable_row)?;
        let color = [
            accent.r as f32 / 255.0,
            accent.g as f32 / 255.0,
            accent.b as f32 / 255.0,
            0.25,
        ];
        return Some(
            SelectionOverlay::new(vec![], color).with_caret(CaretIndicator {
                row,
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
            anchor_stable_row,
            anchor_col,
            focus_stable_row,
            focus_col,
        } => {
            let color = [
                accent.r as f32 / 255.0,
                accent.g as f32 / 255.0,
                accent.b as f32 / 255.0,
                0.25,
            ];
            let last_col = snapshot.cols.saturating_sub(1);
            let start_stable = (*anchor_stable_row).min(*focus_stable_row);
            let end_stable = (*anchor_stable_row).max(*focus_stable_row);
            let start_col = if anchor_stable_row < focus_stable_row {
                *anchor_col
            } else if focus_stable_row < anchor_stable_row {
                *focus_col
            } else {
                (*anchor_col).min(*focus_col)
            };
            let end_col = if focus_stable_row > anchor_stable_row {
                *focus_col
            } else if anchor_stable_row > focus_stable_row {
                *anchor_col
            } else {
                (*anchor_col).max(*focus_col)
            };

            let visible_start = snapshot.viewport_top_stable_row.max(start_stable);
            let visible_end = (snapshot.viewport_top_stable_row + snapshot.rows as isize - 1)
                .min(end_stable);
            let mut spans = Vec::new();
            if visible_start <= visible_end {
                for stable_row in visible_start..=visible_end {
                    let row = stable_to_visible(stable_row)
                        .expect("visible stable row must convert to a visible row");
                    let (s, e) = if start_stable == end_stable {
                        (start_col.min(last_col), end_col.min(last_col))
                    } else if stable_row == start_stable {
                        (start_col.min(last_col), last_col)
                    } else if stable_row == end_stable {
                        (0, end_col.min(last_col))
                    } else {
                        (0, last_col)
                    };
                    if s <= e {
                        spans.push(SelectionOverlaySpan {
                            row,
                            start_col: s,
                            end_col: e,
                        });
                    }
                }
            }

            let caret = stable_to_visible(*focus_stable_row).map(|row| CaretIndicator {
                row,
                col: *focus_col,
                is_selection_endpoint: true,
            });
            let overlay = SelectionOverlay::new(spans, color);
            Some(match caret {
                Some(caret) => overlay.with_caret(caret),
                None => overlay,
            })
        }
        SelectionRegion::BackendNative => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TerminalCopyBand, build_selection_overlay, graphics_signature, image_row_ranges,
        merge_row_ranges, ranges_overlap, retained_damage_to_apply, retained_terminal_texture_size,
        terminal_damage_copy_bands, terminal_layer_render_key,
    };
    use crate::app::selection_model::{
        SelectionOwner, SelectionRegion, SelectionSource, SelectionState,
    };
    use heca_config::theme::Color;
    use heca_core::backend::{TerminalDamage, TerminalRowRange, TerminalSnapshot};
    use heca_core::layout::PaneId;
    use heca_renderer::terminal::{TerminalFontFamilies, TerminalStyle};

    fn snapshot(rows: usize, cell_h: f32) -> TerminalSnapshot {
        TerminalSnapshot {
            cols: 80,
            rows,
            cell_w: 8.0,
            cell_h,
            default_fg: [1.0; 4],
            default_bg: [0.0, 0.0, 0.0, 1.0],
            cursor_color: [1.0; 4],
            cursor: heca_core::backend::TerminalCursor {
                col: 0,
                row: 0,
                visible: true,
                shape: heca_core::backend::TerminalCursorShape::Block,
            },
            lines: Vec::new(),
            viewport_offset: 0,
            at_bottom: true,
            scrollback_rows: rows,
            viewport_top_stable_row: 0,
            hyperlinks: Vec::new(),
            graphics: Vec::new(),
            images: Vec::new(),
        }
    }

    fn selection_snapshot(cols: usize, rows: usize, top_stable: isize) -> TerminalSnapshot {
        TerminalSnapshot {
            cols,
            rows,
            cell_w: 8.0,
            cell_h: 12.0,
            default_fg: [1.0; 4],
            default_bg: [0.0, 0.0, 0.0, 1.0],
            cursor_color: [1.0; 4],
            cursor: heca_core::backend::TerminalCursor {
                col: 0,
                row: 0,
                visible: true,
                shape: heca_core::backend::TerminalCursorShape::Block,
            },
            lines: Vec::new(),
            viewport_offset: 0,
            at_bottom: true,
            scrollback_rows: rows,
            viewport_top_stable_row: top_stable,
            hyperlinks: Vec::new(),
            graphics: Vec::new(),
            images: Vec::new(),
        }
    }

    #[test]
    fn build_selection_overlay_returns_none_when_inactive() {
        let selection = SelectionState::new();
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let snap = selection_snapshot(20, 10, 0);
        assert!(build_selection_overlay(&selection, PaneId(1), &snap, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_returns_none_when_owner_mismatch() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(7)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_stable_row: 2,
                anchor_col: 3,
                focus_stable_row: 2,
                focus_col: 3,
        },
        );
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let snap = selection_snapshot(20, 10, 0);
        // Query with a different pane id -> None
        assert!(build_selection_overlay(&selection, PaneId(1), &snap, &accent).is_none());
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
        let snap = selection_snapshot(20, 10, 0);
        assert!(build_selection_overlay(&selection, PaneId(1), &snap, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_returns_overlay_for_host_grid_on_owning_pane() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_stable_row: 2,
                anchor_col: 3,
                focus_stable_row: 2,
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
        let snap = selection_snapshot(20, 10, 0);
        let overlay = build_selection_overlay(&selection, PaneId(1), &snap, &accent);
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
        assert_eq!(
            overlay.color,
            [100.0 / 255.0, 150.0 / 255.0, 200.0 / 255.0, 0.25]
        );
    }

    #[test]
    fn build_selection_overlay_handles_reverse_multiline_selection() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_stable_row: 8,
                anchor_col: 12,
                focus_stable_row: 8,
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
        let snap = selection_snapshot(20, 10, 0);
        let overlay = build_selection_overlay(&selection, PaneId(1), &snap, &accent).unwrap();
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
        let snap = selection_snapshot(20, 10, 0);
        let overlay = build_selection_overlay(&selection, PaneId(1), &snap, &accent).unwrap();
        // Caret-only state: no selection spans.
        assert!(overlay.spans.is_empty());
        // But we get a caret indicator at the caret position.
        let caret = overlay
            .caret
            .expect("caret should be present in caret-only state");
        assert_eq!((caret.row, caret.col), (3, 7));
        assert!(
            !caret.is_selection_endpoint,
            "caret-only should not be a selection endpoint"
        );
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
        let snap = selection_snapshot(20, 10, 0);
        assert!(build_selection_overlay(&selection, PaneId(1), &snap, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_active_selection_has_focus_caret() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::KeyboardMode,
            SelectionRegion::HostGrid {
                anchor_stable_row: 2,
                anchor_col: 0,
                focus_stable_row: 4,
                focus_col: 5,
        },
        );
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let snap = selection_snapshot(20, 10, 0);
        let overlay = build_selection_overlay(&selection, PaneId(1), &snap, &accent).unwrap();
        // Active selection: should have both selection spans and a focus-end caret.
        assert!(!overlay.spans.is_empty());
        let caret = overlay
            .caret
            .expect("focus caret should be present for active selection");
        assert_eq!((caret.row, caret.col), (4, 5));
        assert!(
            caret.is_selection_endpoint,
            "active selection caret should be a selection endpoint"
        );
    }

    #[test]
    fn terminal_damage_copy_bands_full_covers_entire_texture() {
        let bands =
            terminal_damage_copy_bands(&TerminalDamage::Full, &snapshot(3, 12.0), 36.0, 2.0, 72);
        assert_eq!(bands, vec![TerminalCopyBand { y: 0, height: 72 }]);
    }

    #[test]
    fn terminal_damage_copy_bands_rows_convert_row_ranges_to_pixel_bands() {
        let damage = TerminalDamage::Rows(vec![
            heca_core::backend::TerminalRowRange::new(1, 3),
            heca_core::backend::TerminalRowRange::new(4, 5),
        ]);
        let bands = terminal_damage_copy_bands(&damage, &snapshot(5, 10.0), 50.0, 2.0, 100);
        assert_eq!(
            bands,
            vec![
                TerminalCopyBand { y: 20, height: 40 },
                TerminalCopyBand { y: 80, height: 20 },
            ]
        );
    }

    #[test]
    fn terminal_damage_copy_bands_clamps_rows_to_visible_height() {
        let damage = TerminalDamage::Rows(vec![TerminalRowRange::new(2, 8)]);
        let bands = terminal_damage_copy_bands(&damage, &snapshot(4, 12.0), 48.0, 1.0, 48);
        assert_eq!(bands, vec![TerminalCopyBand { y: 24, height: 24 }]);
    }

    // --- `terminal-00c` app-path retained-presentation coverage -----------------
    //
    // The retained-content foundation (`terminal-00b`) keeps the last frame's
    // content in a per-pane layer and only re-renders dirty rows. These tests
    // pin the pure policy that protects unchanged rows: `retained_damage_to_apply`
    // decides skip / Full / passthrough, `retained_terminal_texture_size` sizes
    // the offscreen scratch, and `terminal_layer_render_key` detects style changes.

    fn style(font_size: f32, surface_alpha: f32) -> TerminalStyle<'static> {
        TerminalStyle {
            font_size,
            families: TerminalFontFamilies {
                normal: "Maple Mono Normal NF",
                bold: None,
                italic: None,
                bold_italic: None,
            },
            surface_alpha,
            ligatures: true,
            hyperlink_style: heca_renderer::terminal::HyperlinkDecor::Underline,
            hyperlink_color: [0.4, 0.6, 1.0, 1.0],
        }
    }

    fn rng(start: usize, end: usize) -> TerminalRowRange {
        TerminalRowRange::new(start, end)
    }

    #[test]
    fn retained_damage_skips_when_backend_reports_none_and_no_structural_change() {
        // No resize, no style change, backend reports nothing dirty, no images:
        // the retained layer already holds the previous frame, so it is skipped.
        assert_eq!(
            retained_damage_to_apply(false, false, false, &TerminalDamage::None, &[], &[]),
            None
        );
    }

    #[test]
    fn retained_damage_passes_dirty_rows_through_when_stable() {
        // Stable layer + backend row damage, no images: only the reported rows.
        let rows = TerminalDamage::Rows(vec![rng(1, 3)]);
        assert_eq!(
            retained_damage_to_apply(false, false, false, &rows, &[], &[]),
            Some(TerminalDamage::Rows(vec![rng(1, 3)]))
        );
    }

    #[test]
    fn retained_damage_passes_full_through_when_stable() {
        assert_eq!(
            retained_damage_to_apply(false, false, false, &TerminalDamage::Full, &[], &[]),
            Some(TerminalDamage::Full)
        );
    }

    #[test]
    fn retained_damage_upgrades_to_full_on_resize_even_if_backend_reports_none() {
        assert_eq!(
            retained_damage_to_apply(true, false, false, &TerminalDamage::None, &[], &[]),
            Some(TerminalDamage::Full)
        );
    }

    #[test]
    fn retained_damage_upgrades_to_full_on_style_change() {
        assert_eq!(
            retained_damage_to_apply(false, true, false, &TerminalDamage::None, &[], &[]),
            Some(TerminalDamage::Full)
        );
        assert_eq!(
            retained_damage_to_apply(
                false,
                true,
                false,
                &TerminalDamage::Rows(vec![rng(0, 2)]),
                &[],
                &[]
            ),
            Some(TerminalDamage::Full)
        );
    }

    #[test]
    fn retained_damage_resize_dominates_style_and_backend_damage() {
        assert_eq!(
            retained_damage_to_apply(
                true,
                true,
                false,
                &TerminalDamage::Rows(vec![rng(0, 1)]),
                &[],
                &[]
            ),
            Some(TerminalDamage::Full)
        );
    }

    #[test]
    fn retained_damage_idle_image_pane_still_skips() {
        // A static image with no text damage and unchanged placements costs nothing.
        assert_eq!(
            retained_damage_to_apply(
                false,
                false,
                false,
                &TerminalDamage::None,
                &[rng(5, 7)],
                &[rng(5, 7)]
            ),
            None
        );
    }

    #[test]
    fn retained_damage_image_change_damages_only_image_rows() {
        // Image appeared (graphics_changed) with no text damage: repaint just the
        // image rows (terminal-task-23), not the whole pane.
        assert_eq!(
            retained_damage_to_apply(
                false,
                false,
                true,
                &TerminalDamage::None,
                &[rng(3, 5)],
                &[]
            ),
            Some(TerminalDamage::Rows(vec![rng(3, 5)]))
        );
    }

    #[test]
    fn retained_damage_image_move_repaints_old_and_new_rows() {
        // Image moved: union of old and new rows so no stale pixels remain.
        assert_eq!(
            retained_damage_to_apply(
                false,
                false,
                true,
                &TerminalDamage::None,
                &[rng(6, 8)],
                &[rng(3, 5)]
            ),
            Some(TerminalDamage::Rows(vec![rng(3, 5), rng(6, 8)]))
        );
    }

    #[test]
    fn retained_damage_image_clear_repaints_old_rows() {
        // Image cleared (now empty, was present): repaint the old rows to erase it.
        assert_eq!(
            retained_damage_to_apply(false, false, true, &TerminalDamage::None, &[], &[rng(3, 5)]),
            Some(TerminalDamage::Rows(vec![rng(3, 5)]))
        );
    }

    #[test]
    fn retained_damage_text_over_image_reblits_image_rows() {
        // Text changed on rows that overlap an image: re-blit the image there so
        // the glyph pass doesn't clobber it. Merged into one contiguous range.
        assert_eq!(
            retained_damage_to_apply(
                false,
                false,
                false,
                &TerminalDamage::Rows(vec![rng(4, 6)]),
                &[rng(5, 8)],
                &[rng(5, 8)]
            ),
            Some(TerminalDamage::Rows(vec![rng(4, 8)]))
        );
    }

    #[test]
    fn retained_damage_text_away_from_image_leaves_image_alone() {
        // Text changed far from a static image: only the text rows are repainted;
        // the image stays retained (not re-blitted).
        assert_eq!(
            retained_damage_to_apply(
                false,
                false,
                false,
                &TerminalDamage::Rows(vec![rng(1, 2)]),
                &[rng(5, 7)],
                &[rng(5, 7)]
            ),
            Some(TerminalDamage::Rows(vec![rng(1, 2)]))
        );
    }

    #[test]
    fn image_row_ranges_clamps_and_merges() {
        use heca_core::backend::GraphicsPlacement;
        let placement = |row, rows| GraphicsPlacement {
            row,
            col: 0,
            cols: 2,
            rows,
            image_id: 1,
            src_top_left: [0.0, 0.0],
            src_bottom_right: [1.0, 1.0],
            z_index: 0,
        };
        // Two placements (rows 1..3 and 2..4) merge to 1..4; a third past the grid
        // clamps to `rows`.
        let g = vec![placement(1, 2), placement(2, 2), placement(9, 5)];
        assert_eq!(image_row_ranges(&g, 10), vec![rng(1, 4), rng(9, 10)]);
    }

    #[test]
    fn merge_row_ranges_coalesces_adjacent_and_overlapping() {
        assert_eq!(
            merge_row_ranges(vec![rng(3, 5), rng(1, 2), rng(2, 3), rng(5, 6)]),
            vec![rng(1, 6)]
        );
        assert!(ranges_overlap(&[rng(4, 6)], &[rng(5, 8)]));
        assert!(!ranges_overlap(&[rng(1, 2)], &[rng(5, 8)]));
    }

    #[test]
    fn graphics_signature_changes_with_placements() {
        use heca_core::backend::GraphicsPlacement;
        let base = GraphicsPlacement {
            row: 0,
            col: 0,
            cols: 2,
            rows: 1,
            image_id: 7,
            src_top_left: [0.0, 0.0],
            src_bottom_right: [1.0, 1.0],
            z_index: 0,
        };
        let empty = graphics_signature(&[]);
        let one = graphics_signature(std::slice::from_ref(&base));
        assert_ne!(empty, one, "presence of an image changes the signature");
        assert_eq!(one, graphics_signature(std::slice::from_ref(&base)), "stable");

        let mut moved = base.clone();
        moved.col = 4;
        assert_ne!(one, graphics_signature(&[moved]), "moving the image changes it");

        let mut other_image = base.clone();
        other_image.image_id = 8;
        assert_ne!(one, graphics_signature(&[other_image]), "new image id changes it");
    }

    #[test]
    fn retained_terminal_texture_size_scales_and_rounds_up() {
        use heca_core::layout::{Point, Rectangle, Size};
        let rect = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 40.0));
        // scale 2.0 → 200x80, ceiled.
        let sz = retained_terminal_texture_size(rect, 2.0);
        assert_eq!((sz.width, sz.height), (200, 80));
        // Fractional physical pixels round up (.ceil) so partial rows aren't lost.
        let sz = retained_terminal_texture_size(
            Rectangle::new(Point::new(0.0, 0.0), Size::new(10.5, 5.25)),
            2.0,
        );
        assert_eq!((sz.width, sz.height), (21, 11));
    }

    #[test]
    fn retained_terminal_texture_size_never_zero_for_positive_rect() {
        use heca_core::layout::{Point, Rectangle, Size};
        // A sub-pixel pane still yields at least 1x1 so the texture is valid.
        let sz = retained_terminal_texture_size(
            Rectangle::new(Point::new(0.0, 0.0), Size::new(0.1, 0.1)),
            1.0,
        );
        assert_eq!((sz.width, sz.height), (1, 1));
    }

    #[test]
    fn terminal_layer_render_key_is_stable_for_identical_style() {
        assert_eq!(terminal_layer_render_key(&style(14.0, 0.8)), terminal_layer_render_key(&style(14.0, 0.8)));
    }

    #[test]
    fn terminal_layer_render_key_changes_with_font_size() {
        assert_ne!(terminal_layer_render_key(&style(14.0, 0.8)), terminal_layer_render_key(&style(15.0, 0.8)));
    }

    #[test]
    fn terminal_layer_render_key_changes_with_surface_alpha() {
        assert_ne!(terminal_layer_render_key(&style(14.0, 0.8)), terminal_layer_render_key(&style(14.0, 0.6)));
    }

    #[test]
    fn terminal_layer_render_key_changes_with_font_family() {
        let a = TerminalStyle {
            font_size: 14.0,
            families: TerminalFontFamilies {
                normal: "Mono A",
                bold: None,
                italic: None,
                bold_italic: None,
            },
            surface_alpha: 0.8,
            ligatures: true,
            hyperlink_style: heca_renderer::terminal::HyperlinkDecor::Underline,
            hyperlink_color: [0.4, 0.6, 1.0, 1.0],
        };
        let b = TerminalStyle {
            font_size: 14.0,
            families: TerminalFontFamilies {
                normal: "Mono B",
                bold: None,
                italic: None,
                bold_italic: None,
            },
            surface_alpha: 0.8,
            ligatures: true,
            hyperlink_style: heca_renderer::terminal::HyperlinkDecor::Underline,
            hyperlink_color: [0.4, 0.6, 1.0, 1.0],
        };
        assert_ne!(terminal_layer_render_key(&a), terminal_layer_render_key(&b));
    }

    #[test]
    fn terminal_layer_render_key_changes_with_ligatures() {
        // Toggling ligatures must change the render key so the retained terminal
        // layer re-renders (the `terminal_ligatures` setting applies live).
        let on = style(14.0, 0.8);
        let mut off = style(14.0, 0.8);
        off.ligatures = false;
        assert_ne!(
            terminal_layer_render_key(&on),
            terminal_layer_render_key(&off)
        );
    }
}
