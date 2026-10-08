//! Keeping every terminal's retained texture up to date.

use super::damage::{
    graphics_signature, image_row_ranges, retained_damage_to_apply, terminal_damage_copy_bands,
};
use crate::app::terminal_host::TerminalMount;
use crate::app::terminal_render::{TerminalRenderState, rect_to_text_box};
use crate::app_state::AppState;
use crate::chrome::terminal::TerminalId;
use heca_core::backend::TerminalDamage;
use heca_core::layout::{Point, Rectangle, Size};
use heca_renderer::image::ImageLayer;
use heca_renderer::terminal::{TerminalRenderer, TerminalStyle};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

pub(crate) fn sync_retained_terminal_layers(
    state: &mut AppState,
    panes: &[TerminalRenderState],
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
            state.terminal_layers.remove(&pane.id);
            continue;
        };

        // Per-pane font zoom: start from the shared style and override the font
        // size for this pane. The render key already folds in `font_size`, so a
        // zoom change on one pane repaints only that pane's retained layer.
        let mut pane_style = terminal_style;
        pane_style.font_size = state.terminal_font_size(pane.pane);
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

        let layer = state.terminal_layers.entry(pane.id).or_insert_with(|| {
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
            pane.id,
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
    state
        .image_renderer
        .evict_unused(IMAGE_TEXTURE_MAX_AGE_FRAMES);
}

/// Frames an unreferenced inline-image texture survives before eviction (~10s at
/// 60fps). The retained layer still shows any on-screen image; this only bounds
/// GPU memory for images no longer being re-rendered.
const IMAGE_TEXTURE_MAX_AGE_FRAMES: u64 = 600;

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
    let backends = &state.backends;
    state
        .terminal_layers
        .retain(|id, _| backends.is_issued(*id));
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
    terminal: TerminalId,
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
        .get(&terminal)
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

#[cfg(test)]
mod tests;
