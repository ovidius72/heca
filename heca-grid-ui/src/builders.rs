//! Builder-method traits that encode the **layout vs. surface** separation.
//!
//! - [`LayoutExt`] — arrangement; for any container, including layout-only `Flex`.
//! - [`StyleExt`] — visual decoration; for **surface** components only (`Surface`,
//!   `Card`, `Button`). Deliberately *not* implemented for `Flex`, so a pure
//!   layout container cannot be turned into a styled surface.
//! - [`Parent`] — holds children.
//!
//! Default methods mutate `base_mut().style`, so a widget opts into a set of
//! builders just by writing `impl LayoutExt for MyWidget {}` (zero boilerplate).

use crate::color::Color;
use crate::component::Component;
use crate::reactive::SignalUpdate;
use crate::scene::{Border, Glow};
use crate::style::{Align, Direction, Justify, Length};

/// Arrangement builders: how a container lays out itself and its children.
pub trait LayoutExt: Component + Sized {
    /// Main-axis direction.
    fn direction(mut self, d: Direction) -> Self {
        self.base_mut().style.direction = d;
        self
    }
    /// Gap between children.
    fn gap(mut self, v: f32) -> Self {
        self.base_mut().style.gap = v;
        self
    }
    /// Main-axis distribution.
    fn justify(mut self, j: Justify) -> Self {
        self.base_mut().style.justify = j;
        self
    }
    /// Cross-axis alignment.
    fn align(mut self, a: Align) -> Self {
        self.base_mut().style.align = a;
        self
    }
    /// Inner padding on all sides.
    fn padding(mut self, p: f32) -> Self {
        self.base_mut().style.padding = p;
        self
    }
    /// Inner padding split per axis: `x` left+right, `y` top+bottom.
    fn padding_xy(mut self, x: f32, y: f32) -> Self {
        let s = &mut self.base_mut().style;
        s.padding_x = Some(x);
        s.padding_y = Some(y);
        self
    }
    /// Width along the main/cross axis.
    fn width(mut self, w: Length) -> Self {
        self.base_mut().style.width = w;
        self
    }
    /// Height along the main/cross axis.
    fn height(mut self, h: Length) -> Self {
        self.base_mut().style.height = h;
        self
    }
    /// Flex grow factor (share of remaining space).
    fn grow(mut self, g: f32) -> Self {
        self.base_mut().style.flex_grow = g;
        self
    }
    /// Disable the widget: dimmed, non-interactive, skipped by focus traversal.
    fn disabled(mut self, disabled: bool) -> Self {
        self.base_mut().disabled.set(disabled);
        self
    }
    /// Explicit Tab-order index (like HTML `tabindex`): indexed widgets are
    /// visited first in ascending order, then unindexed ones in tree position.
    fn tab_index(mut self, index: i32) -> Self {
        self.base_mut().tab_index = Some(index);
        self
    }
}

/// Visual decoration builders — **surfaces only**.
pub trait StyleExt: Component + Sized {
    /// Background fill.
    fn background(mut self, c: Color) -> Self {
        self.base_mut().style.fill = Some(c);
        self
    }
    /// Border outline.
    fn border(mut self, c: Color, width: f32) -> Self {
        self.base_mut().style.border = Some(Border { color: c, width });
        self
    }
    /// Neon outer glow (default radius/intensity).
    fn glow(mut self, c: Color) -> Self {
        self.base_mut().style.glow = Some(Glow {
            color: c,
            radius: 8.0,
            intensity: 1.0,
        });
        self
    }
    /// Neon outer glow with explicit falloff radius and intensity.
    fn glow_with(mut self, c: Color, radius: f32, intensity: f32) -> Self {
        self.base_mut().style.glow = Some(Glow {
            color: c,
            radius,
            intensity,
        });
        self
    }
    /// Corner radius.
    fn radius(mut self, r: f32) -> Self {
        self.base_mut().style.radius = r;
        self
    }

    /// Semantic font multiplier relative to the inherited base font (e.g. `2.0`
    /// for a header, `0.8` for a caption). Scales with a global font change.
    fn font_scale(mut self, scale: f32) -> Self {
        self.base_mut().style.font_scale = scale;
        self
    }
}

/// Components that contain children.
pub trait Parent: Component + Sized {
    /// Append a child component.
    fn child(mut self, c: impl Component + 'static) -> Self {
        self.base_mut().children.push(Box::new(c));
        self
    }
}
