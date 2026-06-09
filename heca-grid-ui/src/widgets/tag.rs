//! [`Tag`] — a small **chip**: an optional leading slot (e.g. an
//! [`Icon`](super::Icon)) plus a label, in a rounded, subtly-bordered pill.
//!
//! Where [`Badge`](super::Badge) is a solid status token (`RUN`, `M`, `OFFLINE`),
//! `Tag` is the quieter metadata chip that can **carry an icon** — a git branch
//! (`⎇ main`), a label, a filter. It is a display widget: it lays out its slot +
//! label and paints the pill chrome; the slots draw themselves, so it stays
//! composable and domain-neutral. The pill hue is configurable via
//! [`color`](Tag::color) (default: the theme's muted token).

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::SignalGet;
use crate::scene::Border;
use crate::style::{Align, Direction};
use crate::widgets::Label;

/// Horizontal inner padding — generous, so the chip has breathing room.
const PAD_X: f32 = 12.0;
/// Vertical inner padding — tight, to keep the pill slim.
const PAD_Y: f32 = 4.0;
/// Gap between the leading slot and the label.
const GAP: f32 = 6.0;
/// Chip text size relative to the base font.
const FONT_SCALE: f32 = 0.8;
/// Translucent fill alpha for the chip background.
const FILL_ALPHA: u8 = 22;
/// Border alpha — a quiet edge in the chip hue.
const BORDER_ALPHA: u8 = 130;

/// A small labeled chip with an optional leading slot.
pub struct Tag {
    base: Base,
    /// Pill hue for the fill + border (default: theme muted).
    color: Option<Color>,
}

impl Tag {
    /// A new chip showing `label`. Add a leading icon with [`leading`](Tag::leading).
    pub fn new(label: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Row;
        base.style.align = Align::Center;
        base.style.padding_x = Some(PAD_X);
        base.style.padding_y = Some(PAD_Y);
        base.style.gap = GAP;
        base.children.push(Box::new(Label::new(label).font_scale(FONT_SCALE)));
        Self { base, color: None }
    }

    /// Set the leading slot — any component (typically an [`Icon`](super::Icon)).
    pub fn leading(mut self, c: impl Component + 'static) -> Self {
        self.base.children.insert(0, Box::new(c));
        self
    }

    /// Pill hue for the fill + border (default: the theme's muted token).
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
}

impl Component for Tag {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let c = self.color.unwrap_or_else(|| cx.theme().muted);
        let pill = self.base.bounds;
        // Capsule: round to half the height.
        let radius = (pill.size.h / 2.0) as f32;
        cx.rect(
            pill,
            c.with_alpha(FILL_ALPHA),
            Some(Border { color: c.with_alpha(BORDER_ALPHA), width: 1.0 }),
            radius,
            None,
        );
        // Leading slot + label draw themselves.
        for child in &self.base.children {
            child.paint(cx);
        }
    }
}

impl LayoutExt for Tag {}
