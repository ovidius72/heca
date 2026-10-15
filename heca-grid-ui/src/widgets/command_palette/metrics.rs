//! Existing panel size variants and spacing.

use super::WidgetSize;

/// Narrowest the panel is allowed to be — and it yields to a window narrower than itself.
pub(super) const PANEL_MIN_W: f64 = 360.0;
/// The **most** of the window a panel may ever take, so it never touches the edges — the guard that
/// makes a small screen safe. Applied after the size variant's cap, and it wins.
pub(super) const PANEL_VIEWPORT_FRAC: f64 = 0.92;
/// Panel top offset as a fraction of the viewport height (palettes sit high).
pub(super) const TOP_FRAC: f64 = 0.12;
/// Room kept clear **below** the panel, as a fraction of the viewport height. `TOP_FRAC` is an
/// offset, not a margin: it guards the top and says nothing about the bottom, which is how the list
/// came to fill the window down to its last pixel and the final row ended up flush against the
/// screen edge.
///
/// Deliberately **wider than the gap [`PANEL_VIEWPORT_FRAC`] leaves at the sides** (4%). Matching
/// the sides was tried first and still read as touching: the panel's bottom edge landed on the top
/// of the bottom pane, so the gap disappeared into a boundary that was already there instead of
/// separating the panel from it.
pub(super) const BOTTOM_FRAC: f64 = 0.08;
/// Panel inner padding.
pub(super) const PAD: f64 = 12.0;
/// Vertical padding inside the query line and each result row.
pub(super) const QUERY_PAD_Y: f64 = 9.0;
pub(super) const ROW_PAD_Y: f64 = 6.0;
/// Horizontal inset of row content.
pub(super) const ROW_PAD_X: f64 = 10.0;
/// Gap between an icon and the label.
pub(super) const ICON_GAP: f64 = 10.0;
/// Command icon size as a fraction of the row font — a touch larger than the text, so the glyph
/// reads as the row's subject rather than as punctuation beside it.
pub(super) const ICON_FONT_MUL: f32 = 1.3;
/// Gap between two keycap chips of one chord.
pub(super) const CAP_GAP: f64 = 3.0;
/// Minimum gap between a label and the reserved shortcut column.
pub(super) const SHORTCUT_GAP: f64 = 16.0;
/// Keycap font as a fraction of the row font — a chip reads as an annotation, not as content.
pub(super) const KEYCAP_FONT_MUL: f32 = 0.85;
/// What a [`WidgetSize`] means for the panel: how wide it may get, and how many rows it shows.
///
/// The caller picks a **semantic** size (a `[settings] command_palette_size` of `small` / `normal` /
/// `large`); the widget owns the pixels, per the styling contract. Deliberately **width and row
/// count only** — the text stays at reading size at every variant, which is why this is a small
/// table here rather than `Style::size`, whose font scaling would cascade to the whole panel.
pub(super) const fn panel_metrics(size: WidgetSize) -> (f64, usize) {
    match size {
        WidgetSize::Caption | WidgetSize::Small => (560.0, 6),
        WidgetSize::Normal => (700.0, 8),
        WidgetSize::Large | WidgetSize::Header => (1000.0, 12),
    }
}
