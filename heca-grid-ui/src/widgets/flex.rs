//! [`Flex`] — the flexible box every layout is built from.
//!
//! Answers the "lay out, align, keep positioned" need: `justify` distributes on
//! the main axis, `align` on the cross axis, `gap`/`padding` set rhythm, and
//! `flex_grow` makes a child absorb remaining space. Built on `taffy`.

use crate::color::Color;
use crate::component::{Base, Component};
use crate::scene::{Border, Glow};
use crate::style::{Align, Direction, Justify, Length};

/// A flexible container of child components.
pub struct Flex {
    base: Base,
}

impl Flex {
    /// A row (horizontal main axis).
    pub fn row() -> Self {
        Self::with_direction(Direction::Row)
    }

    /// A column (vertical main axis).
    pub fn column() -> Self {
        Self::with_direction(Direction::Column)
    }

    fn with_direction(direction: Direction) -> Self {
        let mut base = Base::new();
        base.style.direction = direction;
        Self { base }
    }

    // ── Layout builders ──

    /// Space between children, in logical pixels.
    pub fn gap(mut self, gap: f32) -> Self {
        self.base.style.gap = gap;
        self
    }

    /// Main-axis distribution.
    pub fn justify(mut self, justify: Justify) -> Self {
        self.base.style.justify = justify;
        self
    }

    /// Cross-axis alignment.
    pub fn align(mut self, align: Align) -> Self {
        self.base.style.align = align;
        self
    }

    /// Inner padding on all sides, in logical pixels.
    pub fn padding(mut self, padding: f32) -> Self {
        self.base.style.padding = padding;
        self
    }

    /// Fixed or relative width.
    pub fn width(mut self, width: Length) -> Self {
        self.base.style.width = width;
        self
    }

    /// Fixed or relative height.
    pub fn height(mut self, height: Length) -> Self {
        self.base.style.height = height;
        self
    }

    /// Flex grow factor (share of remaining main-axis space).
    pub fn grow(mut self, grow: f32) -> Self {
        self.base.style.flex_grow = grow;
        self
    }

    // ── Visual builders ──

    /// Background fill.
    pub fn background(mut self, color: Color) -> Self {
        self.base.style.fill = Some(color);
        self
    }

    /// Border outline.
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.base.style.border = Some(Border { color, width });
        self
    }

    /// Neon outer glow.
    pub fn glow(mut self, color: Color) -> Self {
        self.base.style.glow = Some(Glow {
            color,
            radius: 8.0,
            intensity: 1.0,
        });
        self
    }

    /// Corner radius, in logical pixels.
    pub fn radius(mut self, radius: f32) -> Self {
        self.base.style.radius = radius;
        self
    }

    // ── Composition ──

    /// Add a child component.
    pub fn child(mut self, child: impl Component + 'static) -> Self {
        self.base.children.push(Box::new(child));
        self
    }
}

impl Component for Flex {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
}

/// A vertical container — a [`Flex`] column. Convenience alias for readability.
pub type Container = Flex;

/// Construct a vertical container.
pub fn container() -> Flex {
    Flex::column()
}
