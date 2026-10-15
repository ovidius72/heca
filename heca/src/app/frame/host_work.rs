//! **What the host does between two runs of the window's scene**: draw a terminal where the scene
//! put it, or blur what the runs before it drew.
//!
//! The scene decides where, when and what cuts it; this only performs it. Every terminal is drawn
//! by the same rule — in its scene box, cut by the clip in force there and by the round corners of
//! what holds it — so a terminal in a column, a float, a dock, an overlay or a picture carried
//! under the pointer is drawn right without this knowing which.

use super::FrameTerminals;
use crate::app::terminal_render::{TerminalTarget, draw_surface};
use crate::app_state::AppState;

/// What the work is drawn into.
pub(super) struct HostTargets<'a> {
    pub(super) view: &'a wgpu::TextureView,
    pub(super) stencil: &'a wgpu::TextureView,
    pub(super) phys_size: winit::dpi::PhysicalSize<u32>,
}

impl<'a> HostTargets<'a> {
    /// Targets for this frame's values.
    pub(super) fn new(
        view: &'a wgpu::TextureView,
        stencil: &'a wgpu::TextureView,
        v: &super::FrameValues,
    ) -> Self {
        Self {
            view,
            stencil,
            phys_size: v.phys_size,
        }
    }
}

/// Do `work`, which the flush has just reached.
pub(super) fn do_host_work(
    state: &mut AppState,
    terminals: &FrameTerminals,
    targets: &HostTargets<'_>,
    work: &heca_grid_ui::HostWork,
    encoder: &mut wgpu::CommandEncoder,
) {
    match work {
        heca_grid_ui::HostWork::Surface(at) => {
            let stencil = at.rounded.map(|rounded| {
                cut_round_corners(state, targets.stencil, rounded, encoder);
                targets.stencil
            });
            let target = TerminalTarget {
                view: targets.view,
                stencil,
            };
            draw_surface(state, &terminals.0, at, &target, encoder);
        }
        heca_grid_ui::HostWork::Backdrop(at) => blur_backdrop(state, targets, at, encoder),
    }
}

/// **Write `rounded` into the stencil**, alone, so the terminal drawn next is cut to those corners
/// and to nothing else's.
fn cut_round_corners(
    state: &mut AppState,
    stencil: &wgpu::TextureView,
    rounded: heca_grid_ui::scene::Rounded,
    encoder: &mut wgpu::CommandEncoder,
) {
    state.grid_renderer.draw(heca_renderer::grid::GlowRect {
        x: rounded.rect.loc.x as f32,
        y: rounded.rect.loc.y as f32,
        w: rounded.rect.size.w as f32,
        h: rounded.rect.size.h as f32,
        fill: [0.0; 4],
        border: [0.0; 4],
        border_width: 0.0,
        radius: rounded.radius,
        glow: [0.0; 4],
        glow_radius: 0.0,
        glow_intensity: 0.0,
        glow_alpha_scale: 0.0,
        shadow: [0.0; 4],
        shadow_radius: 0.0,
        shadow_offset: [0.0, 0.0],
    });
    state
        .grid_renderer
        .render_stencil(&state.queue, stencil, encoder);
}

/// **Blur what the scene has drawn so far behind `at`, and stamp it back** within its box, cut to
/// the clip in force where it was recorded. Called in scene order, so it blurs exactly what lies
/// under the surface that asked for it and nothing drawn over it.
fn blur_backdrop(
    state: &mut AppState,
    targets: &HostTargets<'_>,
    at: &heca_grid_ui::BackdropAt,
    encoder: &mut wgpu::CommandEncoder,
) {
    let sf = state.scale_factor as f32;
    let vp_w = targets.phys_size.width as f32;
    let vp_h = targets.phys_size.height as f32;
    // Logical → physical, then intersect with the clip in force where it was recorded.
    let (mut x, mut y, mut bw, mut bh) = (
        at.rect.loc.x as f32 * sf,
        at.rect.loc.y as f32 * sf,
        at.rect.size.w as f32 * sf,
        at.rect.size.h as f32 * sf,
    );
    if let Some(c) = at.clip {
        let x1 = (x + bw).min((c.loc.x + c.size.w) as f32 * sf);
        let y1 = (y + bh).min((c.loc.y + c.size.h) as f32 * sf);
        x = x.max(c.loc.x as f32 * sf);
        y = y.max(c.loc.y as f32 * sf);
        bw = (x1 - x).max(0.0);
        bh = (y1 - y).max(0.0);
    }
    if at.radius <= 0.0 || at.alpha <= 0.0 || bw <= 0.0 || bh <= 0.0 {
        return;
    }
    let blurred = state.blur.process(
        &state.device,
        &state.queue,
        encoder,
        targets.view,
        at.radius * sf,
    );
    state.backdrop.draw(
        &state.device,
        &state.queue,
        encoder,
        targets.view,
        blurred,
        (vp_w, vp_h),
        // `None` src-uv samples the same screen location as the destination, so the blur is of
        // exactly what sits behind the rect.
        (x, y, bw, bh),
        None,
        // The frost fades with the layer that asked for it. Left at full strength it would hold
        // the whole session out of focus for the length of the fade and then snap back sharp in
        // one frame — the exact pop the fade exists to remove.
        at.alpha,
        at.corner * sf,
        None,
    );
}
