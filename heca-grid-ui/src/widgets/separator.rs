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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
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
    /// An explicit span along the line, or `None` to stretch to the container.
    ///
    /// Kept as *intent* rather than written straight into the layout, so
    /// [`orientation`](Separator::orientation) and [`length`](Separator::length) can be set in
    /// either order: both recompute both axes from this pair. A `length` written directly onto
    /// `width` would land on the wrong axis if the orientation were set afterwards, and a property
    /// that has to come second is the one thing `#[prop]` refuses to express.
    length: Option<f32>,
}

#[heca_grid_ui_macros::props]
impl Separator {
    fn with(orientation: Orientation) -> Self {
        let mut separator = Self {
            base: Base::new(),
            orientation,
            length: None,
        };
        separator.apply_axes();
        separator
    }

    /// A horizontal rule (spans the container width).
    pub fn horizontal() -> Self {
        Self::with(Orientation::Horizontal)
    }

    /// A vertical rule (spans the container height).
    pub fn vertical() -> Self {
        Self::with(Orientation::Vertical)
    }

    /// Which way the line runs (default [`Horizontal`](Orientation::Horizontal)) — the builder
    /// behind [`horizontal`](Self::horizontal) / [`vertical`](Self::vertical), so a described
    /// separator can choose without the vocabulary needing two kinds.
    #[heca_grid_ui_macros::prop]
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self.apply_axes();
        self
    }

    /// Force an explicit length along the spanning axis (px) instead of relying
    /// on the parent's cross-axis stretch.
    #[heca_grid_ui_macros::prop]
    pub fn length(mut self, len: f32) -> Self {
        self.length = Some(len);
        self.apply_axes();
        self
    }

    /// Write both axes from orientation + length: [`THICKNESS`] across the line, the requested span
    /// (or `Auto`, which the parent's cross-axis stretch fills) along it. Both axes every time, so
    /// flipping the orientation clears the thickness the other axis was carrying.
    fn apply_axes(&mut self) {
        let span = match self.length {
            Some(len) => Length::Px(len),
            None => Length::Auto,
        };
        let (width, height) = match self.orientation {
            Orientation::Horizontal => (span, Length::Px(THICKNESS)),
            Orientation::Vertical => (Length::Px(THICKNESS), span),
        };
        self.base.style.layout.width = width;
        self.base.style.layout.height = height;
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
