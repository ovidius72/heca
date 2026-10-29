//! The button's runtime: how it measures, paints every variant, answers the
//! pointer and keyboard, and eases its hover each frame.

use super::*;
use crate::color::Color;
use crate::component::{Component, Event, GridKey, Handled, PaintCx};
use crate::event::WidgetIntent;
use crate::font::MONO_ADVANCE_RATIO;
use crate::reactive::SignalGet;
use crate::scene::{Border, Glow};
use crate::style::Length;
use crate::widgets::Label;
use heca_core::layout::{Point, Rectangle, Size};

impl Button {
    /// Border that eases from semi-opaque (rest) to solid (hover) by `p`. The
    /// stroke `width` is the theme's `border_width` (so `border_width == 0` means
    /// no border, like every other surface).
    fn animated_border(&self, c: Color, p: f32, width: f32, rest_border: f32) -> Option<Border> {
        if !self.show_border || width <= 0.0 {
            return None;
        }
        let a = rest_border + (255.0 - rest_border) * p.clamp(0.0, 1.0);
        Some(Border {
            color: c.with_alpha(a.round() as u8),
            width,
        })
    }

    /// Paint the composed content (the children) under the button's **state color**.
    ///
    /// The button never draws its content and never touches its children: it publishes one
    /// theme-derived color for this frame, and unstyled children ([`Label`]/[`Icon`](crate::widgets::Icon)) inherit it
    /// (see [`PaintCx::with_content_color`]). Because the button repaints every frame while its
    /// hover progress eases, the value changes per frame and the content animates with it — with
    /// no knowledge of hover, and no per-child wiring. A child with its own color keeps it.
    ///
    /// A disabled button fades the content to `muted` — the one universal, theme-driven "inactive"
    /// cue that reads on every variant, including transparent Ghost/Link where a scrim would not.
    fn paint_content(&self, cx: &mut PaintCx, color: Color) {
        let color = if self.base.disabled.get_untracked() {
            cx.theme()
                .colors
                .muted
                .with_alpha((DISABLED_CONTENT_ALPHA * 255.0).round() as u8)
        } else {
            color
        };
        cx.with_content_color(color, |cx| {
            for child in &self.base.children {
                crate::component::paint_child(child.as_ref(), cx);
            }
        });
    }

    /// The content box: the bounds minus the horizontal padding — i.e. the strip the children
    /// occupy. Used by the [`Link`](ButtonVariant::Link) underline, which therefore spans the real
    /// content whatever it is, instead of a width faked from a character count.
    fn content_box(&self) -> Rectangle {
        let b = self.base.bounds;
        let pad = self.base.style.layout.pad_left(self.base.font) as f64;
        Rectangle::new(
            Point::new(b.loc.x + pad, b.loc.y),
            Size::new((b.size.w - 2.0 * pad).max(0.0), b.size.h),
        )
    }

    /// A fill rising from the bottom by fraction `p`, with a glow (the
    /// bottom-to-top sweep).
    fn paint_rising_fill(&self, cx: &mut PaintCx, fill: Color, glow: Color, p: f32, radius: f32) {
        let b = self.base.bounds;
        let fh = b.size.h * p as f64;
        let rect = Rectangle::new(
            Point::new(b.loc.x, b.loc.y + b.size.h - fh),
            Size::new(b.size.w, fh),
        );
        let g = self.show_glow.then_some(Glow {
            color: glow,
            radius: GLOW_RADIUS,
            intensity: GLOW_INTENSITY,
        });
        cx.rect(rect, fill, None, radius, g);
    }

    /// A thin underline beneath the content, spanning the [content box](Self::content_box).
    fn paint_underline(&self, cx: &mut PaintCx, color: Color) {
        let content = self.content_box();
        let y = content.loc.y + content.size.h / 2.0 + (self.base.font as f64 * 0.5);
        cx.rect(
            Rectangle::new(Point::new(content.loc.x, y), Size::new(content.size.w, 1.5)),
            color,
            None,
            0.0,
            None,
        );
    }

    /// The button's one action: flash, then run `on_click`. Every way of pressing it — pointer,
    /// keyboard, or a caller invoking [`Component::activate`] — goes through here, so they cannot
    /// drift apart and none of them has to know how the others work.
    fn fire(&mut self) {
        self.flash.trigger();
        if let Some(f) = &self.on_click {
            f();
        }
    }
}

impl Component for Button {
    /// A caller with only a `dyn Component` can press this button — no synthetic keypress, no
    /// focus required, because an addressed call is not an event competing for a target.
    /// Its own [`icon_only`](Self::icon_only), reachable through `dyn Component` so a container
    /// short of room can ask without knowing what its children are.
    /// A container styling a row as one group hands its variant down; a button that was given its
    /// own keeps it, which is what lets a quiet toolbar still have a destructive button in it.
    fn set_variant(&mut self, variant: ButtonVariant) {
        // **A named variant is the whole look, frame included, and a container does not take it
        // away.** A destructive button in a quiet row is the only framed one, and that is the
        // style speaking: danger is boxed. Handing it the row's chrome and leaving it only the hue
        // was tried and rejected — the variant is what the button looks like, not a colour a
        // parent may restyle.
        if self.variant == ButtonVariant::default() {
            self.variant = variant;
        }
    }

    fn set_icon_only(&mut self, on: bool) {
        let on = on && self.glyph.is_some();
        if self.icon_only == on {
            return;
        }
        self.icon_only = on;
        if on {
            // ⚠️ **Taken out of the tree, not hidden inside it.**
            //
            // `display: none` on the label leaves the button's box answered differently by the two
            // passes taffy makes — it reported its full height and was then placed as though it had
            // almost none, so the button (and anything holding it) sat low and hung out of its row.
            // Reproduced with a plain `Flex` holding one icon-only button, which is how it was found
            // A child that is not there has no such disagreement.
            self.base.children.retain(|c| c.text_summary().is_none());
        } else if !self.label.is_empty() {
            self.base
                .children
                .push(crate::WidgetModel::new(Label::new(self.label.clone()).bold(true)));
        }
        self.remeasure();
    }

    fn activate(&mut self) -> bool {
        self.fire();
        true
    }

    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The button **hugs its content**: `Auto` in both axes, so taffy measures whatever tree it
    /// holds (a label, an icon + label, a multi-line column) and the box follows. Only the padding
    /// and the gap are the button's own, and both derive from the resolved font + size variant, so
    /// the whole affordance scales together.
    ///
    /// Horizontal padding carries one character of breathing room per side on top of the base pad.
    /// That reproduces the old hand-computed width exactly — it was
    /// `(chars + 2) × font × advance + pad × 2`, i.e. the label plus two characters — but now the
    /// *label* is measured by the layout engine instead of by a character count, so the same
    /// spacing holds for content that isn't text at all.
    fn remeasure(&mut self) {
        let fs = self.base.font;
        let pad = BASE_PAD * self.base.size_scale();
        self.base.style.layout.padding = (pad).into();
        // The extra horizontal room is there for **text**. With the words off it is padding around
        // nothing, which is what made a group of icon-only buttons read as too big.
        let side = if self.icon_only {
            pad
        } else {
            pad + fs * MONO_ADVANCE_RATIO
        };
        self.base.style.layout.padding_x = Some((side).into());
        self.base.style.layout.padding_y = Some((pad).into());
        self.base.style.layout.gap = (fs * GAP_RATIO).into();
        self.base.style.layout.width = Length::Auto;
        self.base.style.layout.height = Length::Auto;
        // **Showing only an icon, it says it cannot give way** — CSS `flex-shrink: 0`.
        //
        // A row shares out a shortfall among whatever will take it, and a button carrying words can
        // take some: its [`Label`] ellipses. One down to its icon has nothing left to give, so
        // shrinking it only eats the control itself — the box narrows around a glyph that does not,
        // and what is left is a sliver too thin to click. The same reasoning `IconButton` already
        // states about its own size.
        //
        // It belongs here rather than in whatever container happens to hold the button, so a plain
        // [`Flex`](crate::widgets::Flex) of icon buttons is right without its author knowing to ask
        // — which is how the defect arrived: a container told its own children not to give way and
        // could not tell the one control it had built itself.
        //
        // ⚠️ **It only ever tightens.** A container may have its own reason to hold a *worded*
        // button rigid — [`ButtonGroup`](crate::widgets::ButtonGroup) does, because a row that
        // fits by ellipsing its labels never reports the overflow that moves a button into the
        // menu. Writing the relaxed value back here would undo that declaration on every layout
        // pass, which is the one thing a re-measure must never do to a property it shares.
        if self.icon_only {
            self.base.style.layout.flex_shrink = Some(0.0);
        }
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        // Snapshot theme colors so we can call &mut cx methods afterwards.
        let (
            surface,
            accent,
            glow_c,
            danger,
            foreground,
            muted,
            border_c,
            border_width,
            radius,
            on_accent,
            on_danger,
            ia,
        ) = {
            // **The hue a container published, if it published one** — a button composed inside a
            // severity-toned card follows the card, the way a `Label`'s ink already follows its
            // content colour. `None` unless a container asked, which is everywhere today, so a
            // button on its own is the theme accent exactly as before.
            let accent = cx.accent();
            let tone = cx.published_accent();
            let t = cx.theme();
            (
                t.colors.surface,
                accent,
                tone.unwrap_or(t.colors.glow),
                // **Destructive is not re-toned.** Its colour is what the variant *means*, not
                // decoration a parent may restyle — a Delete inside a warning-toned panel is still
                // a Delete. Same reasoning that keeps `Badge::danger` red inside a coloured parent.
                t.colors.danger,
                t.colors.foreground,
                t.colors.muted,
                t.colors.border,
                t.colors.border_width,
                t.colors.control_radius(),
                // The label on a *filled* button has to contrast with the fill it actually gets,
                // which is the effective hue and not necessarily the theme accent.
                t.colors.on(accent),
                t.colors.on(t.colors.danger),
                t.colors.interaction,
            )
        };
        // A disabled button never hovers/presses: pin progress to the rest state and strip the
        // vivid accent/danger chrome to `muted` so the whole affordance — border and fill, not
        // just the label — reads as inactive on every variant.
        let disabled = self.base.disabled.get_untracked();
        let p = if disabled {
            0.0
        } else {
            self.progress.clamp(0.0, 1.0)
        };
        let (accent, danger) = if disabled {
            (muted, muted)
        } else {
            (accent, danger)
        };
        // An explicit tone replaces the variant's hue wherever the variant would have used the
        // accent — content, hover wash, held-on frame — without changing which variant this is.
        // A tone the caller set is already in `accent` — `PaintCx::accent` resolved it, the same
        // way it resolves one a container published. Only the variant's own meaning is left to
        // decide here.
        let accent = if disabled {
            accent
        } else if self.dangerous {
            // The theme's danger, read at paint, so it follows a reload like every other colour.
            danger
        } else {
            accent
        };
        let b = self.base.bounds;

        // Faint theme-driven REST glow (`PaintCx::rest_glow`) on the bordered
        // variants, so a control carries the neon identity before any hover/focus and
        // the `glow_size` setting visibly scales it at rest. The tone follows
        // the variant (a destructive button halos in `danger`). Ghost/Link are
        // surface-less at rest and Secondary is the deliberately-quiet variant — none
        // of them halo; a disabled control is flat.
        let base_rest = (self.show_glow && !disabled)
            .then(|| cx.rest_glow(REST_GLOW_RADIUS))
            .flatten();
        let rest_i = base_rest.map_or(0.0, |g| g.intensity);
        let rest_glow = |color: Color| base_rest.map(|g| Glow { color, ..g });

        // **Held on**, drawn once beneath whatever the variant draws — so every variant shows the
        // state the same way and a new variant cannot forget it. The tone follows the ambient
        // control tone exactly as `IconButton`'s does, and the border uses `focus_border_width` so
        // a held button stays legible with decorative borders switched off: this is a status cue,
        // not decoration.
        if self.active && !disabled {
            let tone = cx.accent();
            let line_w = cx.theme().focus_border_width;
            let frame = (line_w > 0.0).then_some(Border {
                color: tone.with_alpha(ia.control_active_border),
                width: line_w,
            });
            let g = self.show_glow.then_some(Glow {
                color: glow_c,
                radius: GLOW_RADIUS,
                intensity: GLOW_INTENSITY,
            });
            cx.rect(b, tone.with_alpha(ia.control_active_fill), frame, radius, g);
        }

        match self.variant {
            ButtonVariant::Primary => {
                cx.rect(
                    b,
                    surface,
                    self.animated_border(accent, p, border_width, ia.control_rest_border as f32),
                    radius,
                    rest_glow(glow_c),
                );
                if p > 0.0 {
                    self.paint_rising_fill(cx, accent, glow_c, p, radius);
                }
                self.paint_content(cx, accent.lerp(on_accent, p));
            }
            ButtonVariant::Destructive => {
                cx.rect(
                    b,
                    surface,
                    self.animated_border(danger, p, border_width, ia.control_rest_border as f32),
                    radius,
                    rest_glow(danger),
                );
                if p > 0.0 {
                    let g = self.show_glow.then_some(Glow {
                        color: danger,
                        radius: GLOW_RADIUS,
                        intensity: GLOW_INTENSITY * p,
                    });
                    cx.rect(b, danger.with_alpha(alpha(p)), None, radius, g);
                }
                self.paint_content(cx, danger.lerp(on_danger, p));
            }
            ButtonVariant::Secondary => {
                // Subtle `muted` fill + `muted` border (both firm toward `foreground` on hover) —
                // a theme-consistent identity. `muted` always contrasts the background and is never
                // equal to `surface`/`border`, unlike the old `surface` fill + `theme.border` border
                // (which vanished on themes where surface≈background or border==surface).
                let bc = muted.lerp(foreground, 0.4 * p);
                cx.rect(
                    b,
                    muted.with_alpha(SECONDARY_FILL_ALPHA),
                    self.animated_border(bc, p, border_width, ia.control_rest_border as f32),
                    radius,
                    rest_glow(glow_c),
                );
                self.paint_content(cx, foreground);
            }
            ButtonVariant::Outline => {
                // Hover: vivid accent border, text → primary (accent), lightest glow.
                // At rest the theme rest-glow carries the halo (tight radius); hover
                // blends both radius and intensity over it as `p` rises.
                let fill = accent.with_alpha(alpha(p * 0.1));
                let g = self.show_glow.then_some(Glow {
                    color: glow_c,
                    radius: REST_GLOW_RADIUS + (GLOW_RADIUS * 0.8 - REST_GLOW_RADIUS) * p,
                    intensity: (GLOW_INTENSITY * 0.6 * p).max(rest_i),
                });
                let g = g.filter(|g| g.intensity > 0.0);
                cx.rect(
                    b,
                    fill,
                    self.animated_border(
                        muted.lerp(accent, p),
                        p,
                        border_width,
                        ia.control_rest_border as f32,
                    ),
                    radius,
                    g,
                );
                self.paint_content(cx, muted.lerp(accent, p));
            }
            ButtonVariant::Ghost => {
                let border = if self.show_border && p > 0.0 && border_width > 0.0 {
                    Some(Border {
                        color: border_c.with_alpha(alpha(p)),
                        width: border_width,
                    })
                } else {
                    None
                };
                // Surface-less at rest (no halo), but the hover surface + border that
                // fade in glow with the fade — like every other hovered control.
                let g = (self.show_glow && !disabled && p > 0.0).then_some(Glow {
                    color: glow_c,
                    radius: GLOW_RADIUS,
                    intensity: GLOW_INTENSITY * p,
                });
                cx.rect(b, surface.with_alpha(alpha(p)), border, radius, g);
                self.paint_content(cx, muted.lerp(foreground, p));
            }
            ButtonVariant::Link => {
                // Color stays constant on hover; press flashes the TEXT (no bg).
                self.paint_content(cx, accent.lerp(foreground, self.flash.amount() * 0.7));
                if p > 0.0 {
                    self.paint_underline(cx, accent.with_alpha(alpha(p)));
                }
            }
        }

        // Press flash — brightening overlay (Link flashes its text above instead).
        // Filled variants need a stronger flash to read over their bright fill.
        if !matches!(self.variant, ButtonVariant::Link) {
            let strength = match self.variant {
                ButtonVariant::Primary | ButtonVariant::Destructive => 0.95,
                _ => 0.6,
            };
            cx.flash(b, self.flash.amount() * strength, radius);
        }

        // No background scrim for the disabled state: it barely shows on transparent variants
        // (Ghost/Link) and keeps the content's bright hue. Instead the muted chrome above +
        // the faded `muted` content color (see `paint_content`) carry the disabled look on
        // every variant.

        // Focus ring — shown whenever the button is focused (not keyboard-only) and enabled. It
        // follows the button's own tone (a destructive button rings in `danger`, not `accent`) so
        // the focus cue matches the widget's border colour instead of clashing with it.
        if !disabled && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            // Focus-outline tone (theme-driven, light/dark-aware): the accent case uses the
            // theme's `focus_ring` token or the accent shifted toward `foreground`; a destructive
            // button derives the same shift from its own `danger` tone. Both stay distinct from the
            // widget's border on dark AND light themes.
            let ring = match self.variant {
                ButtonVariant::Destructive => cx.theme().colors.focus_ring_tone(danger),
                _ => cx.theme().colors.effective_focus_ring(),
            };
            cx.focus_ring(b, ring, radius);
        }
    }

    /// Capture, not bubble: this control is **one click target and one Tab stop**
    /// (`Base::focus_barrier`), so its composed content — an `Icon`, a `Label`, anything — must
    /// never see the press first. Handling it before the children is what keeps that true.
    ///
    /// It takes the press and **fires on the click**: the press captures the pointer (so the
    /// release comes here wherever the cursor went) and the click is what a press and a release on
    /// this button *mean*. Dragging off the button before letting go therefore cancels it, which
    /// is what every other control on the machine does — and it costs nothing here, because the
    /// pairing is the framework's.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            // Keyboard activation: Space/Enter on the focused button == a click. `dispatch` only
            // offers a raw key to the widget that owns the keyboard, so there is no focus check
            // here to forget — and no way for this button to take a key meant for something else.
            Event::Key {
                key: GridKey::Enter | GridKey::Space,
                pressed: true,
            } => {
                self.fire();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    /// **The click, after its children have declined it.**
    ///
    /// The press is taken in capture (so composed content can never take it first) and the click
    /// it turns into is delivered to whoever took that press — this control — which is what makes
    /// "one control, one click target" a framework rule rather than something each control
    /// arranges by swallowing events. Bubble, not capture, so an
    /// [`EventExt`](crate::builders::EventExt) handler registered on this widget gets first
    /// refusal and can take the click with `stop_propagation`.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::Click(_) => {
                self.fire();
                Handled::Yes
            }
            // **A focused button answers the activate key**.
            //
            // Keys reach the focus owner, so this only ever runs for the button the keyboard is
            // actually on — which is why it needs no focus test of its own.
            //
            // `Dialog` already documented this as the way it works ("when a button itself is
            // focused, Enter is delivered straight to it (it consumes it)") and fell back to
            // firing its *primary* action for the case where it was not consumed. The button never
            // consumed it, so every Enter went to the primary action instead of the focused
            // button: Tab to Cancel, press Enter, and you got OK.
            //
            // Found by building an overlay the way a plugin has to — `Overlay` + `Surface` +
            // `Button`s, with no `Dialog` to translate anything — where nothing answered Enter at
            // all. That surface has no primary action to fall back to, so the gap that was hidden
            // natively was total there.
            Event::Widget(WidgetIntent::Activate) => {
                self.fire();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;

        // Hover progress eases toward the hovered target.
        let target = if self.base.hovered() && !self.base.disabled.get_untracked() {
            1.0
        } else {
            0.0
        };
        if (self.progress - target).abs() >= 1e-3 {
            let step = dt / HOVER_DURATION;
            self.progress = if self.progress < target {
                (self.progress + step).min(target)
            } else {
                (self.progress - step).max(target)
            };
            animating = true;
        } else {
            self.progress = target;
        }

        // Press flash fades out.
        animating |= self.flash.tick(dt);

        // Composed content animates itself (e.g. a Spinner child): the button drives its own
        // hover/press, the children drive theirs. (The *color* of the content needs no ticking —
        // it is inherited at paint time; see `paint_content`.)
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }

        // Damage just our own rect each frame so the hover/press animation doesn't
        // force a whole-scene redraw (host safety net). Mirrors `Spinner`.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}
