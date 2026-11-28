//! **How roomy the panel is**, per `[settings] command_palette_size` — the definition of the
//! palette's size variants, and the only sizes the palette owns, in one place (the layout pass has
//! no theme to read them from, as no widget's size variants do). Everything else (padding, gaps, the row's air, where the panel sits) is the theme's and
//! the composed widgets', so there is no geometry here.

use super::WidgetSize;

/// The panel's width (logical px) and how many rows it shows before it scrolls, per size. The
/// overlay caps the panel at the viewport, so a small window is safe however wide this is.
pub(super) const fn panel_metrics(size: WidgetSize) -> (f64, usize) {
    match size {
        WidgetSize::Caption | WidgetSize::Small => (560.0, 9),
        WidgetSize::Normal => (700.0, 12),
        WidgetSize::Large | WidgetSize::Header => (1000.0, 16),
    }
}

/// **How wide a row's shortcuts may grow, in `em`** — a long chord on its line gives way before the
/// title loses its room.
pub(super) const SHORTCUTS_MAX_EM: f32 = 20.0;

/// **How long the panel takes to dissolve in and out** — quick, so it is there as you type.
pub(super) const PANEL_FADE_SECONDS: f32 = 0.1;

/// **The panel's greatest height, as a share of the room the overlay gives it** — with its top at
/// the overlay's top placement, it ends at about 60% of the window's height, whatever the size says
/// about rows; past that the list scrolls.
pub(super) const PANEL_HEIGHT_SHARE: f32 = 0.52;
