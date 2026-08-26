//! [`ToastPosition`] — where a notification sits in the box that holds it.
//!
//! One vocabulary for both a single card that places itself and a
//! [`ToastStack`](super::ToastStack) anchoring a corner, so "top right" means the same thing said
//! either way.
//!
//! **It resolves to auto margins, never to pixels.** An auto margin is the layout engine's own way
//! of saying "push me to that edge" — it eats the free space on that side — so a positioned card
//! lands correctly in a container of any size, at any font, with nothing to keep in step.

use crate::style::Length;

/// Where a card or a stack anchors inside its container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum ToastPosition {
    /// The top-right corner — where notifications go unless told otherwise.
    #[default]
    TopRight,
    TopLeft,
    /// Centred horizontally, against the top edge.
    TopCenter,
    BottomRight,
    BottomLeft,
    /// Centred horizontally, against the bottom edge.
    BottomCenter,
}

#[heca_grid_ui_macros::props]
impl ToastPosition {
    /// Is this position against the right edge?
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub(crate) fn is_right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }

    /// Is this position against the top edge?
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub(crate) fn is_top(self) -> bool {
        matches!(self, Self::TopRight | Self::TopLeft | Self::TopCenter)
    }

    /// Is this position centred on the horizontal axis?
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub(crate) fn is_centered(self) -> bool {
        matches!(self, Self::TopCenter | Self::BottomCenter)
    }

    /// The four margins that put a box here: `(left, right, top, bottom)`, where `Auto` means
    /// "take the slack on this side".
    ///
    /// A centred position claims the slack on **both** horizontal sides, which is what centres it;
    /// an edge position claims it on the far side only, which is what pins it.
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub(crate) fn margins(self) -> (Option<Length>, Option<Length>, Option<Length>, Option<Length>) {
        let slack = Some(Length::Auto);
        let (left, right) = if self.is_centered() {
            (slack, slack)
        } else if self.is_right() {
            (slack, None)
        } else {
            (None, slack)
        };
        let (top, bottom) = if self.is_top() { (None, slack) } else { (slack, None) };
        (left, right, top, bottom)
    }
}
