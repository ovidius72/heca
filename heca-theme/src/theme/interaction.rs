//! Interaction-state alpha tokens grouped on [`Theme::interaction`](super::Theme::interaction).

use serde::{Deserialize, Serialize};

/// Interaction-state alpha tokens — raw `0..=255` alpha bytes a widget lays over a
/// base hue (accent / foreground / danger …) for its hover, active/selected, border,
/// tonal-fill, scrim and overlay states. Grouped on
/// [`Theme::interaction`](super::Theme::interaction) so the interaction feel is a
/// single theme-tuned surface instead of scattered per-widget `const … _ALPHA`
/// values. Values default to the historical per-widget constants; where several
/// widgets shared a role the value is unified (see field docs).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InteractionAlphas {
    // ── Controls (buttons / toggles / inputs / selects) ──
    /// Toggle track fill at full-on (~50% accent wash).
    pub toggle_on_fill: u8,
    /// Hover fill over a control's tone (icon button).
    pub control_hover_fill: u8,
    /// Hover border over a control's tone (icon button).
    pub control_hover_border: u8,
    /// Held-on (toggled) fill (icon button).
    pub control_active_fill: u8,
    /// Held-on (toggled) border (icon button).
    pub control_active_border: u8,
    /// Resting border of a control — unifies button/input/toggle/checkbox/select (was `150` in each).
    pub control_rest_border: u8,
    /// Resting **glow intensity** of a control's surface (×255 — a `Glow.intensity`
    /// of `30` ≈ 0.12), so buttons/inputs/toggles/selects carry a faint neon halo at
    /// rest and the `glow_size` setting visibly scales them without hover/focus.
    /// `0` = flat rest look (glow only on hover/active), matching the pre-token
    /// behaviour. Scaled — like every glow — by `glow_size` at the
    /// `PaintCx::scaled_glow` chokepoint.
    pub control_rest_glow: u8,

    // ── List rows / sidebar cells ──
    /// Hover fill of a list row / sidebar cell — unifies row/item/rail (16/16/18 → 16).
    pub row_hover_fill: u8,
    /// Selected fill of a list row / sidebar cell — unifies row/item/rail (30/30/34 → 30).
    pub row_active_fill: u8,
    /// Selected border of a list row / sidebar cell — unifies row/rail (180/190 → 185).
    pub row_active_border: u8,
    /// Selected-state highlight tint (row).
    pub row_active_tint: u8,
    /// Hover-state highlight tint (row).
    pub row_hover_tint: u8,

    // ── Nav cursor (keyboard-nav highlight on rows / docks) ──
    /// Nav-cursor outline — unifies row/dock (220/235 → 225).
    pub nav_outline: u8,
    /// Nav-cursor wash fill (dock).
    pub nav_wash: u8,

    // ── Tonal fills (badges / tags / alerts / toasts) ──
    /// Badge fill — unifies badge/badge_button (both `38`).
    pub badge_fill: u8,
    /// Tag / alert tonal fill — unifies tag/alert (both `22`).
    pub tag_fill: u8,
    /// Tag border / divider.
    pub tag_border: u8,
    /// Toast background tint.
    pub toast_tint: u8,
    /// Badge-button outline, resting.
    pub outline_rest: u8,
    /// Badge-button outline, hovered.
    pub outline_hover: u8,

    // ── Text selection / list hilite ──
    /// Text-selection fill (input).
    pub selection: u8,
    /// Dropdown row hilite (select).
    pub hilite: u8,

    // ── Overlays (modal / palette / context menu) ──
    /// Backdrop scrim behind a modal/palette — unifies modal/palette (150/140 → 150).
    pub scrim: u8,
    /// Keycap background — unifies context_menu/key_hint (both `200`).
    pub keycap: u8,
    /// Overlay panel border (palette / context menu).
    pub panel_border: u8,
    /// Overlay panel selected-row border.
    pub panel_row_border: u8,
    /// Overlay panel selected-row fill.
    pub panel_row_fill: u8,
    /// Tooltip border, in the accent hue.
    pub tooltip_border: u8,
    /// Context-menu shortcut/hint text.
    pub menu_shortcut: u8,
    /// Context-menu shortcut/hint text, dimmed (disabled row).
    pub menu_shortcut_dim: u8,

    // ── Scrollbar thumb ──
    /// Scrollbar thumb, resting — unifies scroll_bar/scroll_region (both `90`).
    pub thumb_rest: u8,
    /// Scrollbar thumb, hovered — unifies scroll_bar/scroll_region (both `200`).
    pub thumb_hover: u8,

    // ── Dim / unlit ──
    /// Unlit / dimmed element (gauge).
    pub unlit: u8,
}

impl Default for InteractionAlphas {
    fn default() -> Self {
        Self {
            toggle_on_fill: 128,
            control_hover_fill: 28,
            control_hover_border: 190,
            control_active_fill: 64,
            control_active_border: 215,
            control_rest_border: 150,
            control_rest_glow: 30,
            row_hover_fill: 16,
            row_active_fill: 30,
            row_active_border: 185,
            row_active_tint: 90,
            row_hover_tint: 40,
            nav_outline: 225,
            nav_wash: 30,
            badge_fill: 38,
            tag_fill: 22,
            tag_border: 130,
            toast_tint: 16,
            outline_rest: 150,
            outline_hover: 235,
            selection: 70,
            hilite: 48,
            scrim: 150,
            keycap: 200,
            panel_border: 200,
            panel_row_border: 150,
            panel_row_fill: 30,
            tooltip_border: 180,
            menu_shortcut: 180,
            menu_shortcut_dim: 120,
            thumb_rest: 90,
            thumb_hover: 200,
            unlit: 40,
        }
    }
}
