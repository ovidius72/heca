//! [`Surface`] — a styled box: the proper place for visual decoration
//! (background, border, glow, radius), as opposed to the layout-only
//! [`Flex`](super::Flex).
//!
//! A surface also arranges its own children (it has padding/gap/direction via
//! [`LayoutExt`]), so it is both a decorated panel and a container.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{Base, Component};
use crate::style::Direction;

/// A decorated container.
pub struct Surface {
    base: Base,
}

impl Surface {
    /// A vertical surface (column).
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Column;
        Self { base }
    }

    /// A horizontal surface (row).
    pub fn row() -> Self {
        let mut s = Self::new();
        s.base.style.direction = Direction::Row;
        s
    }

    /// A vertical surface (column). Alias for [`Surface::new`].
    pub fn column() -> Self {
        Self::new()
    }
}

impl Default for Surface {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Surface {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
}

impl LayoutExt for Surface {}
impl StyleExt for Surface {}
impl Parent for Surface {}
