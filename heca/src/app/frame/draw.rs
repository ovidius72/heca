//! **The passes of a frame**, each a method of [`Frame`] named for what it draws.

use heca_core::layout::Rectangle;
use heca_grid_ui::Component;
use heca_grid_ui::Scene as GuiScene;

use super::{Frame, PaneScenes};
use crate::app::scene_flush::{ChromePassOpts, flush_scene};
use crate::app::terminal_render::{
    TerminalRenderState, TerminalTarget, draw_surface, pane_scissor_rect,
};
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
    /// content passes `Load` it (never clear). Called once for the tiled panes and once for the
    /// floating ones (after the tiled mask has been consumed), each guarded by there being a pane.
    pub(in crate::app) fn write_mask(
        &mut self,
        state: &mut AppState,
        panes: &[TerminalRenderState],
    ) {
        if panes.is_empty() {
            return;
        }
        for pane in panes {
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
        state
            .grid_renderer
            .render_stencil(&state.queue, &self.stencil, &mut self.encoder);
    }

    /// **The columns, with each terminal drawn where the scene puts it.**
    ///
    /// A terminal is a surface in the scene its pane paints, so it is drawn when the flush reaches
    /// it: what the scene drew before it is under it, what it draws after — the chip, the
    /// scrollbar, the pane's own border — is over it. Nothing here orders terminals against chrome.
    pub(in crate::app) fn columns(
        &mut self,
        state: &mut AppState,
        scenes: &PaneScenes,
        tiled: &[TerminalRenderState],
    ) {
        if tiled.is_empty() {
            return;
        }
        let target = TerminalTarget {
            view: &self.scene,
            stencil: Some(&self.stencil),
            scissor: self.v.content_scissor,
            surface_alpha: self.v.surface_alpha,
            content_clip: self.v.pane_area,
            follow_scene_clip: false,
        };
        flush_scene(
            state,
            &scenes.columns,
            &ChromePassOpts {
                damage: None,
                glow_alpha_scale: self.v.glow_alpha_scale,
            },
            &self.scene,
            &mut self.encoder,
            &mut self.overlay_sink,
            &mut |state, at, encoder| draw_surface(state, tiled, at, &target, encoder),
        );
    }

    /// **The floating panes**, each over what is beneath it.
    ///
    /// Floating panes frost the actual tiled content behind them (the scene already holds tiled
    /// panes + z=0), so a real blur is captured once per frame, after the columns, and stamped
    /// behind each float. Each float then gets its own backdrop (the frost, or a solid
    /// `float_background`) clipped to the floating mask, and its own scene in order: its terminal
    /// where the scene puts it, then what is drawn over it.
    pub(in crate::app) fn floats(
        &mut self,
        state: &mut AppState,
        scenes: &mut PaneScenes,
        floating: &[TerminalRenderState],
    ) {
        let needs_frost = self.v.floating_surface_alpha < 1.0
            && state.appearance.terminal_floating_blur_radius() > 0.0;
        // A handle to the blurred texture, not a borrow of `state.blur`: the loop below hands
        // `&mut state` to the flush while it still needs the blur.
        let blurred: Option<wgpu::TextureView> = needs_frost.then(|| {
            let radius_physical =
                state.appearance.terminal_floating_blur_radius() * state.scale_factor as f32;
            state
                .blur
                .process(
                    &state.device,
                    &state.queue,
                    &mut self.encoder,
                    &self.scene,
                    radius_physical,
                )
                .clone()
        });
        // A second mask: the tiled one has been consumed by the columns. Each float's backdrop +
        // terminal are scissored to its own rect, so the union mask clips it to its own rounded
        // shape — matching the tiled clip, with no corner overflow and a filled (not transparent)
        // margin.
        self.write_mask(state, floating);
        let float_background = state.theme.float_background.to_f32x4();
        for pane in floating {
            if pane.content_rect.is_none() {
                continue;
            }
            let scissor = pane_scissor_rect(
                pane.x,
                pane.y,
                pane.w,
                pane.h,
                state.scale_factor,
                self.v.phys_size,
            );
            if let Some(blurred) = &blurred {
                let scale = state.scale_factor as f32;
                let vp_w = self.v.phys_size.width as f32;
                let vp_h = self.v.phys_size.height as f32;
                state.backdrop.draw(
                    &state.device,
                    &state.queue,
                    &mut self.encoder,
                    &self.scene,
                    blurred,
                    (vp_w, vp_h),
                    (
                        pane.x * scale,
                        pane.y * scale,
                        pane.w * scale,
                        pane.h * scale,
                    ),
                    None,
                    1.0,
                    Some(&self.stencil),
                );
            } else {
                // Solid floating backdrop: fill the full pane with the theme `float_background`
                // (theme-driven floating-window frame color) so the pane is opaque/readable and the
                // inner padding margin is filled, not transparent. Rounded-clipped via the
                // floating stencil.
                state.primitive_renderer.draw_rect(
                    pane.x,
                    pane.y,
                    pane.w,
                    pane.h,
                    float_background,
                );
                state.primitive_renderer.render_clipped(
                    &state.device,
                    &self.scene,
                    &mut self.encoder,
                    scissor,
                    Some(&self.stencil),
                );
            }
            let Some(frame) = pane.pane.and_then(|id| scenes.floats.remove(&id)) else {
                continue;
            };
            let target = TerminalTarget {
                view: &self.scene,
                stencil: Some(&self.stencil),
                scissor,
                surface_alpha: self.v.floating_surface_alpha,
                content_clip: self.v.pane_area,
                follow_scene_clip: false,
            };
            flush_scene(
                state,
                &frame,
                &ChromePassOpts {
                    damage: None,
                    glow_alpha_scale: self.v.glow_alpha_scale,
                },
                &self.scene,
                &mut self.encoder,
                &mut self.overlay_sink,
                &mut |state, at, encoder| draw_surface(state, floating, at, &target, encoder),
            );
        }
    }

    /// **The chrome, painted last** so the sidebar shells sit on top of the pane content instead of
    /// panes bleeding under them. Returns the scene it flushed, for [`backdrops`](Self::backdrops).
    pub(in crate::app) fn chrome(
        &mut self,
        state: &mut AppState,
        docked: &[TerminalRenderState],
    ) -> GuiScene {
        let pane_area_rect = Rectangle::new(
            heca_core::layout::Point::new(self.v.pane_area.loc.x, self.v.pane_area.loc.y),
            heca_core::layout::Size::new(self.v.pane_area.size.w, self.v.pane_area.size.h),
        );
        // The left sidebar is drawn by the grid-ui chrome scene (`build_chrome_scene`) when
        // Expanded; when Hidden its width is 0 and nothing is drawn. There is no collapsed icon
        // rail (dropped — see `docs/sidebar-provider-modes.md`).
        mouse::render_detached_pane(state, pane_area_rect);
        mouse::render_insert_hint(state, pane_area_rect);
        state
            .primitive_renderer
            .render(&state.device, &self.scene, &mut self.encoder);
        state
            .text_renderer
            .render(&state.queue, &self.scene, &mut self.encoder, None);

        // F4.1 — retained tree: rebuild the widget tree only when the chrome signature changes;
        // otherwise re-layout + paint the kept tree (no per-frame signal churn, and a live tree to
        // dispatch events into in F4.2).
        let chrome_sig = crate::chrome::chrome_signature(state, self.v.chrome);
        if state.chrome_tree.as_ref().map(|t| t.sig) != Some(chrome_sig) {
            let (chrome_root, signals, drag_items, intent_source) =
                crate::chrome::build_chrome_root(state, self.v.chrome);
            // **Seat the chrome subtree, keep the window root.** Every surface hangs beside the
            // chrome rather than inside it, so a rebuild — a resize, a sidebar toggle, a theme
            // reload — leaves them untouched.
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
        // Unconditional, because "did anything change" is no longer a question one return value
        // can answer once rows subscribe for themselves. A tick with no time and nothing pending is
        // a walk that finds nothing.
        state.window_root.tick(0.0);
        let (w, h) = (self.v.w, self.v.h);
        let theme = crate::chrome::chrome_gui_theme(state);
        let mut scene = crate::chrome::paint_chrome_root(&mut state.window_root, w, h, &theme);
        // **A drag draws itself.** The insertion line, the swap outline and the picture of the
        // thing under the pointer are painted by the widgets the drag passes through, inside
        // `paint_child` — so nothing opts in, and a plugin's own row gets the same feedback
        // (F003/P097/T496). The host pass that drew this for the left sidebar alone is gone.
        // Follow-link keycaps (prefix+Shift+o) over the focused terminal's hyperlinks, painted
        // into the chrome scene so they sit above pane content. terminal-task-18.
        crate::chrome::paint_link_hints(state, &mut scene, w, h, &theme);
        // Visual-bell flash over the content area (fades out). terminal-task-17.
        crate::chrome::paint_bell_flash(state, &mut scene, self.v.pane_area, w, h, &theme);

        // The trees just built may have declared terminals: one more frame starts and draws them.
        if crate::chrome::terminal::declared_waiting() {
            state.mark_full_redraw();
        }
        // A terminal in a dock is drawn where this scene puts it: no pane mask, clipped by what
        // clips it here (a scroll region, a panel).
        let target = super::docked_target(&self.scene, &self.v);
        let pass = self.pass();
        flush_scene(
            state,
            &scene,
            &pass,
            &self.scene,
            &mut self.encoder,
            &mut self.overlay_sink,
            &mut |state, at, encoder| draw_surface(state, docked, at, &target, encoder),
        );
        scene
    }

    /// **Perform the host work the chrome scene recorded** (`docs/surface-compositor.md` § 0.5).
    ///
    /// Every surface is a child of the window root, so the chrome walk already painted them — one
    /// tree, one paint (§ 0.8). What is left is the GPU work the walk recorded.
    ///
    /// The host asks nothing about layers here and knows no surface by name. A node that wants its
    /// backdrop blurred records the request while it paints — `Overlay::frosted` is the one that
    /// does today — and this performs it: blur the scene texture, stamp it back. Between the base
    /// flush and the overlay flush is exactly "after everything beneath the surface, before the
    /// surface", which is what a backdrop means and the reason the request goes in the base band.
    ///
    /// **The picker's keycaps are NOT painted here, and must never be.** Each is drawn by the
    /// widget that declared the pick, in that widget's own paint (`heca_grid_ui::offer_hint`). A
    /// host cannot know which half of which scene a widget it has never seen paints into, so it
    /// must not try (F003/P082/T416, T427).
    pub(in crate::app) fn backdrops(&mut self, state: &mut AppState, scene: &GuiScene) {
        for req in heca_renderer::scene::host_requests(scene) {
            let heca_grid_ui::scene::HostDraw::Backdrop { radius } = req.draw else {
                continue;
            };
            let sf = state.scale_factor as f32;
            let vp_w = self.v.phys_size.width as f32;
            let vp_h = self.v.phys_size.height as f32;
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
                &mut self.encoder,
                &self.scene,
                radius * sf,
            );
            state.backdrop.draw(
                &state.device,
                &state.queue,
                &mut self.encoder,
                &self.scene,
                blurred,
                (vp_w, vp_h),
                // `None` src-uv samples the same screen location as the destination, so the blur
                // is of exactly what sits behind the rect.
                (x, y, bw, bh),
                None,
                // The frost fades with the layer that asked for it. Left at full strength it would
                // hold the whole session out of focus for the length of the fade and then snap
                // back sharp in one frame — the exact pop the fade exists to remove.
                req.alpha,
                None,
            );
        }
    }
}
