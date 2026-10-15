//! **Where and how strongly a paint lands**: translation, scale, opacity, and the one door every
//! draw goes through so none can skip them.

use super::*;

/// Scale one command: its geometry through `place`, and **everything measured in pixels with it** —
/// the font size, the corner radius, the border width, the glow and shadow falloff and offsets.
///
/// That second half is the whole difference between a zoom and a mistake. A rect that halves while
/// its text stays 14px, its radius stays 6px and its 1px border stays 1px is not the same picture
/// further away; it is a different, wronger picture. `fade_command` beside it is the same idea for
/// colour.
fn scale_command(cmd: DrawCommand, k: f32, place: impl Fn(Rectangle) -> Rectangle) -> DrawCommand {
    let glow = |g: Glow| Glow {
        radius: g.radius * k,
        ..g
    };
    let shadow = |s: Shadow| Shadow {
        radius: s.radius * k,
        dx: s.dx * k,
        dy: s.dy * k,
        ..s
    };
    match cmd {
        DrawCommand::Host(h) => DrawCommand::Host(HostCmd {
            rect: place(h.rect),
            // A blur radius and a corner are distances, so they scale with everything else; a
            // surface id and an alpha are not distances.
            draw: match h.draw {
                HostDraw::Backdrop { radius, corner } => HostDraw::Backdrop {
                    radius: radius * k,
                    corner: corner * k,
                },
                other => other,
            },
            ..h
        }),
        DrawCommand::Rect(r) => DrawCommand::Rect(RectCmd {
            rect: place(r.rect),
            radius: r.radius * k,
            border: r.border.map(|b| Border {
                width: b.width * k,
                ..b
            }),
            glow: r.glow.map(glow),
            shadow: r.shadow.map(shadow),
            ..r
        }),
        DrawCommand::Text(t) => DrawCommand::Text(TextCmd {
            rect: place(t.rect),
            size: t.size * k,
            glow: t.glow.map(glow),
            ..t
        }),
        DrawCommand::Brackets(b) => DrawCommand::Brackets(BracketCmd {
            rect: place(b.rect),
            len: b.len * k,
            thickness: b.thickness * k,
            glow: b.glow.map(glow),
            ..b
        }),
        DrawCommand::Scanline(s) => DrawCommand::Scanline(ScanlineCmd {
            rect: place(s.rect),
            ..s
        }),
        // A clip is geometry too: it must follow the picture it clips, or a scaled subtree is cut
        // to the rect it had at life size.
        DrawCommand::PushClip(r) => DrawCommand::PushClip(place(r)),
        DrawCommand::PopClip => DrawCommand::PopClip,
    }
}

/// Scale every colour in a draw command by `a`, leaving its geometry alone.
///
/// Written out per command rather than as a blanket "multiply anything colour-shaped", because the
/// commands carry colours in several roles — a fill, a border, the light of a glow, the dark of a
/// shadow — and each has to be dimmed on its own or a fading panel loses its outline a frame before
/// its body, or keeps a halo around nothing.
fn fade_command(cmd: DrawCommand, a: f32) -> DrawCommand {
    let dim = |c: Color| c.with_alpha((c.a as f32 * a).round().clamp(0.0, 255.0) as u8);
    match cmd {
        // Host work composites at its own strength times the context's opacity, so a surface or a
        // frost inside a fading overlay fades with it.
        DrawCommand::Host(h) => DrawCommand::Host(HostCmd {
            alpha: h.alpha * a,
            ..h
        }),
        DrawCommand::Rect(r) => DrawCommand::Rect(RectCmd {
            fill: dim(r.fill),
            border: r.border.map(|b| Border {
                color: dim(b.color),
                ..b
            }),
            glow: r.glow.map(|g| Glow {
                color: dim(g.color),
                ..g
            }),
            shadow: r.shadow.map(|s| Shadow {
                color: dim(s.color),
                ..s
            }),
            ..r
        }),
        DrawCommand::Text(t) => DrawCommand::Text(TextCmd {
            color: dim(t.color),
            glow: t.glow.map(|g| Glow {
                color: dim(g.color),
                ..g
            }),
            ..t
        }),
        DrawCommand::Brackets(b) => DrawCommand::Brackets(BracketCmd {
            color: dim(b.color),
            glow: b.glow.map(|g| Glow {
                color: dim(g.color),
                ..g
            }),
            ..b
        }),
        DrawCommand::Scanline(s) => DrawCommand::Scanline(ScanlineCmd {
            color: dim(s.color),
            ..s
        }),
        // Clips carry no colour; they are geometry, and a fade must not move anything.
        clip @ (DrawCommand::PushClip(_) | DrawCommand::PopClip) => clip,
    }
}

impl<'a> PaintCx<'a> {
    /// Paint `f`'s subtree **translated** by `(dx, dy)` — the same components, drawn somewhere
    /// else.
    ///
    /// This is deliberately narrow. A component is laid out in exactly one place, and its bounds
    /// are the contract for hit-testing and drawing alike (`bounds === what is drawn === what is
    /// clickable`). But a control occasionally has to render content it *owns but does not hold* —
    /// a [`Select`](crate::widgets::Select) shows the chosen option in its trigger while that
    /// option is away in the open list. Nothing can be in two places, so the trigger draws a
    /// **second image** of it here.
    ///
    /// What is drawn this way is **not interactive**: it has no bounds of its own, so it is not
    /// hit-tested, focusable or hoverable — the control's own bounds are the click target. Use it
    /// only for such an echo, never to move a widget: shifting where a component *lives* is
    /// [`shift_subtree`] + [`Component::on_layout`], which keeps its bounds honest.
    pub fn with_translate(&mut self, dx: f64, dy: f64, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = self.offset;
        self.offset = (previous.0 + dx, previous.1 + dy);
        f(self);
        self.offset = previous;
    }

    /// Paint `f`'s subtree at `alpha` (`0.0..=1.0`) — a whole surface fading, not a colour choice.
    ///
    /// Multiplicative and nesting: a half-faded panel holding a half-faded row draws it at a
    /// quarter. Every command this context emits is scaled on its way out, so a widget needs to
    /// know nothing about it — which is the point. A fade is something done **to** a surface, and a
    /// surface that had to cooperate with its own fade would mean every widget carrying an alpha.
    ///
    /// It scales colour only, never geometry: the thing stays exactly where it is and stops being
    /// visible, which is what "fade" means. Nor does it change hit-testing — a surface on its way
    /// out is still there until whoever is fading it takes it away.
    pub fn with_opacity(&mut self, alpha: f32, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = self.opacity;
        self.opacity = previous * alpha.clamp(0.0, 1.0);
        f(self);
        self.opacity = previous;
    }

    /// Paint `f`'s subtree **as a picture of itself**, not as itself: the host work it records (a
    /// terminal's surface) is marked an echo, so whoever asks where a surface was drawn is not told
    /// the picture's place. What a carried widget's image under the pointer is painted with.
    pub fn with_echo(&mut self, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = self.echo;
        self.echo = true;
        f(self);
        self.echo = previous;
    }

    /// Paint `f`'s subtree **scaled** by `factor` about `origin` — the whole surface smaller or
    /// larger, not a different layout.
    ///
    /// This is what an overview/exposé opens and closes with, and what a fade cannot express: a map
    /// that appears by *zooming out from life size* says "this is the same thing, further away",
    /// where a fade says "a different picture". niri animates its overview exactly this way.
    ///
    /// **It scales the picture, not the layout.** Nothing is measured again, no widget is told, and
    /// no bounds change — which is the point: the tree is laid out once, at life size, and a scale
    /// is something done *to* it, exactly as [`with_opacity`](Self::with_opacity) is. It therefore
    /// does not change hit-testing either; a surface mid-zoom is still where its bounds say.
    ///
    /// Multiplicative and nesting, like opacity, so a scaled panel inside a scaled layer draws at
    /// the product. Every command is scaled on its way out through `emit` — position
    /// **and** size, and with them the font size, the corner radius, the border width, the glow and
    /// the shadow. Scaling geometry while leaving those alone is what makes a naive zoom look
    /// wrong: text that stays huge in a shrinking box, hairlines that turn into slabs.
    pub fn with_scale(&mut self, factor: f32, origin: Point, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = (self.scale, self.scale_origin);
        // Compose about the outer transform's own frame, so nesting behaves.
        self.scale_origin = self.scaled_point(origin);
        self.scale = previous.0 * factor.max(0.0);
        f(self);
        (self.scale, self.scale_origin) = previous;
    }

    /// A point under the active scale.
    pub(super) fn scaled_point(&self, p: Point) -> Point {
        if self.scale == 1.0 {
            return p;
        }
        let o = self.scale_origin;
        Point::new(
            o.x + (p.x - o.x) * self.scale as f64,
            o.y + (p.y - o.y) * self.scale as f64,
        )
    }

    /// A rect under the active scale: its position moves toward the origin and its size shrinks.
    pub(super) fn scaled_rect(&self, r: Rectangle) -> Rectangle {
        if self.scale == 1.0 {
            return r;
        }
        Rectangle::new(
            self.scaled_point(r.loc),
            Size::new(r.size.w * self.scale as f64, r.size.h * self.scale as f64),
        )
    }

    /// Emit a command, scaled by the active [opacity](Self::with_opacity) and
    /// [scale](Self::with_scale). The one door to the scene, so a new draw helper cannot forget to
    /// honour either one — it never hears about them.
    pub(super) fn emit(&mut self, cmd: DrawCommand) {
        let cmd = if self.scale == 1.0 {
            cmd
        } else {
            scale_command(cmd, self.scale, |r| self.scaled_rect(r))
        };
        let a = self.opacity;
        self.scene
            .push(if a >= 1.0 { cmd } else { fade_command(cmd, a) });
    }

    /// Apply the active [translation](Self::with_translate) to a rect on its way to the scene.
    pub(super) fn placed(&self, r: Rectangle) -> Rectangle {
        if self.offset == (0.0, 0.0) {
            return r;
        }
        Rectangle::new(
            Point::new(r.loc.x + self.offset.0, r.loc.y + self.offset.1),
            r.size,
        )
    }

    /// Set the visible viewport size (the host passes the window size).
    pub fn with_viewport(mut self, viewport: Size) -> Self {
        self.viewport = viewport;
        self
    }

    /// The visible viewport size in logical px.
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// Whether `r` lies fully outside the viewport (plus a halo margin for
    /// glow/shadow spill) and can be skipped. Off-screen content emits no draw
    /// command, so scrolling a tall page or maximizing the window doesn't pay to
    /// paint / shape / upload what isn't visible. The default viewport is
    /// "infinite" (headless), where nothing is ever culled; overlays draw
    /// on-screen, so they're never culled either.
    pub(super) fn culled(&self, r: Rectangle) -> bool {
        let vp = self.viewport;
        let r = self.placed(r);
        r.loc.y + r.size.h < -CULL_MARGIN
            || r.loc.y > vp.h + CULL_MARGIN
            || r.loc.x + r.size.w < -CULL_MARGIN
            || r.loc.x > vp.w + CULL_MARGIN
    }
}
