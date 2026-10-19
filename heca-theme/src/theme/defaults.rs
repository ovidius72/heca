//! Serde default helpers backing the `#[serde(default = …)]` fields of
//! [`Theme`](super::Theme), so a theme TOML can omit any of them.

use crate::color::Color;

pub(super) fn default_surface() -> Color {
    Color::rgb(12, 18, 24)
}
pub(super) fn default_muted() -> Color {
    Color::rgb(96, 130, 146)
}
pub(super) fn default_glow_color() -> Color {
    Color::rgb(64, 224, 255)
}
pub(super) fn default_danger() -> Color {
    Color::rgb(255, 70, 84)
}
pub(super) fn default_success() -> Color {
    Color::rgb(80, 255, 170)
}
pub(super) fn default_warning() -> Color {
    Color::rgb(255, 190, 70)
}
pub(super) fn default_pane_padding() -> f32 {
    4.0
}
pub(super) fn default_true() -> bool {
    true
}
pub(super) fn default_icon_secondary_alpha() -> f32 {
    // Kept low so the duotone secondary layer stays a subtle wash — a higher value makes
    // solid/filled glyphs (Stop, Circle, filled Play) collapse into a flat, muddy blob.
    0.30
}
pub(super) fn default_active_wash_alpha() -> f32 {
    0.11
}
pub(super) fn default_drag_edge_width() -> f32 {
    3.0
}
pub(super) fn default_drag_image_alpha() -> f32 {
    0.6
}
pub(super) fn default_drag_image_scale() -> f32 {
    0.5
}
pub(super) fn default_active_glow_radius() -> f32 {
    10.0
}
pub(super) fn default_active_glow_strength() -> f32 {
    0.55
}
pub(super) fn default_card_background_alpha() -> f32 {
    0.02
}
/// Default frosted-backdrop radius: `24` logical px — strong enough that text behind a full-screen
/// overlay becomes texture rather than words, gentle enough that the shapes of the session stay
/// recognisable underneath it.
pub(super) fn default_overlay_frost_radius() -> f32 {
    24.0
}
pub(super) fn default_float_bg() -> Color {
    Color::new(49, 50, 68, 255)
}
pub(super) fn default_float_accent() -> Color {
    Color::new(137, 180, 250, 255)
}
pub(super) fn default_float_focus() -> Color {
    Color::new(250, 179, 135, 255)
}
pub(super) fn default_drag_ghost_bg() -> Color {
    Color::new(137, 180, 250, 217)
}
pub(super) fn default_drag_ghost_fg() -> Color {
    Color::new(255, 255, 255, 255)
}
pub(super) fn default_drag_source_bg() -> Color {
    Color::new(137, 180, 250, 38)
}
pub(super) fn default_drag_source_border() -> Color {
    Color::new(137, 180, 250, 255)
}
pub(super) fn default_drop_target_bg() -> Color {
    Color::new(137, 180, 250, 45)
}
pub(super) fn default_drop_target_border() -> Color {
    Color::new(137, 180, 250, 255)
}
pub(super) fn default_drop_insertion() -> Color {
    Color::new(137, 180, 250, 255)
}
pub(super) fn default_sidebar_label_font_size() -> f32 {
    14.0
}
pub(super) fn default_sidebar_button_font_size() -> f32 {
    11.0
}
