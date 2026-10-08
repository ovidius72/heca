//! **What the host does between two runs of the window's scene**: draw a terminal where the scene
//! put it, or blur what the runs before it drew.
//!
//! The scene decides where and when; this only performs it. A terminal is told apart by the list it
//! is in — tiled, floating or docked — because each is masked and scissored differently.

use heca_core::layout::Rectangle;

use super::FrameTerminals;
use crate::app::terminal_render::{TerminalTarget, draw_surface, pane_scissor_rect};
use crate::app_state::AppState;

/// What the work is drawn into, and how each kind of terminal is masked.
pub(super) struct HostTargets<'a> {
    pub(super) view: &'a wgpu::TextureView,
    pub(super) stencil: &'a wgpu::TextureView,
    /// The content area's scissor, which a tiled terminal is cut to.
    pub(super) tiled_scissor: Option<(u32, u32, u32, u32)>,
    pub(super) surface_alpha: f32,
    pub(super) floating_surface_alpha: f32,
    pub(super) content_clip: Rectangle,
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
            tiled_scissor: v.content_scissor,
            surface_alpha: v.surface_alpha,
            floating_surface_alpha: v.floating_surface_alpha,
            content_clip: v.pane_area,
            phys_size: v.phys_size,
        }
    }

    /// A tiled terminal: rounded to its pane by the mask, cut to the content area.
    fn tiled(&self) -> TerminalTarget<'a> {
        TerminalTarget {
            view: self.view,
            stencil: Some(self.stencil),
            scissor: self.tiled_scissor,
            surface_alpha: self.surface_alpha,
            content_clip: self.content_clip,
            follow_scene_clip: false,
        }
    }

    /// A floating terminal: rounded to its pane by the mask, cut to its own pane.
    fn floating(&self, state: &AppState, pane: (f32, f32, f32, f32)) -> TerminalTarget<'a> {
        TerminalTarget {
            view: self.view,
            stencil: Some(self.stencil),
            scissor: pane_scissor_rect(
                pane.0,
                pane.1,
                pane.2,
                pane.3,
                state.scale_factor,
                self.phys_size,
            ),
            surface_alpha: self.floating_surface_alpha,
            content_clip: self.content_clip,
            follow_scene_clip: false,
        }
    }

    /// A terminal no pane owns: no mask, clipped by what clips it in its scene.
    fn docked(&self) -> TerminalTarget<'a> {
        TerminalTarget {
            view: self.view,
            stencil: None,
            scissor: None,
            surface_alpha: self.surface_alpha,
            content_clip: self.content_clip,
            follow_scene_clip: true,
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
            let id = at.id;
            let find = |list: &'_ [crate::app::terminal_render::TerminalRenderState]| {
                list.iter().position(|t| t.id.0 == id)
            };
            if let Some(i) = find(&terminals.tiled) {
                let target = targets.tiled();
                draw_surface(state, &terminals.tiled[i..=i], at, &target, encoder);
            } else if let Some(i) = find(&terminals.floating) {
                let t = &terminals.floating[i];
                let target = targets.floating(state, (t.x, t.y, t.w, t.h));
                draw_surface(state, &terminals.floating[i..=i], at, &target, encoder);
            } else if let Some(i) = find(&terminals.docked) {
                let target = targets.docked();
                draw_surface(state, &terminals.docked[i..=i], at, &target, encoder);
            }
        }
        heca_grid_ui::HostWork::Backdrop(at) => blur_backdrop(state, targets, at, encoder),
    }
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
