//! **What is in force where a widget paints**: the band it draws into, the clip, and the colours
//! a container publishes to what it holds.

use super::*;

impl<'a> PaintCx<'a> {
    /// Run `f` with draws routed to the scene's **overlay layer** (painted on
    /// top of everything). Used by popovers/dropdowns for correct z-order.
    pub fn with_overlay(&mut self, f: impl FnOnce(&mut PaintCx<'a>)) {
        // The overlay band starts unclipped, so no corner from below reaches into it.
        let rounded = self.rounded.take();
        self.scene.begin_overlay();
        f(self);
        self.scene.end_overlay();
        self.rounded = rounded;
    }

    /// **Paint `f` over this widget's children.** A widget paints first and its children after, so
    /// a frame drawn in `paint` sits under whatever it holds; drawn here it sits over it. See
    /// [`Scene::begin_outline`](crate::scene::Scene::begin_outline) — it keeps the clips it was
    /// called inside, and paints in place inside an overlay.
    pub fn outline(&mut self, f: impl FnOnce(&mut PaintCx<'a>)) {
        self.scene.begin_outline();
        f(self);
        self.scene.end_outline();
    }

    /// Run `f` with all its draws **clipped** to `rect` (logical px). Content that
    /// falls outside is scissored away by the renderer — the primitive a scrolling
    /// viewport uses so partial rows/glyphs are cut at the panel edge instead of
    /// spilling out. Nested clips intersect with their parent.
    pub fn with_clip(&mut self, rect: Rectangle, f: impl FnOnce(&mut PaintCx<'a>)) {
        self.with_rounded_clip(rect, 0.0, f);
    }

    /// [`with_clip`](Self::with_clip) with **round corners** of `radius` (CSS `overflow: hidden`
    /// with `border-radius`). The renderer cuts square; the corners reach the content a host draws
    /// itself, carried on each surface painted inside (see [`Rounded`](crate::scene::Rounded)), so
    /// a terminal in a rounded pane follows its corners wherever the pane is drawn.
    pub fn with_rounded_clip(
        &mut self,
        rect: Rectangle,
        radius: f32,
        f: impl FnOnce(&mut PaintCx<'a>),
    ) {
        // Through the one door, so the clip follows the picture's translation and scale like
        // everything it cuts.
        self.emit(DrawCommand::PushClip(self.placed(rect)));
        let rounded = self.rounded;
        if radius > 0.0 {
            self.rounded = Some(crate::scene::Rounded {
                rect: self.scaled_rect(self.placed(rect)),
                radius: radius * self.scale,
            });
        }
        // Remembered as well as emitted: the renderer's scissor cannot help a draw that goes into
        // the **overlay band**, which starts unclipped on purpose (`Scene::begin_overlay`). A
        // widget drawing there asks `clip()` and keeps itself inside by hand.
        let outer = self.clip.replace(match self.clip {
            Some(outer) => outer
                .intersection(rect)
                .unwrap_or(Rectangle::new(rect.loc, Size::new(0.0, 0.0))),
            None => rect,
        });
        f(self);
        self.clip = outer;
        self.rounded = rounded;
        self.emit(DrawCommand::PopClip);
    }

    /// **What is currently clipping this paint** — every open [`with_clip`](Self::with_clip)
    /// intersected, in the widget's own layout coordinates. `None` means nothing does.
    ///
    /// For a widget that draws into the overlay band, where the renderer's scissor does not reach:
    /// see `paint_hint_label`, which keeps a keycap inside the
    /// dock its row lives in rather than letting it fall on the frame.
    pub fn clip(&self) -> Option<Rectangle> {
        self.clip
    }

    /// The active theme.
    pub fn theme(&self) -> &Theme {
        self.theme
    }

    /// Paint `f`'s subtree with `color` as the **inherited content color** — the color that
    /// unstyled text and glyphs ([`Label`](crate::widgets::Label), [`Icon`](crate::widgets::Icon))
    /// use when the caller gave them none. This is `color` inheritance in the CSS sense, and it is
    /// how a control tints the content it *composes* rather than draws.
    ///
    /// A control (e.g. [`Button`](crate::widgets::Button)) cannot set its children's colors
    /// directly: children are `impl Component`, so it doesn't know their types, and the
    /// [`Theme`] its state color derives from is only reachable here, in `paint`. So instead of
    /// pushing color *into* the children, it publishes one value they *pull*:
    ///
    /// ```ignore
    /// // In the control's `paint`, after its own chrome:
    /// let tint = accent.lerp(on_accent, hover_progress);          // state-derived, per frame
    /// cx.with_content_color(tint, |cx| {
    ///     for child in &self.base.children { child.paint(cx); }   // children just paint themselves
    /// });
    /// ```
    ///
    /// The control repaints every frame while its hover/press animation runs, so the value changes
    /// each frame and **the children animate without knowing anything about hover, or the parent,
    /// or animation at all**.
    ///
    /// The fallback is deliberately narrow. It applies only where a widget has no color of its own:
    /// an explicit `.color(..)` always wins, and a widget with an *intrinsic semantic* color
    /// (`Badge::danger`, `StatusDot::online`) ignores it entirely — exactly as a `.badge-danger`
    /// stays red inside a colored parent on the web. Nesting restores the outer value on exit.
    pub fn with_content_color(&mut self, color: Color, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = self.content_color.replace(color);
        f(self);
        self.content_color = previous;
    }

    /// The inherited content color, if a parent published one via
    /// [`with_content_color`](Self::with_content_color). Widgets that render bare text or glyphs
    /// resolve their color as: **own explicit color → this → a theme token** (usually
    /// `theme.colors.foreground`).
    pub fn content_color(&self) -> Option<Color> {
        self.content_color
    }

    /// Paint the closure's subtree with `tone` as the **inherited control tone** — the *chrome*
    /// counterpart of [`with_content_color`](Self::with_content_color).
    ///
    /// Content colour is ink: it reaches text and glyphs. This reaches a control's own
    /// **chrome** — a [`Button`](crate::widgets::Button)'s border and hover fill, an
    /// [`IconButton`](crate::widgets::IconButton)'s hover tint — which is otherwise the theme
    /// accent and nothing else. A composition that has a hue of its own needs both: a
    /// notification card is severity-toned, and a `Retry` inside a *danger* card cannot be blue.
    ///
    /// **Why a second channel rather than widening the first**: `Item`, `Row`, `Choice`,
    /// `ContextMenu`, `CommandPalette` and `Select` all publish a content colour today, and every
    /// button composed inside one would have silently restyled. This one starts empty — nothing
    /// publishes it — so a control looks exactly as it did unless a container asks otherwise.
    ///
    /// The tone is a **theme token resolved by the publisher at paint**, never a colour stored at
    /// build time, so a theme reload re-tones what is already on screen. Nesting restores the
    /// outer value on exit.
    pub fn with_accent(&mut self, tone: Color, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = self.published_accent.replace(tone);
        f(self);
        self.published_accent = previous;
    }

    /// The inherited control tone, if a container published one via
    /// [`with_accent`](Self::with_accent). A control resolves its hue as: **its own
    /// explicit tone → this → the theme accent.**
    ///
    /// Prefer [`accent`](Self::accent), which applies the last two steps for you. Reach for this
    /// only when a widget has its own tone to try first: `self.tone.unwrap_or_else(||
    /// cx.accent())`.
    pub fn published_accent(&self) -> Option<Color> {
        self.published_accent
    }

    /// **The accent to paint chrome with** — the hue a container published, else the theme's.
    ///
    /// This is the accessor for the accent as a *hue*: a focus ring, a hover fill, a selected
    /// wash, a scrollbar thumb, a caret. Reading `cx.theme().colors.accent` for those is the bug
    /// it replaces — it ignores whatever the containing composition asked for, so a control inside
    /// a toned subtree stayed the global accent while the button beside it followed. The rule
    /// (**own tone → inherited → theme**) was written out by hand in three widgets and nowhere
    /// else; it lives here now, so the next widget gets it without knowing it exists.
    ///
    /// ⚠️ **Not for the accent as a NAME.** `BadgeVariant::Accent`, `AlertVariant::Info`,
    /// `ToastSeverity::Info` and `Tone::Accent` are *declared meanings* — what the author said
    /// the thing IS — and a container's hue does not get to redefine them, exactly as a
    /// destructive button stays destructive inside a warning-toned panel. Those keep reading the
    /// theme directly.
    ///
    /// **Why it matters beyond styling**: a subtree that needed its own accent used to get it by
    /// being painted under a *swapped theme*, which forces a separate paint call per subtree and is
    /// what stopped a container from ever painting its own children. With the hue inherited
    /// instead, painting is the same walk for everything.
    pub fn accent(&self) -> Color {
        self.published_accent.unwrap_or(self.theme.colors.accent)
    }

    /// Paint the closure's subtree with `glow` as the **inherited content glow** — the
    /// halo counterpart of [`with_content_color`](Self::with_content_color), and it
    /// exists for the same reason.
    ///
    /// A control that *composes* its content holds its children as `impl Component`:
    /// it cannot reach in and style them, and it cannot know whether a child is even
    /// a glyph. [`RailCell`](crate::widgets::RailCell) is the case — its resting look
    /// *is* a bare icon, so the only way to give that icon a halo is for the cell to
    /// publish one and let the glyph pull it.
    ///
    /// The published glow is **unscaled**, like the one [`rest_glow`](Self::rest_glow)
    /// returns: `glow_size` is applied once at the drawing chokepoint
    /// ([`icon_glowing`](Self::icon_glowing), [`rect`](Self::rect)), never by the
    /// publisher. Scaling before publishing would apply the setting twice.
    pub fn with_content_glow(&mut self, glow: Option<Glow>, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = std::mem::replace(&mut self.content_glow, glow);
        f(self);
        self.content_glow = previous;
    }

    /// The inherited content glow, if a parent published one via
    /// [`with_content_glow`](Self::with_content_glow).
    pub fn content_glow(&self) -> Option<Glow> {
        self.content_glow
    }
}
