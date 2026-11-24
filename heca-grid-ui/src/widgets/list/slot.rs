//! The frame round each item a [`List`](super::List) builds, which reports its height so the list's
//! estimate follows what is really on screen.

use std::cell::Cell;
use std::rc::Rc;

use crate::component::{Base, Component};
use crate::style::Direction;

/// Heights measured this layout: their sum and how many.
pub(super) type Measured = Rc<Cell<(f64, usize)>>;

/// **One built item** — what [`render_item`](super::List::render_item) returned, in the row the list
/// selects. After every layout it adds its height to the list's measure.
pub(super) struct Slot {
    base: Base,
    measured: Measured,
}

impl Slot {
    pub(super) fn new(row: impl crate::builders::IntoComponent, measured: Measured) -> Self {
        let mut base = Base::container();
        base.style.layout.direction = Direction::Column;
        base.style.layout.flex_shrink = Some(0.0);
        base.children.push(row.into_component());
        Self { base, measured }
    }
}

impl Component for Slot {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn on_layout(&mut self) {
        let h = self.base.bounds.size.h;
        if h > 0.0 {
            let (sum, count) = self.measured.get();
            self.measured.set((sum + h, count + 1));
        }
    }
}
