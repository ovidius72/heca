//! [`Label`] — a single run of text bound to a reactive signal.

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::TextAlign;
use crate::style::Length;

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
}

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
        };
        label.remeasure();
        label
    }

    /// Text horizontal alignment.
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

    /// Explicit font size in logical px — overrides the inherited theme font.
    pub fn font_size(mut self, size: f32) -> Self {
        self.base.style.font_size = size;
        self.base.font = size;
        self.remeasure();
        self
    }

    /// Semantic font multiplier relative to the inherited base font (header ≈ 2.0,
    /// caption ≈ 0.8). Scales with a global font change.
    pub fn font_scale(mut self, scale: f32) -> Self {
        self.base.style.font_scale = scale;
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

    /// Naive monospace measure from the resolved font ([`Base::font`]).
    fn remeasure(&mut self) {
        let text = self.text.get_untracked();
        self.seen_text = text.clone();
        let chars = text.chars().count() as f32;
        let fs = self.base.font;
        self.base.style.width = Length::Px(chars * fs * MONO_ADVANCE_RATIO);
        self.base.style.height = Length::Px(fs * MONO_LINE_RATIO);
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
            self.bold.get_untracked(),
        );
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
