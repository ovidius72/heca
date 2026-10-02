//! **Flushing a painted scene to the GPU, in the order the scene says.**
//!
//! A scene is rects and text, plus the host work it asks for in place — a terminal surface is one.
//! [`flush_scene`] draws it the way it reads: flush what is drawn up to a surface, put the surface
//! there, flush what is drawn over it. A terminal inside a pane, a dock or an overlay is therefore
//! covered by exactly what the scene draws after it (a chip, a scrollbar, the pane's own border)
//! and by nothing else — no pass of the host's own decides that.

use heca_renderer::grid::GridRenderer;
use heca_renderer::text::TextRenderer;

use crate::app_state::AppState;

/// What one flush needs besides the scene.
pub(super) struct ChromePassOpts {
    pub(super) damage: Option<heca_grid_ui::Rectangle>,
    pub(super) glow_alpha_scale: f32,
}

/// What draws the terminal surface `id` when the flush reaches it. Takes the state and the encoder
/// because the surface needs both; the flush holds neither across the call.
pub(super) type DrawSurface<'a> = dyn FnMut(&mut AppState, u64, &mut wgpu::CommandEncoder) + 'a;

/// **Flush `scene`'s base layer in scene order**, then hand its overlay segments to `overlay_sink`
/// so they are flushed once, above every surface, by [`flush_overlay_band`].
///
/// A scene with no surface is one flush — the same single pass it always was.
pub(super) fn flush_scene(
    state: &mut AppState,
    scene: &heca_grid_ui::Scene,
    opts: &ChromePassOpts,
    view: &wgpu::TextureView,
    encoder: &mut wgpu::CommandEncoder,
    overlay_sink: &mut Vec<heca_grid_ui::Scene>,
    draw_surface: &mut DrawSurface<'_>,
) {
    for run in scene.base_runs() {
        flush_base(
            &mut state.grid_renderer,
            &mut state.text_renderer,
            &state.queue,
            &run.draws,
            opts,
            view,
            encoder,
        );
        if let Some(surface) = run.then {
            draw_surface(state, surface.id, encoder);
        }
    }
    overlay_sink.extend(scene.overlay_segments());
}

/// One rects-then-text pass over a base run.
///
/// Takes the renderer fields individually (not `&mut AppState`) because the frame holds borrows of
/// the state across it.
fn flush_base(
    grid: &mut GridRenderer,
    text: &mut TextRenderer,
    queue: &wgpu::Queue,
    scene: &heca_grid_ui::Scene,
    opts: &ChromePassOpts,
    view: &wgpu::TextureView,
    encoder: &mut wgpu::CommandEncoder,
) {
    // NOTE: `begin_frame()` is called once at the top of `render_frame`, not here.
    // Calling it per flush reset the persistent vertex-buffer write offset to 0 every pass, so each
    // pass overwrote the previous pass's vertices at buffer offset 0 — and since all passes are
    // submitted in one `queue.submit` at end of frame, every encoded pass read the *last* pass's
    // vertex data. One `begin_frame()` per frame makes each `render()` append at a distinct offset
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
    heca_renderer::scene::enqueue_scene(grid, text, scene, opts.glow_alpha_scale);
    grid.render(queue, view, encoder);
    text.render(queue, view, encoder, None);
}

/// Flush the collected overlay segments from every surface, in accumulation order (panes →
/// floats → chrome, so higher surfaces' overlays sit on top), **above all surface bases**.
/// This is the overlay **top band** of the surface compositor's paint z-order: a tooltip /
/// popover always paints over every pane, float, and the chrome/sidebars, never occluded by a
/// surface that flushed after its own. Full repaint (no damage/clip); the segments already
/// carry their own geometry.
pub(super) fn flush_overlay_band(
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
