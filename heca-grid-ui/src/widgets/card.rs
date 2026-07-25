//! [`Card`] — a surface preset: a padded column with a title header, ready for
//! body content via `.child(...)`. Demonstrates composing a reusable component
//! from [`Surface`](super::Surface) conventions + [`Label`](super::Label).

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::SignalGet;
use crate::scene::Border;
use crate::style::Direction;
use crate::widgets::Label;

/// Title text multiplier relative to the inherited base font (small header).
const TITLE_SCALE: f32 = 0.85;

/// A titled, padded surface card.
pub struct Card {
    base: Base,
}

impl Card {
    /// A card with a `title` header label. Add body content with `.child(...)`.
    pub fn new(title: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.layout.direction = Direction::Column;
        base.style.layout.padding = 18.0;
        base.style.layout.gap = 10.0;
        base.children
            .push(Box::new(Label::new(title).font_scale(TITLE_SCALE)));
        Self { base }
    }
}

impl Component for Card {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Paint the surface chrome with the **theme** corner radius + border width
    /// (so the global radius/border settings reach cards too), then children.
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let (radius, bw) = {
            let t = cx.theme();
            (t.colors.border_radius, t.colors.border_width)
        };
        let s = &self.base.style;
        // Keep the styled border color, but take its width from the theme.
        let border = s.visual.border.map(|b| Border {
            color: b.color,
            width: bw,
        });
        if s.visual.fill.is_some() || border.is_some() || s.visual.glow.is_some() {
            cx.rect(
                self.base.bounds,
                s.visual.fill.unwrap_or(Color::TRANSPARENT),
                border,
                radius,
                s.visual.glow,
            );
        }
        for child in &self.base.children {
            child.paint(cx);
        }
    }
}

impl LayoutExt for Card {}
impl StyleExt for Card {}
impl Parent for Card {}
