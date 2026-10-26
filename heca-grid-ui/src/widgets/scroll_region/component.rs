//! Component lifecycle, painting and input.

use super::*;

impl Component for ScrollRegion {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Layout just re-computed every bound to its natural position — clear the
    /// baked shift so the next `sync_shift` re-applies it from scratch instead
    /// of compounding. (Post-order: children already assigned.)
    ///
    /// The offset is also **re-clamped** here: a relayout can GROW the viewport
    /// (window resize, zoom-out), leaving the stored offset beyond the new max —
    /// re-applying it raw would shift the content past the edge with no
    /// scrollbar left to bring it back. `scroll_to`/`scroll_to_x` clamp against
    /// the freshly-assigned natural bounds and re-bake the shift.
    fn on_layout(&mut self) {
        self.applied_offset = 0.0;
        self.applied_offset_x = 0.0;
        let mut y = self.scroll_offset.get_untracked();
        let bottom = self.max_offset() as f32;
        if self.follow_bottom && y >= self.previous_bottom {
            y = bottom;
        }
        self.previous_bottom = bottom;
        let x = self.scroll_offset_x.get_untracked();
        self.set_offset_y(y, None);
        self.set_offset_x(x, None);
        self.reserve_scrollbar_gutters();
        self.follow_revealed();
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let vp = self.base.bounds;
        // Styled-surface decoration (background/border/glow/radius from the theme
        // via `StyleExt`) painted in viewport space, before the clipped content — a
        // plain region sets none of these and stays frameless (and, having no
        // surface, carries no glow). A STYLED region (a scrollable panel) without an
        // explicit `.glow(..)` falls back to the theme rest glow, like every surface.
        let s = &self.base.style;
        if (s.visual.fill.is_some() || s.visual.border.is_some()) && s.visual.glow.is_none() {
            cx.rect(
                vp,
                s.visual.fill.unwrap_or(crate::color::Color::TRANSPARENT),
                s.visual.border,
                s.visual.radius,
                cx.rest_glow(SURFACE_GLOW_RADIUS),
            );
        } else {
            cx.paint_base(&self.base);
        }
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
        // Scrollbar thumb on top, in viewport space (not scrolled with content).
        // Theme-driven hover affordance (mirrors MarkerGroup's grip bar): the
        // thumb is a dim accent at rest, brightens when its grab lane is hovered,
        // and is full-bright while dragged — reading as "grab here". Radius and
        // color both come from the theme (no hardcoded radius/Color).
        if let Some(t) = self.thumb_rect() {
            let alpha = if self.thumb_grab.is_some() || self.thumb_hovered {
                cx.theme().colors.interaction.thumb_hover
            } else {
                cx.theme().colors.interaction.thumb_rest
            };
            let theme = cx.theme();
            let color = cx.accent().with_alpha(alpha);
            cx.rect(t, color, None, theme.colors.control_radius(), None);
        }
        // Horizontal scrollbar thumb (bottom edge), same theme-driven affordance.
        if let Some(t) = self.h_thumb_rect() {
            let alpha = if self.h_thumb_grab.is_some() || self.h_thumb_hovered {
                cx.theme().colors.interaction.thumb_hover
            } else {
                cx.theme().colors.interaction.thumb_rest
            };
            let theme = cx.theme();
            let color = cx.accent().with_alpha(alpha);
            cx.rect(t, color, None, theme.colors.control_radius(), None);
        }
    }

    /// The region **clips** its content, so a press or a move outside its bounds must not reach
    /// content that has been scrolled out of sight. `dispatch` applies that; the widget only
    /// declares it.
    fn clips_children(&self) -> bool {
        true
    }

    /// **What the content is cut to**: the viewport short of each scrollbar's lane, widened on an
    /// axis that does not scroll. The framework cuts every child to it as it draws it.
    fn children_clip(&self, _theme: &crate::theme::Theme) -> Option<crate::scene::Rounded> {
        // Reserve a gutter for each visible scrollbar so content is never drawn
        // *under* the thumb: clip the content short of the lane on the right (when
        // the vertical bar shows) and/or the bottom (horizontal bar). The thumbs are
        // then painted below in the full viewport rect, sitting in the clear gutter.
        let mut content_clip = self.base.bounds;
        if self.v_overflow() {
            content_clip.size.w = (content_clip.size.w - SCROLLBAR_GUTTER).max(0.0);
        }
        if self.h_overflow() {
            content_clip.size.h = (content_clip.size.h - SCROLLBAR_GUTTER).max(0.0);
        }
        // Widen the clip on an axis the region does NOT scroll, so a child's GLOW
        // (which paints outside its own bounds — every bordered surface carries a
        // rest halo) isn't scissored flat against the edge, which reads as the row
        // being "cut off". Only the scrolling axis needs a tight clip — that is the
        // one where content genuinely moves through the viewport and must be cut at
        // the boundary. A non-scrolling axis has nothing to hide, so the few px of
        // bleed is free.
        //
        // Safe to widen on both sides now that `reserve_scrollbar_gutters` keeps content out of the
        // lane by LAYOUT: the clip is no longer the only thing standing between a card and the
        // scrollbar, so letting a halo bleed a few px costs nothing. It was not always so — when
        // the gutter existed only as a clip, this widening handed it straight back and content was
        // drawn under the bar.
        if !self.axes.is_horizontal() {
            content_clip.loc.x -= GLOW_BLEED;
            content_clip.size.w += 2.0 * GLOW_BLEED;
        }
        if !self.axes.is_vertical() {
            content_clip.loc.y -= GLOW_BLEED;
            content_clip.size.h += 2.0 * GLOW_BLEED;
        }
        Some(crate::scene::Rounded {
            rect: content_clip,
            radius: 0.0,
        })
    }

    /// Before the children: the offset has to be baked into bounds so everything below hit-tests
    /// against what is actually drawn, and a scrollbar gesture beats whatever sits under it.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        // Bake the current scroll offset into bounds first, so hit-testing and
        // DnD see the visual position (bounds === what's drawn).
        self.sync_shift();
        let vp = self.base.bounds;

        if matches!(ev, Event::Key { pressed: true, .. } | Event::TextInput(_) | Event::Widget(_)) {
            self.reveal_paused = false;
        }
        match ev {
            Event::PointerDown(p) => self.press_capture(p.pos, vp, ev),
            Event::PointerMove(p) => self.move_capture(p.pos, vp, ev),
            // Scroll and the release deliberately fall through to the children first — see `on_event`.
            _ => Handled::No,
        }
    }

    /// After the children have declined: the wheel (so the innermost hovered region wins, which is
    /// what makes nesting work) and the end of a gesture.
    fn on_event(&mut self, ev: &Event) -> Handled {
        let vp = self.base.bounds;
        match ev {
            Event::Scroll(p) => {
                // **Ctrl or Cmd with the wheel is a zoom, not a scroll** — the browser's rule, and
                // the reason the gesture means the same over a list as over a terminal. The region
                // lets it carry on outward, to whatever zooms.
                if p.modifiers.ctrl || p.modifiers.meta {
                    return Handled::No;
                }
                // **The wheel carries a position now**, and the router only delivers it to what is
                // under the cursor — so being here *is* the hover test this arm used to do by
                // hand against a flag it maintained from moves. What is left is the honest
                // question: can this region scroll the axis it was asked for? If not it declines,
                // and the event carries on to the region outside it.
                //
                // The host already mapped modifiers to axes (plain wheel → `delta_y`,
                // `Shift`+wheel → `delta_x`), so each axis is simply consumed if it can move.
                let mut handled = false;
                let (min_y, max_y) = self.offset_bounds_y();
                let (min_x, max_x) = self.offset_bounds_x();
                if p.delta_y != 0.0 && max_y > min_y {
                    let next = self.scroll_offset.get_untracked() as f64
                        + (p.delta_y as f64) * WHEEL_STEP_FRAC * vp.size.h;
                    self.set_offset_y(next as f32, Some(ev));
                    handled = true;
                }
                if p.delta_x != 0.0 && max_x > min_x {
                    let next = self.scroll_offset_x.get_untracked() as f64
                        + (p.delta_x as f64) * WHEEL_STEP_FRAC * vp.size.w;
                    self.set_offset_x(next as f32, Some(ev));
                    handled = true;
                }
                if handled { Handled::Yes } else { Handled::No }
            }
            // Keyboard scrolling. Semantic, so the arithmetic lives here rather
            // than in the app: the widget is what knows its viewport and its content.
            //
            // After the children, like the wheel, so the innermost scrollable region wins when
            // regions nest. A region that cannot scroll the axis asked for returns `No` and the
            // event carries on — a vertical-only region must not swallow a horizontal page.
            Event::Widget(intent) => self.scroll_intent(*intent, ev),
            Event::PointerUp(_) => {
                // No bounds check: a drag that started here ends here, wherever the cursor drifted
                // to — the press captured the pointer, so the release is delivered here whatever
                // it is over. Gating a release on position is exactly how a thumb gets stuck.
                let was_dragging = self.release_grabs();
                if was_dragging {
                    self.end_scroll(Some(ev));
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
            _ => Handled::No,
        }
    }
    /// Children first (the default recursion), then the held-track-press repeat:
    /// after `TRACK_REPEAT_DELAY` a held press in a scrollbar track keeps paging
    /// toward the cursor every `TRACK_REPEAT_INTERVAL` until released. Returns
    /// `true` while a press is held so the host keeps ticking.
    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating |= self.advance_eased(dt);
        let mut fire: Option<(bool, Point)> = None;
        if let Some(tr) = &mut self.track_repeat {
            tr.next_in -= dt;
            if tr.next_in <= 0.0 {
                tr.next_in = TRACK_REPEAT_INTERVAL;
                fire = Some((tr.horizontal, tr.pos));
            }
            animating = true;
        }
        if let Some((horizontal, pos)) = fire {
            self.page_toward(horizontal, pos, None);
        }
        // A wheel gesture reports every movement and never reports stopping, so the end is a
        // pause. Keep asking for frames while one is pending, or the timer would only advance when
        // something else happened to redraw — and the end would land late, or never.
        if let Some(left) = self.wheel_idle {
            let left = left - dt;
            if left <= 0.0 {
                self.wheel_idle = None;
                self.end_scroll(None);
            } else {
                self.wheel_idle = Some(left);
                animating = true;
            }
        }
        animating
    }
}
