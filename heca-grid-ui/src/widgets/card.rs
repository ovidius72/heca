//! [`Card`] — a surface preset: a padded column with a title header, ready for
//! body content via `.child(...)`. Demonstrates composing a reusable component
//! from [`Surface`](super::Surface) conventions + [`Label`](super::Label).

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{Base, Component};
use crate::style::Direction;
use crate::widgets::Label;

/// A titled, padded surface card.
pub struct Card {
    base: Base,
}

impl Card {
    /// A card with a `title` header label. Add body content with `.child(...)`.
    pub fn new(title: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Column;
        base.style.padding = 18.0;
        base.style.gap = 10.0;
        base.style.radius = 4.0;
        base.children
            .push(Box::new(Label::new(title).font_size(13.0)));
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
}

impl LayoutExt for Card {}
impl StyleExt for Card {}
impl Parent for Card {}
