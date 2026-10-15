//! **The shared look**: flashes, drag marks, disabled dimming, bracket frames and a widget's base.

use super::*;

impl<'a> PaintCx<'a> {
    /// Queue a theme-tinted "press flash" overlay over `rect`. `amount` is the
    /// flash strength in `0.0..=1.0` (see [`Flash`](crate::effects::Flash));
    /// `radius` must match the widget's corner radius so the overlay follows a
    /// rounded shape instead of poking square corners past it.
    pub fn flash(&mut self, rect: Rectangle, amount: f32, radius: f32) {
        if amount <= 0.0 {
            return;
        }
        let a = (amount.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.rect(
            rect,
            self.theme.colors.foreground.with_alpha(a),
            None,
            radius,
            None,
        );
    }

    /// Draw the **drag ghost** — the small labelled chip that follows the cursor
    /// during a drag. Theme-driven: an accent-filled rounded rect with the label in
    /// the background color, painted on the **overlay layer** so it sits above all
    /// chrome. `rect` is the chip's bounds (the app positions it at the cursor).
    ///
    /// When `swap` is true the chip gains an inset **double frame** — the same motif
    /// as [`swap_indicator`](Self::swap_indicator) — so the cursor-following chip tells
    /// the user this drag is an **exchange**, not a move (there is no OS "swap" cursor).
    pub fn drag_ghost(&mut self, rect: Rectangle, text: &str, swap: bool) {
        let (accent, bg, radius) = (
            self.accent(),
            self.theme.colors.background,
            self.theme.colors.border_radius,
        );
        let font = (rect.size.h as f32 * 0.55).clamp(10.0, 15.0);
        self.with_overlay(|cx| {
            cx.rect(rect, accent.with_alpha(217), None, radius, None);
            cx.rect(
                rect,
                Color::TRANSPARENT,
                Some(Border {
                    color: accent,
                    width: 1.5,
                }),
                radius,
                None,
            );
            if swap {
                let inset = 3.0_f64;
                let inner = Rectangle::new(
                    Point::new(rect.loc.x + inset, rect.loc.y + inset),
                    Size::new(
                        (rect.size.w - inset * 2.0).max(0.0),
                        (rect.size.h - inset * 2.0).max(0.0),
                    ),
                );
                cx.rect(
                    inner,
                    Color::TRANSPARENT,
                    Some(Border {
                        color: bg.with_alpha(180),
                        width: 1.0,
                    }),
                    (radius - inset as f32).max(0.0),
                    None,
                );
            }
            cx.text(rect, text, bg, font, TextAlign::Center, TextStyle::REGULAR);
        });
    }

    /// Draw a **drop indicator** over a target's `bounds`: an accent insertion line
    /// for [`DropSide::Before`]/[`After`](DropSide), or an accent wash + outline for
    /// [`DropSide::Onto`]. Theme-driven (derives from `accent`); the app calls this
    /// over the bounds returned by [`resolve_at`](crate::drag::resolve_at).
    pub fn drop_indicator(&mut self, bounds: Rectangle, side: DropSide) {
        let accent = self.accent();
        match side {
            DropSide::Onto => {
                self.rect(
                    bounds,
                    accent.with_alpha(45),
                    None,
                    self.theme.colors.border_radius,
                    None,
                );
                self.rect(
                    bounds,
                    Color::TRANSPARENT,
                    Some(Border {
                        color: accent,
                        width: 1.5,
                    }),
                    self.theme.colors.border_radius,
                    None,
                );
            }
            DropSide::Before | DropSide::After => {
                let thickness = 2.0;
                let y = if side == DropSide::Before {
                    bounds.loc.y - thickness / 2.0
                } else {
                    bounds.loc.y + bounds.size.h - thickness / 2.0
                };
                self.rect(
                    Rectangle::new(
                        Point::new(bounds.loc.x, y),
                        Size::new(bounds.size.w, thickness),
                    ),
                    accent,
                    None,
                    1.0,
                    None,
                );
            }
        }
    }

    /// Draw a **swap indicator** over a target's whole `bounds` — for an *exchange*
    /// gesture (Shift+drag) where the entire target is swapped with the source. There
    /// is no before/after for a swap, so this deliberately draws a whole-item **double
    /// accent frame** (+ faint wash) instead of [`drop_indicator`](Self::drop_indicator)'s
    /// insertion line or `Onto` wash — a distinct cue that the whole item is the target.
    /// Theme-driven (derives from `accent` / `radius` / `border_width`).
    pub fn swap_indicator(&mut self, bounds: Rectangle) {
        let accent = self.accent();
        let radius = self.theme.colors.border_radius;
        // Faint wash + bold outer frame.
        self.rect(bounds, accent.with_alpha(28), None, radius, None);
        let outer_w = (self.theme.colors.border_width * 2.0).max(2.5);
        self.rect(
            bounds,
            Color::TRANSPARENT,
            Some(Border {
                color: accent,
                width: outer_w,
            }),
            radius,
            None,
        );
        // Inset second line → reads as a "double frame" (= swap the whole item),
        // visually separating it from the single-outline `Onto` look.
        let inset = 3.0_f64;
        let inner = Rectangle::new(
            Point::new(bounds.loc.x + inset, bounds.loc.y + inset),
            Size::new(
                (bounds.size.w - inset * 2.0).max(0.0),
                (bounds.size.h - inset * 2.0).max(0.0),
            ),
        );
        self.rect(
            inner,
            Color::TRANSPARENT,
            Some(Border {
                color: accent.with_alpha(120),
                width: 1.0,
            }),
            (radius - inset as f32).max(0.0),
            None,
        );
    }

    /// Dim `rect` with a background-colored scrim — the standard look for a
    /// **disabled** widget. `radius` must match the widget's corner radius so the
    /// scrim follows its rounded shape. DRY: every widget reuses this instead of
    /// dimming each color by hand.
    pub fn dim(&mut self, rect: Rectangle, radius: f32) {
        let a = (DISABLED_SCRIM * 255.0).round() as u8;
        self.rect(
            rect,
            self.theme.colors.background.with_alpha(a),
            None,
            radius,
            None,
        );
    }

    /// Draw the prominent **flat corner-bracket frame** used by container chrome
    /// ([`Pane`](crate::widgets::Pane), [`DockFrame`](crate::widgets::DockFrame)):
    /// a subtle continuous accent line tracing the full rounded perimeter, with
    /// bright thick accent **corners** (rounded arc + a short straight arm along
    /// each edge) layered on top — the Tron reticle. DRY: the frame is defined
    /// once here instead of per-widget.
    ///
    /// Width/radius come from `theme.colors.border_width`/`theme.colors.border_radius`: at
    /// `border_width == 0` the frame draws **nothing** (no border anywhere),
    /// consistent with every other widget's border gate. For a surface that
    /// carries its own width/radius (e.g. a self-themed sidebar shell), use
    /// [`bracket_frame_with`](Self::bracket_frame_with).
    pub fn bracket_frame(&mut self, rect: Rectangle) {
        let (radius, border_width) = (
            self.theme.colors.border_radius,
            self.theme.colors.border_width,
        );
        self.bracket_frame_with(rect, border_width, radius);
    }

    /// [`bracket_frame`](Self::bracket_frame) with an explicit `border_width` and
    /// corner `radius`, independent of the theme — so one surface can size its own
    /// reticle (the per-widget `Pane::border_width`/radius overrides). The accent
    /// color still comes from the theme. `border_width <= 0.0` ⇒ no frame.
    pub fn bracket_frame_with(&mut self, rect: Rectangle, border_width: f32, radius: f32) {
        let accent = self.accent();
        // Borders off (`border_width == 0`) ⇒ no frame at all, like every other
        // widget. A container that needs definition without a border should carry
        // a fill, not a forced hairline.
        if border_width <= 0.0 {
            return;
        }
        let b = rect;

        // Subtle continuous accent line tracing the whole rounded perimeter — the
        // dimmed "midsection" that the bright corners sit on top of.
        self.rect(
            b,
            Color::TRANSPARENT,
            Some(Border {
                color: accent.with_alpha(BRACKET_DIM_ALPHA),
                width: border_width,
            }),
            radius,
            None,
        );

        // Bright thick corner brackets. The renderer draws the accent border as a
        // band just OUTSIDE the rect edge, carrying the theme corner radius — so
        // redrawing that same border clipped to a corner-sized box yields a bright
        // *rounded* corner plus a short straight arm along each edge, without
        // faking the rounding. `keep` is the bright span from each corner (rounded
        // arc + `BRACKET_ARM_LEN`); `m` grows the clip box outward so it also
        // captures the outer border band hugging the corner.
        let bracket_width = border_width + BRACKET_WIDTH_BOOST;
        let keep = f64::from(radius + BRACKET_ARM_LEN);
        let m = f64::from(bracket_width);
        let (x, y, w, h) = (b.loc.x, b.loc.y, b.size.w, b.size.h);
        let bright = Border {
            color: accent,
            width: bracket_width,
        };

        let corners = [
            Point::new(x - m, y - m),               // top-left
            Point::new(x + w - keep, y - m),        // top-right
            Point::new(x - m, y + h - keep),        // bottom-left
            Point::new(x + w - keep, y + h - keep), // bottom-right
        ];
        for loc in corners {
            self.with_clip(Rectangle::new(loc, Size::new(keep + m, keep + m)), |cx| {
                cx.rect(b, Color::TRANSPARENT, Some(bright), radius, None);
            });
        }
    }

    /// Paint the shared chrome for a component's base (background/border/glow).
    pub fn paint_base(&mut self, base: &Base) {
        let s = &base.style;
        if s.visual.fill.is_none() && s.visual.border.is_none() && s.visual.glow.is_none() {
            return;
        }
        self.rect(
            base.bounds,
            s.visual.fill.unwrap_or(Color::TRANSPARENT),
            s.visual.border,
            s.visual.radius,
            s.visual.glow,
        );
    }

    /// The theme-driven **rest glow** a widget surface carries before any
    /// hover/focus/active state: color from the theme's `glow` token, intensity
    /// from `interaction.control_rest_glow` (`None` when that token is `0` — the
    /// flat look), halo radius supplied by the caller (each widget scales its own).
    /// This is the single definition every widget shares, so the whole library
    /// honors the `glow_size` setting at rest uniformly (the returned glow runs
    /// through `scaled_glow` in `rect` like every other).
    /// Widgets add their own gates on top (a disabled control never halos); a
    /// tone-following widget (e.g. a destructive button) overrides the color.
    pub fn rest_glow(&self, radius: f32) -> Option<Glow> {
        let i = self.theme.colors.interaction.control_rest_glow;
        (i > 0).then_some(Glow {
            color: self.theme.colors.glow,
            radius,
            intensity: i as f32 / 255.0,
        })
    }

    /// Scale a glow by the theme's `glow_size` token (the **sole** owner of glow:
    /// presence + halo radius). `intensity` is intentionally *not* applied here —
    /// it controls only the scanline/CRT overlay — so the two settings no longer
    /// overlap. `GlowLevel::None` drops the glow entirely.
    pub(super) fn scaled_glow(&self, glow: Option<Glow>) -> Option<Glow> {
        // `GlowLevel` (theme/config `glow_size`) owns BOTH dimensions of glow:
        // `radius_scale` (halo size) and `strength_scale` (alpha). Apply both here —
        // the single chokepoint — so every widget's glow tracks the config uniformly
        // instead of baking a fixed intensity. `Medium` (default) is 1.0×, so this is
        // a no-op for the default theme; `none` drops glow entirely.
        let radius = self.theme.colors.glow_size.radius_scale();
        let strength = self.theme.colors.glow_size.strength_scale();
        glow.filter(|_| radius > 0.0).map(|g| Glow {
            radius: g.radius * radius,
            intensity: g.intensity * strength,
            ..g
        })
    }
}
