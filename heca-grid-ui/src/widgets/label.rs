//! [`Label`] — a single run of text bound to a reactive signal.

use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet};
use crate::scene::TextAlign;
use crate::style::Length;

/// A text label. Its content is a [`Signal`], so updating it marks the label
/// dirty and triggers a repaint.
pub struct Label {
    base: Base,
    text: Signal<String>,
    align: TextAlign,
    color: Option<Color>,
}

impl Label {
    /// A label showing `text`.
    pub fn new(text: impl Into<String>) -> Self {
        let base = Base::new();
        let text = signal(text.into());
        let mut label = Self {
            base,
            text,
            align: TextAlign::Start,
            color: None,
        };
        label.remeasure();
        label
    }

    /// Text horizontal alignment.
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Explicit text color (defaults to the theme foreground).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
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
        let chars = self.text.get_untracked().chars().count() as f32;
        let fs = self.base.font;
        self.base.style.width = Length::Px(chars * fs * MONO_ADVANCE_RATIO);
        self.base.style.height = Length::Px(fs * MONO_LINE_RATIO);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        cx.paint_base(&self.base);
        let color = self.color.unwrap_or_else(|| cx.theme().foreground);
        cx.text(
            self.base.bounds,
            &self.text.get_untracked(),
            color,
            self.base.font,
            self.align,
            false,
        );
    }
}
