//! **One frame, in the order it is drawn.** `render_frame` (in `render.rs`) is the list of phases;
//! each phase lives here as a function or a method of [`Frame`], named for what it does:
//!
//! 1. [`begin`] — what has to be true before anything is drawn, and the values every phase reads;
//! 2. [`paint_scenes`] — the columns and each float painted into scenes, so every terminal's real
//!    position is known; [`collect_panes`] and [`sync_terminal_layers`] bring the terminals up to date;
//! 3. [`Frame::open`] — the surface, the command encoder and the views the frame draws into;
//! 4. [`Frame::clear_and_background`], [`Frame::write_mask`], [`Frame::columns`],
//!    [`Frame::floats`], [`Frame::chrome`], [`Frame::backdrops`], [`Frame::finish`] — the passes.

use std::collections::HashMap;

use heca_core::layout::{PaneId, Rectangle};
use heca_grid_ui::Scene as GuiScene;

use crate::app::scene_flush::{ChromePassOpts, flush_scene};
use crate::app::terminal_render::{
    TerminalRenderState, TerminalTarget, draw_surface, pane_scissor_rect,
};
use crate::app_state::AppState;
use crate::chrome::ChromeConfig;

mod draw;
mod panes;

pub(super) use panes::{collect_panes, paint_scenes, sync_terminal_layers};

/// What every phase of a frame reads, worked out once at the top.
pub(super) struct FrameValues {
    chrome: ChromeConfig,
    /// The window in physical pixels.
    phys_size: winit::dpi::PhysicalSize<u32>,
    scale: f32,
    /// The window in logical pixels.
    w: f32,
    h: f32,
    /// The content area: where the panes live.
    pane_area: Rectangle,
    ws_offset: (f32, f32),
    glow_alpha_scale: f32,
    surface_alpha: f32,
    /// Floating panes use independent opacity/blur/border knobs so they can stay readable (opaque
    /// by default) while tiled panes are frosted.
    floating_surface_alpha: f32,
    pane_border_radius: f32,
    content_scissor: Option<(u32, u32, u32, u32)>,
}

/// **Every terminal a frame draws**, grouped by where it sits.
pub(in crate::app) struct FrameTerminals {
    pub(in crate::app) tiled: Vec<TerminalRenderState>,
    pub(in crate::app) floating: Vec<TerminalRenderState>,
    /// The ones no pane owns: a dock's, an overlay's. Drawn where their scene puts them, with no
    /// pane mask.
    pub(in crate::app) docked: Vec<TerminalRenderState>,
}

impl FrameTerminals {
    pub(in crate::app) fn all(&self) -> impl Iterator<Item = &TerminalRenderState> {
        self.tiled.iter().chain(&self.floating).chain(&self.docked)
    }
}

/// The columns and each float, painted into scenes before anything is measured against them.
pub(super) struct PaneScenes {
    columns: GuiScene,
    floats: HashMap<PaneId, GuiScene>,
}

/// Everything the frame draws into.
pub(super) struct Frame {
    v: FrameValues,
    surface_texture: wgpu::SurfaceTexture,
    view: wgpu::TextureView,
    encoder: wgpu::CommandEncoder,
    /// The persistent scene texture the passes draw into. A handle, not a borrow of the compositor,
    /// so a phase can hand `&mut state` to the flush while it holds the view.
    scene: wgpu::TextureView,
    /// Stencil buffer paired with the scene texture: holds the rounded content-clip mask written
    /// each frame so terminal content follows the pane's rounded border.
    stencil: wgpu::TextureView,
    /// Overlay content (hover tooltips, popovers) from every surface — panes, floats, chrome — is
    /// collected here and flushed once at the very end, above all bases (the surface-compositor top
    /// band).
    overlay_sink: Vec<GuiScene>,
}

/// **Before anything is drawn**: mount what widgets asked to open, bring the retained trees up to
/// date, read the values the phases share, and reset the renderers' per-frame buffers. `None` when
/// nothing asked for a frame.
pub(super) fn begin(state: &mut AppState) -> Option<FrameValues> {
    // Mount whatever a widget asked to open since the last frame: a declared context menu is a
    // layer, and this is the one moment the host has `&mut AppState` and has not yet drawn.
    crate::chrome::drain_pending_menus(state);
    // And the drags that finished: the same shape, drained in the same breath.
    crate::chrome::drain_pending_drops(state);
    if !state.needs_redraw {
        return None;
    }
    state.needs_redraw = false;
    let _ = crate::chrome::sync_chrome_state(state);
    // Build/position the retained per-pane info-bar headers *before* the GPU borrow below, so
    // render can paint them read-only and `mouse.rs` can dispatch pointer events into them.
    // The retained per-pane shells — the frame, the pane's identity and its pick letter. Same
    // moment and same reason as the headers: built before the GPU borrow so render can paint them
    // read-only (F011/P094/T451).
    crate::chrome::sync_panes(state);
    // The terminals extensions declared since the last frame are started now, so the scenes
    // painted next already place them.
    crate::chrome::terminal::start_declared(state);

    let phys_size = state.window.inner_size();
    let scale = state.scale_factor as f32;
    let chrome = ChromeConfig::of(state);
    let pane_area = chrome.content_rect();
    state.text_renderer.begin_frame();
    state.text_renderer.set_damage(None);
    state.text_renderer.set_clip(None);
    // Reset the grid renderer's persistent vertex/index buffer offsets once per frame so each flush
    // appends at a distinct region (see `scene_flush::flush_base`).
    state.grid_renderer.begin_frame();

    // The three pane frame colours are **not** read here. A pane's frame colour is its own —
    // written onto its retained shell by `chrome::sync_panes`, which is also where it becomes the
    // hue the pane publishes to its contents. Reading them here meant the host decided how a widget
    // looked.
    let ws_offset = state
        .session
        .workspace_geometries()
        .first()
        .map(|(_, rect)| (rect.loc.x as f32, rect.loc.y as f32))
        .unwrap_or((0.0, 0.0));
    let content_scissor = pane_scissor_rect(
        pane_area.loc.x as f32,
        pane_area.loc.y as f32,
        pane_area.size.w as f32,
        pane_area.size.h as f32,
        state.scale_factor,
        phys_size,
    );
    Some(FrameValues {
        chrome,
        phys_size,
        scale,
        w: phys_size.width as f32 / scale,
        h: phys_size.height as f32 / scale,
        pane_area,
        ws_offset,
        glow_alpha_scale: heca_renderer::scene::glow_alpha_scale_for_background(
            state.theme.background.to_f32x4(),
        ),
        surface_alpha: state.terminal_surface_opacity(),
        floating_surface_alpha: state.terminal_floating_surface_opacity(),
        pane_border_radius: state.appearance.effective_pane_border_radius(&state.theme),
        content_scissor,
    })
}

impl Frame {
    /// **Open the frame's surface and command encoder.** `None` when the surface is not there to
    /// draw into (lost, outdated): the frame is skipped.
    pub(super) fn open(state: &mut AppState, v: FrameValues) -> Option<Self> {
        let surface_texture = match state.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                state
                    .surface
                    .configure(&state.device, &state.surface_config);
                return None;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => std::process::exit(1),
            Err(_) => return None,
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render"),
            });
        Some(Self {
            v,
            surface_texture,
            view,
            encoder,
            scene: state.compositor.scene_view().clone(),
            stencil: state.compositor.stencil_view().clone(),
            overlay_sink: Vec::new(),
        })
    }

    /// What one flush needs: no damage region, because the scene texture is cleared every frame
    /// (the clear pass) and panes redraw in full. A partial (damage-scissored) repaint would leave
    /// the rest as bare background for that frame — the dark "re-render" flash seen mid-animation
    /// (e.g. the split button's press flash). Partial chrome is only sound with a *preserved*
    /// scene, which this render path does not keep. See PLAN.md "Damage-region render" for the
    /// deferred optimization that would make it sound.
    fn pass(&self) -> ChromePassOpts {
        ChromePassOpts {
            damage: None,
            glow_alpha_scale: self.v.glow_alpha_scale,
        }
    }

    /// **The top band, then present**: every surface's overlay content (tooltips, popovers), above
    /// all bases, flushed segment by segment like any scene — so a terminal inside an overlay is
    /// drawn where its segment puts it — and the scene texture onto the screen.
    pub(super) fn finish(mut self, state: &mut AppState, docked: &[TerminalRenderState]) {
        let pass = self.pass();
        let target = docked_target(&self.scene, &self.v);
        let overlays = std::mem::take(&mut self.overlay_sink);
        for segment in &overlays {
            flush_scene(
                state,
                segment,
                &pass,
                &self.scene,
                &mut self.encoder,
                &mut Vec::new(),
                &mut |state, at, encoder| draw_surface(state, docked, at, &target, encoder),
            );
        }
        state.compositor.blit(&self.view, &mut self.encoder);
        state.queue.submit(std::iter::once(self.encoder.finish()));
        self.surface_texture.present();
    }
}

/// Where a terminal no pane owns is drawn: no mask, and clipped by what clips it in its scene.
fn docked_target<'a>(scene: &'a wgpu::TextureView, v: &FrameValues) -> TerminalTarget<'a> {
    TerminalTarget {
        view: scene,
        stencil: None,
        scissor: None,
        surface_alpha: v.surface_alpha,
        content_clip: v.pane_area,
        follow_scene_clip: true,
    }
}
