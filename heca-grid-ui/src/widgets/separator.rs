//! [`Separator`] — a thin divider line. A display widget: a 1px rect in the
//! theme's border color. A horizontal separator spans its container's width (a
//! column child); a vertical one spans its height (a row child) — it asks to be
//! stretched itself, so it spans whether or not the container stretches its
//! children. Use [`length`](Separator::length) to cut it shorter than that.

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

    /// Cut the line to an explicit length (px) instead of spanning the whole container.
    #[heca_grid_ui_macros::prop]
    pub fn length(mut self, len: f32) -> Self {
        self.length = Some(len);
        self.apply_axes();
        self
    }

    /// Write both axes from orientation + length: [`THICKNESS`] across the line, and along it
    /// either the requested span or the whole container. Both axes every time, so flipping the
    /// orientation clears the thickness the other axis was carrying.
    ///
    /// With no length the separator **asks to be stretched** (`align_self`) rather than leaving it
    /// to the container. A row or column that centres its children — which is the common case, and
    /// what every other row in the showcase does — would otherwise give an `Auto` cross-size
    /// nothing to fill, and the rule would be laid out one pixel by zero and simply not appear.
    /// This widget used to answer that by telling the caller to pass a `length`, which is a
    /// workaround at every call site for something the rule can say once about itself.
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
        // An explicit length is a definite size, so it wins on its own and the container's own
        // alignment places the line; asking to stretch as well would be noise.
        self.base.style.layout.align_self = match self.length {
            Some(_) => None,
            None => Some(crate::style::Align::Stretch),
        };
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
