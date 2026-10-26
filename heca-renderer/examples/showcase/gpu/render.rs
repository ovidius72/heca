use super::*;

impl GpuState {
    pub(crate) fn render(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        // Tick BOTH trees (bitwise `|` so neither is short-circuited away).
        let animating = self.ui.tick(dt) | self.overlays.tick(dt);

        // Zoom-aware scale: a bigger effective scale shrinks the logical viewport, so
        // the same widgets render larger (a global zoom). Feed it to both renderers
        // for crisp rasterization at the zoomed size.
        let phys = self.window.inner_size();
        let eff = self.effective_scale();
        self.grid.set_scale_factor(eff);
        self.text.set_scale_factor(eff);
        let scale = eff as f32;
        let (w, h) = (phys.width as f32 / scale, phys.height as f32 / scale);

        self.grid.set_screen_size(&self.queue, w, h);
        self.text.set_screen_size(&self.queue, w, h);

        // Theme switching: when the theme selection changes, load the new theme
        // and rebuild the showcase tree so every widget picks up the new palette.
        let mut restore_scroll = None;
        let theme_idx = self.ctl.theme_idx.get_untracked();
        if theme_idx != self.current_theme_idx {
            restore_scroll = Some((self.ui.scroll_offset_x().get_untracked(),
                self.ui.scroll_offset().get_untracked()));
            self.current_theme_idx = theme_idx;
            self.theme = load_grid_theme(THEME_NAMES[theme_idx]);
            // Reset control signals to match the new theme's baked-in values.
            self.ctl.glow.set(self.theme.colors.glow_size);
            self.ctl.radius.set(self.theme.colors.border_radius);
            self.ctl.border.set(self.theme.colors.border_width);
            self.ctl.font.set(self.theme.font_size);
            self.ctl.intensity.set(self.theme.colors.intensity);
            self.ctl.overlay_frame.set(self.theme.colors.overlay_frame);

            let built = build_ui(&self.theme, self.ctl);
            let BuiltUi {
                ui,
                overlays,
                sidebar_mode,
                rail_hints,
                rail_states,
                rail_letters,
                attention_req,
                palette_open,
                menu_open,
                menu_anchor,
                toasts,
            } = built;
            self.ui = ui;
            self.overlays = overlays;
            self.sidebar_mode = sidebar_mode;
            self.rail_hints = rail_hints;
            self.rail_states = rail_states;
            self.rail_letters = rail_letters;
            self.attention_req = attention_req;
            self.palette_open = palette_open;
            self.menu_open = menu_open;
            self.menu_anchor = menu_anchor;
            self.toasts = toasts;
            self.focus.clear(&mut self.ui);
            self.focus_ov.clear(&mut self.overlays);
            self.layout_dirty = true;
            self.force_full = true;
            self.window
                .set_title(&format!("{} — {}", APP_TITLE, self.theme.colors.name));
        }

        // Fold live theme controls in (paint-only except font, which reflows layout).
        self.theme.colors.glow_size = self.ctl.glow.get_untracked();
        self.theme.colors.border_radius = self.ctl.radius.get_untracked();
        self.theme.colors.border_width = self.ctl.border.get_untracked();
        self.theme.colors.intensity = self.ctl.intensity.get_untracked();
        self.theme.colors.overlay_frame = self.ctl.overlay_frame.get_untracked();
        let font = self.ctl.font.get_untracked();
        if (font - self.theme.font_size).abs() > f32::EPSILON {
            self.theme.font_size = font;
            self.layout_dirty = true;
        }
        // Global SIZE control: push the chosen variant onto every widget + reflow.
        let size = self.ctl.size.get_untracked();
        if size != self.applied_size {
            apply_size(&mut self.ui, size);
            apply_size(&mut self.overlays, size);
            self.applied_size = size;
            self.layout_dirty = true;
        }
        // Global ZOOM control: the effective scale changed the logical viewport, so
        // relayout against the new (w, h).
        let zoom = self.ctl.zoom.get_untracked();
        if (zoom - self.applied_zoom).abs() > f32::EPSILON {
            self.applied_zoom = zoom;
            self.layout_dirty = true;
        }

        // Recompute layout ONLY when an input changed it (resize / font / content
        // event) — never on pure-animation frames. The full per-frame taffy
        // relayout was the bulk of the render() CPU.
        // **A widget that changed the shape of its own tree asks for a layout pass.** Damage is
        // about pixels; this is about position — a stacked toast that finished leaving is gone
        // from the tree, and the cards below it are still laid out around the hole. Asked BEFORE
        // the layout block so the pass runs on this frame, not the next input event.
        if heca_grid_ui::needs_layout(&self.ui) | heca_grid_ui::needs_layout(&self.overlays) {
            self.layout_dirty = true;
        }

        if self.layout_dirty {
            // The root ScrollRegion IS the window; the page inside it lays out at
            // natural size and the region scrolls/clips it (the scroll offset
            // survives relayout; the region re-clamps + re-applies it in `on_layout`).
            layout_page(&mut self.ui, &self.theme, Size::new(w as f64, h as f64), restore_scroll);
            // The overlay layer lays out at the viewport size, independent of the
            // page scroll — a Dialog taffy-centers on the real window (BUG B).
            self.overlays.base_mut().style.layout.width = Length::Px(w);
            self.overlays.base_mut().style.layout.height = Length::Px(h);
            LayoutEngine::new()
                .theme(&self.theme)
                .compute(&mut self.overlays, Size::new(w as f64, h as f64));
            self.layout_dirty = false;
            self.force_full = true; // relayout moves everything → repaint in full
        }

        let scene = build_scene(&self.ui, &self.overlays, &self.theme, w, h, self.clip_demo);

        // Damage region for this frame: full on an input/layout/scroll change (it can
        // alter unknown regions), otherwise just the widgets that flagged themselves
        // needs-paint (a spinner, the caret) — so an animation repaints only its rect.
        // `collect_damage` also clears the flags. Union damage across BOTH trees.
        let dirty = match (
            heca_grid_ui::collect_damage(&self.ui),
            heca_grid_ui::collect_damage(&self.overlays),
        ) {
            (Some(a), Some(b)) => {
                let x0 = a.loc.x.min(b.loc.x);
                let y0 = a.loc.y.min(b.loc.y);
                let x1 = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
                let y1 = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
                Some(Rectangle::new(
                    Point::new(x0, y0),
                    Size::new(x1 - x0, y1 - y0),
                ))
            }
            (a, None) => a,
            (None, b) => b,
        };
        self.force_full |= self.ui_images.begin_frame();
        let damage: Option<[f32; 4]> = if std::mem::take(&mut self.force_full) {
            None // whole frame
        } else {
            match dirty {
                Some(r) => Some([
                    r.loc.x as f32,
                    r.loc.y as f32,
                    r.size.w as f32,
                    r.size.h as f32,
                ]),
                // Animated but reported no damage (an un-wired continuous animation)
                // → repaint in full for correctness; nothing dirty → empty rect
                // (render nothing; the blit shows the preserved scene texture).
                None if animating => None,
                None => Some([0.0, 0.0, 0.0, 0.0]),
            }
        };
        self.grid.set_damage(damage);
        self.text.set_damage(damage);

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("showcase"),
            });

        // Render the UI into the compositor's persistent scene texture (not directly
        // to the swapchain), then blit it to screen. The persistent texture is what
        // makes damage-region redraw possible — unchanged pixels survive between
        // frames, so a frame can re-render only the damaged region.
        let scene_view = self.compositor.scene_view();

        // Each pass is a rects-then-text flush (the renderer draws all queued rects,
        // then all queued text). Base first, then each overlay as its *own* pass — so
        // overlay content occludes base text (not just base rects), AND a later overlay
        // occludes an earlier one. Flushing every overlay in a single pass would draw
        // all overlay rects then all overlay text, letting a lower overlay's text bleed
        // over a higher overlay's panel (the overlapping-overlay text-bleed bug).
        // Reset the renderers' per-frame buffer offsets so the base + overlay passes
        // append to the persistent buffers (no per-frame staging allocation).
        self.grid.begin_frame();
        self.text.begin_frame();
        let glow_alpha_scale = heca_renderer::scene::glow_alpha_scale_for_background(
            self.theme.colors.background.to_f32x4(),
        );
        for layer in std::iter::once(scene.base_layer()).chain(scene.overlay_segments()) {
            for run in layer.base_runs() {
                enqueue_scene(&mut self.grid, &mut self.text, &run.draws, glow_alpha_scale);
                self.grid.render(&self.queue, scene_view, &mut encoder);
                self.text.render(&self.queue, scene_view, &mut encoder, None);
                if let Some(heca_grid_ui::HostWork::Image(at)) = run.then {
                    self.ui_images.draw(heca_renderer::ui_image::ImageFrame {
                        device: &self.device, queue: &self.queue, encoder: &mut encoder,
                        target: scene_view, stencil: at.rounded.map(|_| self.compositor.stencil_view()),
                        viewport: (self.config.width as f32, self.config.height as f32),
                        scale, damage,
                    }, &at);
                }
            }
        }
        // Blit the composited scene onto the swapchain.
        self.compositor.blit(&view, &mut encoder);
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        // Decide when the next frame is needed: a continuous animation (spinner,
        // slide) runs at the ~30fps cap; otherwise sleep until the soonest timed
        // redraw a widget asks for (e.g. a focused caret's next blink) — or, if
        // nothing is pending, wait for input. This keeps a focused idle Input from
        // pegging a core at the full frame rate just to blink twice a second.
        self.next_frame_in = if animating {
            Some(0.0)
        } else {
            // Soonest timed wake across BOTH trees (e.g. a caret in the page vs. a
            // toast slide in the overlay layer).
            match (self.ui.next_redraw(), self.overlays.next_redraw()) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, None) => a,
                (None, b) => b,
            }
        };
    }
}

fn layout_page(ui: &mut ScrollRegion, theme: &Theme, viewport: Size, restore_scroll: Option<(f32, f32)>) {
    ui.base_mut().style.layout.width = Length::Px(viewport.w as f32);
    ui.base_mut().style.layout.height = Length::Px(viewport.h as f32);
    LayoutEngine::new().theme(theme).compute(ui, viewport);
    // The replacement tree now has measured scroll limits. Restore the reader's position
    // after its initial reveal, using the viewport's own clamping and follow-pause behavior.
    if let Some((x, y)) = restore_scroll {
        ui.scroll_to_x(x);
        ui.scroll_to(y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuilding_the_theme_preserves_the_readers_scroll_position() {
        let mut theme = Theme::default();
        let ctl = ThemeCtl {
            glow: signal(theme.colors.glow_size), radius: signal(theme.colors.border_radius),
            border: signal(theme.colors.border_width), font: signal(theme.font_size),
            intensity: signal(theme.colors.intensity), overlay_frame: signal(theme.colors.overlay_frame),
            size: signal(WidgetSize::Normal), zoom: signal(ZOOM_DEFAULT), theme_idx: signal(0),
        };
        let viewport = Size::new(600.0, 700.0);
        let mut built = build_ui(&theme, ctl);
        layout_page(&mut built.ui, &theme, viewport, None);
        for y in [0.0, 900.0, 2400.0] {
            let position = (built.ui.scroll_to_x(50.0), built.ui.scroll_to(y));
            theme.font_size += 1.0;
            let mut replacement = build_ui(&theme, ctl);
            layout_page(&mut replacement.ui, &theme, viewport, Some(position));
            assert_eq!((replacement.ui.scroll_offset_x().get_untracked(),
                replacement.ui.scroll_offset().get_untracked()), position,
                "theme rebuilding must not jump to a preselected catalog example");
            built = replacement;
        }
    }
}

