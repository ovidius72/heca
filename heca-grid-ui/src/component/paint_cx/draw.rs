//! **The basic draws**: surfaces, blurs, rectangles, text and icons.

use super::*;

impl<'a> PaintCx<'a> {
    /// Place a decoded image. The shared image service owns decoding, pixel density and caching.
    pub fn image(&mut self, rect: Rectangle, image: std::sync::Arc<crate::image::ImageData>, fit: crate::image::ImageFit, color: Color) {
        if self.culled(rect) { return }
        self.emit(DrawCommand::Host(HostCmd {
            draw: HostDraw::Image { image, fit, color }, rect: self.placed(rect), alpha: 1.0,
            echo: self.echo, rounded: self.rounded,
        }));
    }

    /// Draw a segment with the active translation, scale, opacity and clip.
    pub fn line(&mut self, mut line: crate::scene::LineCmd) {
        line.start = self.placed(Rectangle::new(line.start, Size::new(0.0, 0.0))).loc;
        line.end = self.placed(Rectangle::new(line.end, Size::new(0.0, 0.0))).loc;
        self.emit(DrawCommand::Line(line));
    }

    /// Queue a rounded rectangle with optional border and glow.
    /// **Place content something else rasterised** — a terminal, an image, a video, a plugin's own
    /// canvas — in `rect`.
    ///
    /// This widget says where; the host owns the texture and does the drawing. `id` is opaque here:
    /// nothing about textures, formats or devices crosses into this library. See
    /// [`HostDraw`].
    pub fn surface(&mut self, rect: Rectangle, id: u64) {
        if self.culled(rect) {
            return;
        }
        let rect = self.placed(rect);
        self.emit(DrawCommand::Host(HostCmd {
            draw: HostDraw::Surface { id },
            rect,
            alpha: 1.0,
            echo: self.echo,
            rounded: self.rounded,
        }));
    }

    /// **Blur whatever is already drawn behind this widget**, within `rect`.
    ///
    /// Recorded in scene order, so it blurs exactly what came before it and nothing of what comes
    /// after. `corner` rounds the blurred box to the corner radius of the widget that asks for it.
    /// `alpha` fades the blurred copy — a surface arriving fades its backdrop in with itself,
    /// rather than holding the session out of focus and snapping sharp in one frame at the end.
    pub fn backdrop_blur(&mut self, rect: Rectangle, radius: f32, corner: f32, alpha: f32) {
        if radius <= 0.0 || alpha <= 0.0 || self.culled(rect) {
            return;
        }
        let rect = self.placed(rect);
        self.emit(DrawCommand::Host(HostCmd {
            draw: HostDraw::Backdrop { radius, corner },
            rect,
            alpha,
            echo: self.echo,
            rounded: self.rounded,
        }));
    }

    pub fn rect(
        &mut self,
        rect: Rectangle,
        fill: Color,
        border: Option<Border>,
        radius: f32,
        glow: Option<Glow>,
    ) {
        if self.culled(rect) {
            return;
        }
        self.emit(DrawCommand::Rect(RectCmd {
            rect: self.placed(rect),
            fill,
            border,
            radius,
            glow: self.scaled_glow(glow),
            shadow: None,
        }));
    }

    /// Queue a soft **drop shadow** for `rect` (corner `radius`): a dark, blurred,
    /// offset halo drawn *behind* it that lifts the shape off the background. Call
    /// this **before** painting the shape's fill, so the shape occludes the
    /// shadow's center and only its fringe shows. Unlike [`glow`](PaintCx::rect),
    /// the shadow darkens (composites a dark color with alpha), so it reads on
    /// dark themes — and it's independent of the glow/border tokens, so it shows
    /// even when both are off (e.g. a floating Modal at `border_width == 0`).
    pub fn drop_shadow(&mut self, rect: Rectangle, radius: f32, shadow: Shadow) {
        if shadow.color.a == 0 || shadow.radius <= 0.0 {
            return;
        }
        self.emit(DrawCommand::Rect(RectCmd {
            rect: self.placed(rect),
            fill: Color::TRANSPARENT,
            border: None,
            radius,
            glow: None,
            shadow: Some(shadow),
        }));
    }

    /// Build a box border in `color` at the **theme's**
    /// [`border_width`](heca_theme::Theme::border_width), or `None` when borders are off
    /// (`border_width == 0`). Widgets should build their box border with this instead of hardcoding
    /// a stroke, so they all honor the token (and disappear together at width 0). The single
    /// chokepoint that keeps border width theme-driven across the widget set.
    pub fn border(&self, color: Color) -> Option<Border> {
        let w = self.theme.colors.border_width;
        (w > 0.0).then_some(Border { color, width: w })
    }

    /// Queue the **thin-outline focus indicator**: an accent-toned outline drawn *just outside*
    /// `rect` (a CSS-style `outline` with an offset gap), with a restrained halo. An alternative to
    /// the corner-bracket reticle ([`bracket_frame`](Self::bracket_frame)).
    ///
    /// This is **the** keyboard focus indicator for the whole widget set (Button, IconButton,
    /// Toggle, Checkbox, Input, Select, Tabs, Item, Row, RailCell, Toast, BadgeButton,
    /// ScrollRegion, …). Because it sits **outside** the widget box rather than on its edge, it is
    /// visible whether or not the widget draws its own border — borderless variants (e.g.
    /// Ghost/Link buttons) get the same clear ring — and it never merges into the widget's own
    /// border. It is purely drawn (it does not affect layout), so it can overlap neighbouring
    /// padding like a real focus outline. `radius` is the widget's own corner radius; the outline
    /// widens it by the offset to stay concentric. Width uses the
    /// [`focus_border_width`](crate::theme::Theme::focus_border_width) token (its own width, so it
    /// stays visible even when decorative borders are off), and the halo is scaled by `glow_size`
    /// inside [`rect`](Self::rect) — dropping to nothing when glow is `none`. The glow is kept low
    /// so it reads as a focus cue, not an alarm. Pair with the theme's
    /// [`effective_focus_ring`](heca_theme::Theme::effective_focus_ring) /
    /// [`focus_ring_tone`](heca_theme::Theme::focus_ring_tone) for the color. (The decorative
    /// corner-bracket reticle is a different primitive — [`bracket_frame`](Self::bracket_frame).)
    pub fn focus_ring(&mut self, rect: Rectangle, color: Color, radius: f32) {
        // Offset gap (logical px) between the widget edge and the outline — like CSS `outline-offset`.
        const OFFSET: f32 = 2.0;
        let o = OFFSET as f64;
        let outset = Rectangle::new(
            Point::new(rect.loc.x - o, rect.loc.y - o),
            Size::new(rect.size.w + 2.0 * o, rect.size.h + 2.0 * o),
        );
        let border = Border {
            color,
            width: self.theme.focus_border_width,
        };
        // `rect` runs the glow through `scaled_glow`, so this halo tracks `glow_size`
        // (and vanishes at `none`) exactly like every other glow in the library.
        let glow = Some(Glow {
            color,
            radius: 3.0,
            intensity: 0.3,
        });
        self.rect(
            outset,
            Color::TRANSPARENT,
            Some(border),
            radius + OFFSET,
            glow,
        );
    }

    /// Queue a text run within `rect` at an explicit logical `size`. The renderer
    /// centers the text within `rect` (per `align` horizontally, always centered
    /// vertically) using real glyph metrics.
    ///
    /// `style` is the **font** style ([`TextStyle`]: weight + slant). Decorations — underline,
    /// strikethrough — are not shaped: a line is a rect, and the widget draws it itself from the
    /// theme (see [`Label`](crate::widgets::Label)).
    pub fn text(
        &mut self,
        rect: Rectangle,
        text: &str,
        color: Color,
        size: f32,
        align: TextAlign,
        style: TextStyle,
    ) {
        if self.culled(rect) {
            return;
        }
        self.emit(DrawCommand::Text(TextCmd {
            rect: self.placed(rect),
            text: text.to_string(),
            color,
            size,
            align,
            style,
            font: FontRole::Text,
            glow: None,
        }));
    }

    /// Queue a single icon glyph centered in `rect`, shaped with the icon font
    /// ([`FontRole::Icon`]). `glyph` is the codepoint as a string; the renderer
    /// selects the embedded icon family. Used by [`Icon`](crate::widgets::Icon).
    pub fn icon(&mut self, rect: Rectangle, glyph: &str, color: Color, size: f32) {
        self.icon_glowing(rect, glyph, color, size, None);
    }

    /// Draw a **Nerd Font** glyph — the counterpart of [`icon`](PaintCx::icon) for the app's second
    /// glyph set ([`NfIcon`](crate::widgets::NfIcon)), which supplies what Phosphor has none of:
    /// keyboard keys. `glyph` is the codepoint as a string.
    pub fn nf_icon(&mut self, rect: Rectangle, glyph: &str, color: Color, size: f32) {
        if self.culled(rect) {
            return;
        }
        self.emit(DrawCommand::Text(TextCmd {
            rect: self.placed(rect),
            text: glyph.to_string(),
            color,
            size,
            align: TextAlign::Center,
            style: TextStyle::REGULAR,
            font: FontRole::NerdFont,
            glow: None,
        }));
    }

    /// [`icon`](PaintCx::icon) with an additive halo behind the glyph.
    ///
    /// This is the glyph counterpart of a surface's glow, and it goes through the
    /// **same chokepoint**: `glow` is scaled by the theme's `glow_size` token before
    /// it reaches the scene, so `GlowLevel::None` drops the halo entirely and every
    /// other level scales it exactly as it scales a rect's. Pass
    /// [`rest_glow`](PaintCx::rest_glow) to give a bare glyph the same faint resting
    /// halo that bordered surfaces carry.
    ///
    /// A halo is deliberately opt-in per call rather than a property of the icon
    /// font: terminal cell glyphs are the hottest path in the app and must never
    /// take it.
    pub fn icon_glowing(
        &mut self,
        rect: Rectangle,
        glyph: &str,
        color: Color,
        size: f32,
        glow: Option<Glow>,
    ) {
        if self.culled(rect) {
            return;
        }
        let glow = self.scaled_glow(glow);
        self.emit(DrawCommand::Text(TextCmd {
            rect: self.placed(rect),
            text: glyph.to_string(),
            color,
            size,
            align: TextAlign::Center,
            style: TextStyle::REGULAR,
            font: FontRole::Icon,
            glow,
        }));
    }
}
