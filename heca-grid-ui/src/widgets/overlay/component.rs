//! `Overlay` as a [`Component`]: how an open surface paints, takes its place, advances its arrival,
//! and answers the pointer and the keyboard.

use super::*;

impl Component for Overlay {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Focusable only while open, so a host's overlay scan can route input here
    /// when the `Overlay` is mounted directly (a composing widget like `Dialog`
    /// is found first in pre-order and intercepts instead).
    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// Layout just reset the panel to its taffy-computed position; in
    /// [`Anchored`](OverlayPosition::Anchored) mode, re-place it against the
    /// trigger rect (idempotent — see [`place_panel`](Overlay::place_panel)).
    /// [`Center`](OverlayPosition::Center) mode keeps taffy's centering untouched.
    fn on_layout(&mut self) {
        self.place_panel();
    }

    /// **A layer is not scrolled into view.** `Base::focused` says this widget holds the keyboard
    /// while it is open, and `wants_visible` defaults to exactly that — so an enclosing
    /// `ScrollRegion` would scroll the page to wherever this widget's layout node happens to sit,
    /// every frame it is open. A layer draws over the page; the page does not come to it.
    fn wants_visible(&self) -> bool {
        false
    }

    /// This surface's arrival and exit — what a host drives, and what it carries across a rebuild.
    fn show(&mut self) {
        Overlay::show(self);
    }

    fn set_default_focus(&mut self, name: &str) {
        self.default_focus = Some(name.to_string());
    }

    fn follow_open(&mut self, open: Signal<bool>) {
        self.base.open = open;
        self.base.follow_focus_modal(open);
        self.base
            .presence
            .assume_open(crate::reactive::SignalGet::get_untracked(&open));
    }

    fn advance_focus(&mut self, forward: bool) {
        if let Some(panel) = self.base.children.first_mut() {
            self.focus.advance(panel.as_mut(), forward);
        }
    }

    fn focus_first_quiet(&mut self) {
        if let Some(panel) = self.base.children.first_mut() {
            self.focus.focus_first_quiet(panel.as_mut());
        }
    }

    fn close(&mut self) {
        Overlay::close(self);
    }

    /// Advance the arrival or exit, then the subtree.
    ///
    /// Runs [`settle`](Overlay::settle) because a **composing** widget drives the same surface
    /// through [`open_signal`](Overlay::open_signal) — one flip of that signal is an arrival or a
    /// dismissal exactly as a host's call is, and neither may be the only way an animation starts,
    /// or the only way the keyboard is placed.
    ///
    /// (It named `Component::set_open` before, which does not exist and never has.)
    fn tick(&mut self, dt: f32) -> bool {
        self.settle();
        let mut animating = self.base.tick_presence(dt);
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    /// The overlay's **input** surface: nothing at all while closed (the panel is still in the
    /// tree and still laid out, and must be completely inert), the whole viewport while
    /// **blocking** (the scrim owns every point), and just the panel otherwise, so a press beside a
    /// non-blocking layer reaches the page behind it.
    fn hit_bounds(&self) -> Option<Rectangle> {
        if !self.is_open() {
            return None;
        }
        if self.blocking {
            let vp = self.viewport.get();
            return Some(if vp.w.is_finite() {
                Rectangle::new(Point::new(0.0, 0.0), vp)
            } else {
                Rectangle::new(
                    Point::new(-SCRIM_REACH, -SCRIM_REACH),
                    Size::new(2.0 * SCRIM_REACH, 2.0 * SCRIM_REACH),
                )
            });
        }
        Some(self.panel_bounds())
    }

    /// A **blocking** overlay occludes the whole viewport (its scrim owns every
    /// point); a non-blocking one occludes only the panel itself.
    fn overlay_occludes(&self, pos: Point) -> bool {
        self.is_open() && (self.blocking || self.panel_bounds().contains(pos))
    }

    /// Paints while it is **present** — open, or dismissed and still leaving — and paints
    /// everything it draws under its [`Animation`]'s frame.
    ///
    /// One place, for the whole surface: the scrim, the panel chrome and every descendant go
    /// through the same [`AnimationFrame::apply`](crate::animation::AnimationFrame::apply), so no widget inside knows an animation is
    /// running and a new animation needs no drawing code anywhere. A frame is something done *to*
    /// a picture — it never re-measures, never moves bounds, and therefore never changes what is
    /// clickable.
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() || !self.is_present() {
            return;
        }
        self.viewport.set(cx.viewport());
        let (background, scrim_a, frost_radius) = {
            let t = cx.theme();
            (
                t.colors.background,
                t.colors.interaction.scrim,
                t.colors.overlay_frost_radius,
            )
        };
        let panel = self.panel_bounds();

        let frame = self.base.presence.frame();

        // **The frost is recorded here, in the BASE segment, before anything this surface draws.**
        //
        // Two placements matter and neither is arbitrary:
        //
        // - **Not inside `with_overlay`.** Everything this surface draws goes to the deferred
        //   overlay band so it composites above its siblings; a backdrop must sit in the base
        //   segment instead, because the host performs it between flushing the base and flushing
        //   the overlay bands — which is exactly "after everything beneath me, before me".
        // - **Faded by the animation but NOT scaled.** `frame.apply` also scales and translates,
        //   and a blurred *region* that zooms is not what "blur what is behind me" means. The
        //   opacity alone is wanted: it is what makes the backdrop dissolve with the surface that
        //   asked for it.
        //
        // It blurs what it occludes — the viewport when blocking, the panel alone when not —
        // mirroring `overlay_occludes`, so the frost and the input policy can never disagree about
        // this surface's reach.
        if self.frosted {
            let reach = match self.blocking {
                true => self.scrim_rect(panel),
                false => panel,
            };
            cx.with_opacity(frame.opacity, |cx| {
                cx.backdrop_blur(reach, frost_radius, 1.0);
            });
        }

        frame.apply(cx, self.scale_origin(), |cx| {
            cx.with_overlay(|cx| {
                // Scrim over the whole viewport — the visual half of the blocking
                // layer policy (the event half swallows outside input below).
                if self.blocking {
                    cx.rect(
                        self.scrim_rect(panel),
                        background.with_alpha(scrim_a),
                        None,
                        0.0,
                        None,
                    );
                }

                // Lift the panel, fill it, stamp the shared bracket reticle (same
                // visual language as Pane / DockFrame) — the base layer takes the
                // chrome plain; specializations pass their own accents.
                paint_panel_chrome(cx, panel, PanelChrome::default());

                // The panel's real children on top of the fill. A nested overlay
                // painted in here records a DEEPER scene segment → composites above
                // everything this layer draws (Scene::overlay_segments).
                for child in &self.base.children {
                    paint_child(child.as_ref(), cx);
                }
            })
        });
    }

    /// What the panel did not take. A press here landed on the scrim: it fires
    /// `on_outside_click`, and a **blocking** layer swallows it (and every other pointer event)
    /// so nothing behind the modal is driven through it.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        match ev {
            Event::PointerDown(p) => {
                if !self.panel_bounds().contains(p.pos)
                    && let Some(f) = &self.on_outside_click
                {
                    f();
                }
                self.swallow()
            }
            Event::PointerUp(_) | Event::PointerMove(_) | Event::Scroll(_) => self.swallow(),
            // **The dismiss key, answered where the keys already arrive.**
            //
            // An open overlay holds the keyboard — it follows `open`, set in the constructor —
            // so this key has always been delivered here and was simply dropped. Every surface
            // built on top wrote its own dismissal instead (`Dialog`, `ContextMenu`,
            // `CommandPalette`: three copies of one sentence), and a surface **composed** rather
            // than built had none at all — a plugin's overlay could not be closed from the
            // keyboard (F003/P097/T502).
            //
            // **Unanswered when nothing was declared**, which is what keeps those three exactly as
            // they were: they set no dismissal on their inner overlay, so the key passes up to
            // them untouched. A surface that declares one closes on it, whoever composed it.
            // **Tab moves through the panel, and stays inside it.**
            //
            // Universal, so it is not gated on anything being declared: an overlay is a place the
            // keyboard is contained, and containment is what makes Tab mean something here rather
            // than wandering into the page behind. `Dialog` did this for itself and nothing else
            // did, which is why a composed surface — a plugin's — had no traversal at all.
            Event::Key {
                key: crate::component::GridKey::Tab,
                pressed: true,
            } => {
                let forward = !crate::event::modifiers().shift;
                Component::advance_focus(self, forward);
                Handled::Yes
            }
            Event::Widget(WidgetIntent::Dismiss) => match &self.on_dismiss {
                Some(f) => {
                    f();
                    Handled::Yes
                }
                None => Handled::No,
            },
            // A **non-blocking** layer is not on the path of a press beside it, so the press
            // reaches it as the outside event instead. Same hook, both shapes.
            Event::PointerDownOutside(_) => {
                if let Some(f) = &self.on_outside_click {
                    f();
                }
                Handled::No
            }
            _ => Handled::No,
        }
    }
}

impl Overlay {
    /// A blocking layer consumes what nothing in it wanted; a non-blocking one lets it through.
    fn swallow(&self) -> Handled {
        if self.blocking {
            Handled::Yes
        } else {
            Handled::No
        }
    }
}
