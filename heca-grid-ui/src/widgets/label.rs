//! [`Label`] — a single run of text bound to a reactive signal.
//!
//! # Weight, slant, decorations
//! The four text attributes split cleanly in two, and the split is the interesting part:
//!
//! - **`bold` / `italic` are FONT attributes** — the shaper picks the glyphs. Bold selects the
//!   embedded family's real bold face; italic is a **synthesized oblique** (a glyph shear), because
//!   the embedded family ships no italic face and asking for a real one would substitute a
//!   *proportional* fallback and break the monospace metrics the whole grid rests on. See
//!   [`font`](crate::font).
//! - **`underline` / `strikethrough` are DECORATIONS** — a line is not a glyph, it is a rect. The
//!   label draws them itself, in its own resolved color, like any other chrome. Nothing about them
//!   reaches the shaper, which is why the renderer needed no change to gain them.

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{TextAlign, TextStyle};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Decoration thickness as a fraction of the resolved font — so a rule under 10px text is hairline
/// and one under a 28px header is proportionate. Floored at 1px so it never vanishes.
const DECORATION_RATIO: f32 = 0.07;
/// How far **below the text's centre line** the underline sits, in font units — just clear of the
/// descenders of the embedded face.
const UNDERLINE_OFFSET_RATIO: f32 = 0.42;
/// How far **above the centre line** the strikethrough sits: a touch high, because a line through
/// the exact middle of the box reads as low against lowercase text (whose visual mass sits above the
/// box centre).
const STRIKE_OFFSET_RATIO: f32 = 0.06;

/// A text label. Its content is a [`Signal`], so updating it marks the label
/// dirty and triggers a repaint.
///
/// **Color is inherited when unset.** With no explicit [`color`](Label::color), the label paints
/// in the [content color](crate::component::PaintCx::with_content_color) published by an enclosing
/// control — which is how a `Label` composed inside a [`Button`](super::Button) tracks that
/// button's hover/disabled state without either widget knowing about the other. With no inherited
/// color either, it falls back to the theme foreground.
pub struct Label {
    base: Base,
    text: Signal<String>,
    seen_text: String,
    align: TextAlign,
    color: Option<Color>,
    /// Bold weight. A [`Signal`] because it can be **state-driven by an enclosing widget** — an
    /// [`Item`](super::Item) bolds its label while the row is active — and the inherited paint
    /// context carries only a color, not a weight. The parent flips this in its `tick`.
    bold: Signal<bool>,
    /// Slanted (a synthesized oblique — see the module docs). A [`Signal`] for the same reason as
    /// [`bold`](Self::bold): a parent can drive it from state without rebuilding the tree.
    italic: Signal<bool>,
    /// A rule under the text — drawn by this widget, not shaped.
    underline: Signal<bool>,
    /// A rule through the text — drawn by this widget, not shaped.
    strikethrough: Signal<bool>,
}

#[heca_grid_ui_macros::props]
impl Label {
    /// A label showing `text`.
    pub fn new(text: impl Into<String>) -> Self {
        let base = Base::new();
        let text = signal(text.into());
        let seen_text = text.get_untracked();
        let mut label = Self {
            base,
            text,
            seen_text,
            align: TextAlign::Start,
            color: None,
            bold: signal(false),
            italic: signal(false),
            underline: signal(false),
            strikethrough: signal(false),
        };
        label.remeasure();
        label
    }

    /// Text horizontal alignment.
    #[heca_grid_ui_macros::prop]
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Explicit text color. Unset ⇒ the enclosing control's
    /// [content color](crate::component::PaintCx::with_content_color), else the theme foreground.
    /// Setting it opts the label **out** of that inheritance.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Render the label with bold weight.
    #[heca_grid_ui_macros::prop]
    pub fn bold(self, bold: bool) -> Self {
        self.bold.set(bold);
        self
    }

    /// The bold-weight signal — set it to re-weight the label in place, without rebuilding the
    /// tree. An enclosing widget uses this to drive a state-dependent weight (e.g. an
    /// [`Item`](super::Item) bolding its label while active).
    pub fn bold_signal(&self) -> Signal<bool> {
        self.bold
    }

    /// Render the label slanted — a **synthesized oblique**, not a separate face (see the module
    /// docs): the glyphs are sheared, so the advances (and the monospace grid) are untouched.
    #[heca_grid_ui_macros::prop]
    pub fn italic(self, italic: bool) -> Self {
        self.italic.set(italic);
        self
    }

    /// The slant signal — flip it in place, no rebuild.
    pub fn italic_signal(&self) -> Signal<bool> {
        self.italic
    }

    /// Draw a rule **under** the text. A decoration, not a font attribute: the label paints it as a
    /// rect in its own resolved color, so it tints with the label (including a parent's inherited
    /// content color).
    #[heca_grid_ui_macros::prop]
    pub fn underline(self, underline: bool) -> Self {
        self.underline.set(underline);
        self
    }

    /// The underline signal — flip it in place, no rebuild (a link that underlines on hover).
    pub fn underline_signal(&self) -> Signal<bool> {
        self.underline
    }

    /// Draw a rule **through** the text (a struck-out / completed item).
    #[heca_grid_ui_macros::prop]
    pub fn strikethrough(self, strikethrough: bool) -> Self {
        self.strikethrough.set(strikethrough);
        self
    }

    /// The strikethrough signal — flip it in place, no rebuild.
    pub fn strikethrough_signal(&self) -> Signal<bool> {
        self.strikethrough
    }

    /// The rect of the **text run itself** inside the label's bounds — the decorations' reference.
    ///
    /// The label's box can be wider than its text (a stretched parent), and `align` decides where
    /// the run sits in it, so a decoration must follow the run rather than span the box. Width comes
    /// from the same monospace measure `remeasure` uses, which is what keeps rule and glyphs in
    /// agreement.
    fn run_rect(&self) -> Rectangle {
        let bounds = self.base.bounds;
        let font = self.base.font as f64;
        let chars = self.text.get_untracked().chars().count() as f64;
        let width = (chars * font * MONO_ADVANCE_RATIO as f64).min(bounds.size.w);
        let x = match self.align {
            TextAlign::Start => bounds.loc.x,
            TextAlign::Center => bounds.loc.x + (bounds.size.w - width) / 2.0,
            TextAlign::End => bounds.loc.x + bounds.size.w - width,
        };
        Rectangle::new(Point::new(x, bounds.loc.y), Size::new(width, bounds.size.h))
    }

    /// Explicit font size in logical px — overrides the inherited theme font.
    #[heca_grid_ui_macros::prop]
    pub fn font_size(mut self, size: f32) -> Self {
        self.base.style.visual.font_size = size;
        self.base.font = size;
        self.remeasure();
        self
    }

    /// Semantic font multiplier relative to the inherited base font (header ≈ 2.0,
    /// caption ≈ 0.8). Scales with a global font change.
    #[heca_grid_ui_macros::prop]
    pub fn font_scale(mut self, scale: f32) -> Self {
        self.base.style.visual.font_scale = scale;
        self
    }

    /// The reactive text signal, so callers can update the label live.
    pub fn text_signal(&self) -> Signal<String> {
        self.text
    }
}

impl Component for Label {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The label's text — a `Label` is the leaf that gives a composed subtree its
    /// [accessible name](Component::text_summary), so a control can render the text of content it
    /// does not own (a [`Select`](super::Select) trigger showing the chosen option).
    fn text_summary(&self) -> Option<String> {
        Some(self.text.get_untracked())
    }

    /// Naive monospace measure from the resolved font ([`Base::font`]).
    fn remeasure(&mut self) {
        let text = self.text.get_untracked();
        self.seen_text = text.clone();
        let chars = text.chars().count() as f32;
        let fs = self.base.font;
        self.base.style.layout.width = Length::Px(chars * fs * MONO_ADVANCE_RATIO);
        self.base.style.layout.height = Length::Px(fs * MONO_LINE_RATIO);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        cx.paint_base(&self.base);
        // Own color → the enclosing control's inherited content color → the theme foreground.
        // The middle step is what makes a composed label track its parent's state (a Button's
        // hover sweep / disabled fade) with no wiring between the two widgets.
        let color = self
            .color
            .or_else(|| cx.content_color())
            .unwrap_or_else(|| cx.theme().colors.foreground);
        cx.text(
            self.base.bounds,
            &self.text.get_untracked(),
            color,
            self.base.font,
            self.align,
            TextStyle::REGULAR
                .bold(self.bold.get_untracked())
                .italic(self.italic.get_untracked()),
        );

        // Decorations. Drawn here, in the same resolved color as the glyphs, over the *run* (not the
        // box — the run is where the text actually is; see `run_rect`). An empty label has a
        // zero-width run and so draws nothing.
        let underline = self.underline.get_untracked();
        let strikethrough = self.strikethrough.get_untracked();
        if !(underline || strikethrough) {
            return;
        }
        let run = self.run_rect();
        if run.size.w <= 0.0 {
            return;
        }
        let font = self.base.font;
        let thickness = (font * DECORATION_RATIO).max(1.0) as f64;
        let mid_y = run.loc.y + run.size.h / 2.0;
        let mut rule = |y: f64| {
            cx.rect(
                Rectangle::new(Point::new(run.loc.x, y), Size::new(run.size.w, thickness)),
                color,
                None,
                0.0,
                None,
            );
        };
        if underline {
            rule(mid_y + (font * UNDERLINE_OFFSET_RATIO) as f64);
        }
        if strikethrough {
            rule(mid_y - (font * STRIKE_OFFSET_RATIO) as f64 - thickness / 2.0);
        }
    }

    fn tick(&mut self, _dt: f32) -> bool {
        let next = self.text.get_untracked();
        if next != self.seen_text {
            self.seen_text = next;
            self.remeasure();
            self.base.mark_needs_paint();
        }
        false
    }
}

// Gives `Label` the shared builders — notably `size` (the size variant) and
// width/height — even though it has no children.
impl LayoutExt for Label {}
