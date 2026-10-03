//! Putting a retained terminal texture on screen, and the cursor and selection over it.

use crate::app::terminal_host::TerminalMount;
use crate::app::terminal_render::{TerminalTarget, rect_to_text_box};
use crate::app_state::AppState;
use crate::chrome::terminal::TerminalId;
use heca_core::layout::Rectangle;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::terminal::{SelectionOverlay, TerminalRenderer};
use heca_renderer::text::TextRenderer;

pub(crate) fn blit_retained_terminal_layer(
    state: &AppState,
    terminal: TerminalId,
    encoder: &mut wgpu::CommandEncoder,
    target: &TerminalTarget<'_>,
    viewport_px: (f32, f32),
    mount: &TerminalMount,
    clip: Option<Rectangle>,
) -> bool {
    let Some(layer) = state.terminal_layers.get(&terminal) else {
        return false;
    };
    let rect = mount.content_rect;
    // The part of the terminal that shows: all of it, or what its clip leaves. The texture is
    // cropped to match, so what shows is the same picture, not a squeezed one.
    let shown = match clip {
        Some(clip) => rect.intersection(clip),
        None => Some(rect),
    };
    let Some(shown) = shown else {
        return true;
    };
    let uv = [
        ((shown.loc.x - rect.loc.x) / rect.size.w) as f32,
        ((shown.loc.y - rect.loc.y) / rect.size.h) as f32,
        ((shown.loc.x + shown.size.w - rect.loc.x) / rect.size.w) as f32,
        ((shown.loc.y + shown.size.h - rect.loc.y) / rect.size.h) as f32,
    ];
    let scale = state.scale_factor as f32;
    state.backdrop.draw(
        &state.device,
        &state.queue,
        encoder,
        target.view,
        layer.view(),
        viewport_px,
        (
            shown.loc.x as f32 * scale,
            shown.loc.y as f32 * scale,
            shown.size.w as f32 * scale,
            shown.size.h as f32 * scale,
        ),
        Some(uv),
        1.0,
        target.stencil,
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
