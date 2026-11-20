//! One binding of a command, drawn as keycap chips — the leaf a palette row's shortcut column holds.
//!
//! A leaf, like [`Icon`](crate::widgets::Icon) or [`StatusDot`](crate::widgets::StatusDot): it sizes
//! itself from what it draws, so the layout engine can place it, and it draws through the shared
//! keycap primitive ([`KeyCap::paint`]) — the chip metric and look are the one the context menu and
//! the pick letters use, never re-derived here. Its colour is the **inherited content colour**, so
//! the row that holds it says "accent" or "muted" with a tone and this widget is told nothing.

use crate::component::{Base, Component, PaintCx};
use crate::font::MONO_LINE_RATIO;
use crate::reactive::SignalGet;
use crate::style::Length;
use crate::widgets::{KeyCap, KeycapVariant};
use heca_core::layout::{Point, Rectangle, Size};

/// Gap between two chips of one chord.
const CAP_GAP: f64 = 3.0;
/// Chip font as a fraction of the row font — a chip reads as an annotation, not as content.
const KEYCAP_FONT_MUL: f32 = 0.85;

/// A chord of keycap chips, one line high: `[λ] [⇧] [e]`.
pub(super) struct KeyChord {
    base: Base,
    caps: Vec<KeyCap>,
}

impl KeyChord {
    pub(super) fn new(caps: Vec<KeyCap>) -> Self {
        let mut base = Base::new();
        // **It never gives way**: squeezing a chip row does not make it smaller, it cuts a chip.
        base.style.layout.flex_shrink = Some(0.0);
        let mut chord = Self { base, caps };
        chord.remeasure();
        chord
    }

    /// The chip font for a row drawn at `font`.
    fn cap_font(font: f32) -> f32 {
        font * KEYCAP_FONT_MUL
    }

    /// How wide `caps` come out for a row at `font` — chips end to end with their gaps. The one
    /// measure: the widget sizes itself by it, and a list reserving **one column for every row's
    /// chords** asks it for each chord to find the widest.
    pub(super) fn width_of(caps: &[KeyCap], font: f32) -> f64 {
        let cap_font = Self::cap_font(font);
        let chips: f64 = caps.iter().map(|cap| cap.size(cap_font).w).sum();
        chips + CAP_GAP * caps.len().saturating_sub(1) as f64
    }
}

impl Component for KeyChord {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// One text line high, so stacked chords step down exactly as the row's text lines do, with the
    /// chips centred in their line.
    fn remeasure(&mut self) {
        let font = self.base.font;
        self.base.style.layout.width = Length::Px(Self::width_of(&self.caps, font) as f32);
        self.base.style.layout.height = Length::Px(font * MONO_LINE_RATIO);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let cap_font = Self::cap_font(self.base.font);
        let color = cx.content_color();
        let line = self.base.bounds;
        let mut x = line.loc.x;
        for cap in &self.caps {
            let size = cap.size(cap_font);
            let chip = Rectangle::new(
                Point::new(x, line.loc.y + (line.size.h - size.h) / 2.0),
                Size::new(size.w, size.h),
            );
            cap.paint(cx, chip, cap_font, color, KeycapVariant::Bordered);
            x += size.w + CAP_GAP;
        }
    }
}
