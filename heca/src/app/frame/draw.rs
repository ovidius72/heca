//! **The passes of a frame**, each a method of [`Frame`] named for what it draws.

use heca_core::layout::Rectangle;

use super::host_work::{HostTargets, do_host_work};
use super::{Frame, FrameTerminals};
use crate::app::scene_flush::flush_scene;
use crate::app::terminal_render::TerminalRenderState;
use crate::app_state::AppState;
use crate::mouse;

impl Frame {
    /// **Clear the scene, then lay the bottom layers**: the frosted z=0 background and the top bar.
    pub(in crate::app) fn clear_and_background(&mut self, state: &mut AppState) {
        let bg = state.theme.background.to_linear_f32x4();
        // Transparent window: clear fully transparent so empty/background areas show the frosted
        // vibrancy at full strength. Chrome panels draw translucent (chrome_alpha) on top; opaque
        // panes/text/borders stay crisp. transparent == false reproduces today's opaque clear
        // exactly.
        let (clear_r, clear_g, clear_b, clear_a) = if state.appearance.is_transparent() {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            (bg[0] as f64, bg[1] as f64, bg[2] as f64, bg[3] as f64)
        };
        self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.scene,
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
        // Bottom-most layer: a blurred vertical gradient composited at `background_alpha()` (opaque
        // by default per the locked decision). Tiled panes then render translucent
        // (`surface_alpha`) directly over this — their frost IS z=0 showing through, not a
        // per-pane tint. Drawn pre-stencil (`stencil = None`) so the tiled content-clip never
        // clips the background. `BackgroundLayer` caches the blurred result; it recomputes only on
        // resize or param change (see `heca-renderer/src/background.rs`).
        let top = state
            .appearance
            .effective_background_gradient_top(&state.theme)
            .to_linear_f32x4();
        let bottom = state
            .appearance
            .effective_background_gradient_bottom(&state.theme)
            .to_linear_f32x4();
        let blur_radius = state.appearance.background_blur_radius() * self.v.scale;
        state.background.set_params(top, bottom, blur_radius);
        // Order invariant: BackgroundLayer snapshots the blurred gradient into its own cache inside
        // `render()`, so the shared `state.blur` is free to be reused afterwards by the
        // floating-pane frost pass. The z=0 layer MUST be rendered BEFORE any other `state.blur`
        // user this frame — if a later blur user runs first, the z=0 cache would capture that
        // user's output instead of the gradient. (See `heca-renderer/src/background.rs`.)
        let bg_view =
            state
                .background
                .render(&state.device, &state.queue, &mut self.encoder, &state.blur);
        let vp_w = self.v.phys_size.width as f32;
        let vp_h = self.v.phys_size.height as f32;
        state.backdrop.draw(
            &state.device,
            &state.queue,
            &mut self.encoder,
            &self.scene,
            bg_view,
            (vp_w, vp_h),
            (0.0, 0.0, vp_w, vp_h),
            Some([0.0, 0.0, 1.0, 1.0]),
            state.appearance.background_alpha(),
            0.0,
            None,
        );

        // Frosted chrome colors come from the loaded theme's surface tone, with alpha derived from
        // the current appearance settings.
        let (side_bg, _, _) = crate::chrome::chrome_colors(state);
        state.primitive_renderer.draw_rect(
            0.0,
            0.0,
            self.v.w,
            self.v.chrome.tab_bar_height,
            side_bg.to_f32x4(),
        );
        state
            .primitive_renderer
            .render(&state.device, &self.scene, &mut self.encoder);
        state
            .text_renderer
            .render(&state.queue, &self.scene, &mut self.encoder, None);
    }

    /// **Write the rounded content-clip mask for `panes`.**
    ///
    /// Marks each pane's rounded rect (the full pane, radius = the pane border radius) in the
    /// stencil buffer so the content passes (backdrop + text + primitives) test against it and fill
    /// to the pane edge, following the rounded border. The border is drawn OUTSIDE this rect, so
    /// content meets the border's inner edge with no gap (outer-border style). One pass writes the
    /// union of masks; each pane's content is separately scissored to its own rect, so the union
    /// mask clips it to its own rounded shape. The grid renderer appends this at its running frame
    /// offset (composing with the later border/color passes).
    ///
    /// Clear-once contract: `render_stencil` clears the stencil to 0 then writes the mask; the
    /// content passes `Load` it (never clear). Called once a frame, for the tiled and the floating
    /// panes together.
    pub(in crate::app) fn write_mask<'a>(
        &mut self,
        state: &mut AppState,
        panes: impl Iterator<Item = &'a TerminalRenderState>,
    ) {
        let mut any = false;
        for pane in panes {
            any = true;
            state.grid_renderer.draw(heca_renderer::grid::GlowRect {
                x: pane.x,
                y: pane.y,
                w: pane.w,
                h: pane.h,
                fill: [0.0; 4],
                border: [0.0; 4],
                border_width: 0.0,
                radius: self.v.pane_border_radius,
                glow: [0.0; 4],
                glow_radius: 0.0,
                glow_intensity: 0.0,
                glow_alpha_scale: 0.0,
                shadow: [0.0; 4],
                shadow_radius: 0.0,
                shadow_offset: [0.0, 0.0],
            });
        }
        if !any {
            return;
        }
        state
            .grid_renderer
            .render_stencil(&state.queue, &self.stencil, &mut self.encoder);
    }

    /// **Flush the window's scene**, once, in the order it was painted.
    ///
    /// The workspace (the columns, then the floats), the chrome and every surface are children of
    /// one root, painted by one walk into one scene, so what is later in the tree is on top. A
    /// terminal is a surface in that scene and is drawn when the flush reaches it; a frost blurs
    /// what the runs before it drew. Nothing here orders panes against chrome.
    pub(in crate::app) fn flush_window(
        &mut self,
        state: &mut AppState,
        scene: &heca_grid_ui::Scene,
        terminals: &FrameTerminals,
    ) {
        let pass = self.pass();
        let targets = HostTargets::new(&self.scene, &self.stencil, &self.v);
        flush_scene(
            state,
            scene,
            &pass,
            &self.scene,
            &mut self.encoder,
            &mut self.flushed,
            &mut |state, work, encoder| do_host_work(state, terminals, &targets, work, encoder),
        );
        // The pane being moved with the mouse and the hint of where it would land are drawn
        // straight to the screen, over the window.
        let pane_area_rect = Rectangle::new(
            heca_core::layout::Point::new(self.v.pane_area.loc.x, self.v.pane_area.loc.y),
            heca_core::layout::Size::new(self.v.pane_area.size.w, self.v.pane_area.size.h),
        );
        mouse::render_detached_pane(state, pane_area_rect);
        mouse::render_insert_hint(state, pane_area_rect);
        state
            .primitive_renderer
            .render(&state.device, &self.scene, &mut self.encoder);
        state
            .text_renderer
            .render(&state.queue, &self.scene, &mut self.encoder, None);
    }
}
