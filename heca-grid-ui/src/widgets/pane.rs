//! [`Pane`] — a generic container framed by **prominent flat corner brackets**
//! (no glow, no shadow). The shell for sidebars and panes: a dark surface with a
//! subtle border and accent corner angles; its children (e.g. [`Item`](super::Item)
//! rows in a sidebar) stack inside.
//!
//! Like [`Surface`](super::Surface) it is a styled, child-holding container
//! (`LayoutExt` + `StyleExt` + `Parent`), but it always draws the corner
//! brackets and defaults to a vertical (column) layout.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::SignalGet;
use crate::scene::Border;
use crate::style::Direction;
use heca_core::layout::{Point, Rectangle, Size};

/// Bright bracket length along each edge, measured from the corner (in addition
/// to the rounded arc). The straight midsection between the two brackets on an
/// edge is dimmed back to a subtle line.
const ARM_LEN: f32 = 12.0;

/// Bright corner brackets are drawn thicker than the subtle border for emphasis.
const BRACKET_WIDTH_MUL: f32 = 2.0;

/// Alpha of the fill-colored overlay used to dim the straight border midsections.
/// ~0.7 over the bright accent border leaves a ~30% accent line — matching the
/// subtle continuous border, while the corners stay fully bright.
const STRAIGHT_DIM: u8 = 178;

/// A bracket-framed container for sidebars / panes.
pub struct Pane {
    base: Base,
}

impl Pane {
    /// A new vertical (column) pane. Add content with `.child(...)`.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Column;
        Self { base }
    }

    /// A horizontal (row) pane.
    pub fn row() -> Self {
        let mut pane = Self::new();
        pane.base.style.direction = Direction::Row;
        pane
    }
}

impl Component for Pane {
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
        let (accent, background, radius, border_width) = {
            let t = cx.theme();
            (t.accent, t.background, t.radius, t.border_width)
        };
        let b = self.base.bounds;
        let fill = self.base.style.fill;

        // Background fill — rounded by theme radius.
        if let Some(f) = fill {
            cx.rect(b, f, None, radius, self.base.style.glow);
        }

        // Bright accent border tracing the full rounded perimeter. The renderer's
        // bracket primitive only draws square 90° corners, so instead of brackets
        // we draw a full rounded border (which has the radius) and then dim its
        // straight midsections — leaving the rounded corners + short arms bright.
        let bracket_width = border_width * BRACKET_WIDTH_MUL;
        cx.rect(
            b,
            Color::TRANSPARENT,
            Some(Border { color: accent, width: bracket_width }),
            radius,
            None,
        );

        // Dim the straight midsection of each edge back to a subtle ~30% line, so
        // only the rounded corners (plus an `ARM_LEN` arm) stay bright. The overlay
        // is the fill (or background) color at ~0.7 alpha, with no glow.
        let cover = fill.unwrap_or(background).with_alpha(STRAIGHT_DIM);
        let keep = f64::from(radius + ARM_LEN);
        let t = f64::from(bracket_width) + 1.0;
        let (x, y, w, h) = (b.loc.x, b.loc.y, b.size.w, b.size.h);

        let mid_w = w - 2.0 * keep;
        if mid_w > 0.0 {
            cx.rect(Rectangle::new(Point::new(x + keep, y), Size::new(mid_w, t)), cover, None, 0.0, None);
            cx.rect(Rectangle::new(Point::new(x + keep, y + h - t), Size::new(mid_w, t)), cover, None, 0.0, None);
        }
        let mid_h = h - 2.0 * keep;
        if mid_h > 0.0 {
            cx.rect(Rectangle::new(Point::new(x, y + keep), Size::new(t, mid_h)), cover, None, 0.0, None);
            cx.rect(Rectangle::new(Point::new(x + w - t, y + keep), Size::new(t, mid_h)), cover, None, 0.0, None);
        }

        // Children (sidebar Items, pane content, …).
        for child in &self.base.children {
            child.paint(cx);
        }
    }
}

impl Default for Pane {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Pane {}
impl StyleExt for Pane {}
impl Parent for Pane {}
