//! [`Pane`] — a generic container framed by **prominent flat corner brackets**
//! (no glow, no shadow). The shell for sidebars and panes: a dark surface with a
//! subtle border and accent corner angles; its children (e.g. [`Item`](super::Item)
//! rows in a sidebar) stack inside.
//!
//! Like [`Surface`](super::Surface) it is a styled, child-holding container
//! (`LayoutExt` + `StyleExt` + `Parent`), but it always draws the corner
//! brackets and defaults to a vertical (column) layout.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{paint_child, Base, Component, PaintCx};
use crate::reactive::SignalGet;
use crate::style::Direction;

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
        let radius = cx.theme().radius;
        let b = self.base.bounds;
        let fill = self.base.style.fill;

        // Background fill — rounded by theme radius.
        if let Some(f) = fill {
            cx.rect(b, f, None, radius, self.base.style.glow);
        }

        // Prominent flat corner-bracket frame (shared with DockFrame).
        cx.bracket_frame(b, fill);

        // Children (sidebar Items, pane content, …); skip any hidden (collapsed).
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
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
