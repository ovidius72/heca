//! [`Separator`] — a thin divider line. A display widget: a 1px rect in the
//! theme's border color. A horizontal separator spans its container's width (a
//! column child); a vertical one spans its height (a row child) — both rely on
//! the default cross-axis `Stretch`. Use [`length`](Separator::length) to force
//! an explicit size when the parent centers instead of stretching.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::SignalGet;
use crate::style::Length;

/// Line thickness (logical px).
const THICKNESS: f32 = 1.0;

/// Orientation of a [`Separator`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// A horizontal rule (spans width); place in a column.
    #[default]
    Horizontal,
    /// A vertical rule (spans height); place in a row.
    Vertical,
}

/// A thin divider line.
pub struct Separator {
    base: Base,
    orientation: Orientation,
}

#[heca_grid_ui_macros::props]
impl Separator {
    fn with(orientation: Orientation) -> Self {
        let mut base = Base::new();
        match orientation {
            Orientation::Horizontal => base.style.layout.height = Length::Px(THICKNESS),
            Orientation::Vertical => base.style.layout.width = Length::Px(THICKNESS),
        }
        Self { base, orientation }
    }

    /// A horizontal rule (spans the container width).
    pub fn horizontal() -> Self {
        Self::with(Orientation::Horizontal)
    }

    /// A vertical rule (spans the container height).
    pub fn vertical() -> Self {
        Self::with(Orientation::Vertical)
    }

    /// Force an explicit length along the spanning axis (px) instead of relying
    /// on the parent's cross-axis stretch.
    #[heca_grid_ui_macros::prop]
    pub fn length(mut self, len: f32) -> Self {
        match self.orientation {
            Orientation::Horizontal => self.base.style.layout.width = Length::Px(len),
            Orientation::Vertical => self.base.style.layout.height = Length::Px(len),
        }
        self
    }
}

impl Component for Separator {
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
        let color = cx.theme().colors.border;
        cx.rect(self.base.bounds, color, None, 0.0, None);
    }
}

impl LayoutExt for Separator {}
