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
        base.style.direction = direction;
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
}

impl LayoutExt for Flex {}
impl Parent for Flex {}

/// A vertical container — a [`Flex`] column. Convenience alias for readability.
pub type Container = Flex;

/// Construct a vertical container.
pub fn container() -> Flex {
    Flex::column()
}
