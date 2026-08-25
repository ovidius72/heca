//! [`Flex`] — the **layout-only** flexible box.
//!
//! `Flex` arranges children (direction, justify, align, gap, padding, grow) but
//! carries **no visual styling** — there is no `.background()`/`.border()`/
//! `.glow()` on it. Decoration lives on surface components
//! ([`Surface`](super::Surface), [`Card`](super::Card), [`Button`](super::Button)).
//!
//! Use `Flex` to *group and position*; reach for a surface when you want
//! something actually drawn. Builders come from [`LayoutExt`] and [`Parent`].

use crate::builders::{LayoutExt, Parent};
use crate::component::{Base, Component};
use crate::style::{Direction, Length};

/// A layout-only flexible container of child components.
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

    /// A zero-size placeholder — an empty slot that takes no layout space. Used
    /// to fill an optional slot (e.g. an [`Item`](super::Item) leading/trailing
    /// slot, or a [`DockFrame`](super::DockFrame) header-controls slot) until a
    /// real component replaces it.
    pub fn empty() -> Self {
        Self::row().width(Length::Px(0.0)).height(Length::Px(0.0))
    }

    fn with_direction(direction: Direction) -> Self {
        let mut base = Base::new();
        base.style.layout.direction = direction;
        Self { base }
    }
}

impl Component for Flex {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// **Text runs on one line share a baseline.**
    ///
    /// A row of text at two different sizes cannot be lined up by any box alignment: the renderer
    /// centres each run's line box in the rect it is given and drops the baseline an ascent below
    /// that box's top, and *both* of those scale with the font size. Centre the boxes and the
    /// smaller run's baseline lands above the larger one's — it reads as a superscript. `Start`
    /// aligns their tops, `End` their bottoms; none of the four is what a reader means by "on the
    /// same line".
    ///
    /// So the row does it: measure where each text child's baseline falls inside its own box, and
    /// drop the shallower ones until they all meet the deepest. Nothing is added to the row's
    /// height — a child is only ever moved down into space its taller sibling already claimed.
    ///
    /// It lives here rather than at a call site because every row of text wants it and none of them
    /// should have to know it: a name beside a dimmed `(program)` suffix, a heading beside a count,
    /// a label beside a unit. The alternative is each caller forcing one font size on both runs,
    /// which is a design decision made to work around a layout defect.
    fn on_layout(&mut self) {
        if self.base.style.layout.direction != Direction::Row {
            return;
        }
        // Where a child's baseline sits, measured down from the top of its own box.
        let offset = |c: &Box<dyn Component>| -> Option<f64> {
            let b = c.base();
            // Only a leaf that measures its own text has a baseline to speak of; a container's is
            // whatever its children work out among themselves.
            c.measure_text()?;
            let line = (b.font * crate::font::MONO_LINE_RATIO) as f64;
            let ascent = (b.font * crate::font::MONO_LINE_RATIO * crate::font::BASELINE_RATIO) as f64;
            Some((b.bounds.size.h - line) / 2.0 + ascent)
        };
        let deepest = self
            .base
            .children
            .iter()
            .filter_map(|c| offset(c).map(|o| o + c.base().bounds.loc.y))
            .fold(f64::NEG_INFINITY, f64::max);
        if !deepest.is_finite() {
            return;
        }
        for child in self.base.children.iter_mut() {
            let Some(own) = offset(child) else { continue };
            let dy = deepest - (child.base().bounds.loc.y + own);
            if dy.abs() > 0.01 {
                crate::component::shift_subtree(child.as_mut(), 0.0, dy);
            }
        }
    }
}

impl LayoutExt for Flex {}
impl Parent for Flex {}

/// A vertical container — a [`Flex`] column. Convenience alias for readability.
pub type Container = Flex;

/// Construct a vertical container.
pub fn container() -> Flex {
    Flex::column()
}
