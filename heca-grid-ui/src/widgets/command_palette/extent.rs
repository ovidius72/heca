//! What a [`WidgetSize`] means for the palette's panel: how wide it is and how many rows it shows.
//! The panel's own size — the layout engine does the rest.

use crate::font::MONO_LINE_RATIO;
use crate::style::{Length, Spacing, WidgetSize};

/// How much of the window's width the panel may take, whatever the variant — it never touches the
/// edges. A share of what the overlay leaves, so a small screen is safe by construction.
pub(super) const MAX_WIDTH_SHARE: f32 = 0.92;
/// How far down the window the panel starts, and how much of its height is kept clear beneath it:
/// palettes sit high, and the room under the list is wider than the gap at its sides, because a
/// bottom edge resting on a pane boundary reads as touching it.
pub(super) const TOP_SHARE: f32 = 0.12;
pub(super) const BOTTOM_SHARE: f32 = 0.08;

/// The panel's size for one variant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Extent {
    /// How wide the panel is, before the window takes any of it away.
    pub(super) width: Length,
    /// How tall the list may grow before it scrolls: the variant's row count, in rows of the
    /// panel's own font.
    pub(super) list_height: Length,
}

/// A row of a described list, in `em`: two text lines and the row's own padding. The unit a row
/// count is turned into a height with — the list scrolls past that many such rows.
fn row_em() -> f32 {
    2.0 * MONO_LINE_RATIO + 2.0 * Spacing::Sm.scale()
}

/// `Small` / `Normal` / `Large` decide the panel's width and how many rows it shows, and nothing
/// else — the text stays at reading size at every variant, which is why this is its own property
/// and not [`LayoutExt::size`](crate::builders::LayoutExt::size), whose font scaling would cascade
/// through the whole panel. `Caption` reads as `Small` and `Header` as `Large`.
pub(super) fn extent(size: WidgetSize) -> Extent {
    let (width, rows) = match size {
        WidgetSize::Caption | WidgetSize::Small => (560.0, 6.0),
        WidgetSize::Normal => (700.0, 8.0),
        WidgetSize::Large | WidgetSize::Header => (1000.0, 12.0),
    };
    Extent {
        width: Length::Px(width),
        list_height: Length::Em(rows * row_em()),
    }
}
